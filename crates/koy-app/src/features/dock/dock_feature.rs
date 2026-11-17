//! Dock özelliği: ekran başına dock penceresi, sabitli uygulamalar, pencere izleme, AppBar (yer ayırma), görev
//! çubuğunu gizleme ve tepsi barındırma, kenardan açma, tam ekran ve kabuk açılır pencereleri. Karşılığı:
//! `Features/Dock/DockFeature.cs`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::{Duration, Instant};

use koy_core::display::{DisplayMonitor, IMonitorService, MonitorPolicy};
use koy_core::dock::{
    AppBarOwnership, DockCandidateWindow, DockController, DockEdge, DockForegroundFilter, DockPositionResolver, DockPresentationMath,
    DockProximityMapping, DockProximityMode, DockSettingsDiff, DockWindowMonitorScope, EdgeRevealTracker, IAppBarService,
    OrderedPinnedAppsService,
};
use koy_core::event::Subscription;
use koy_core::power::{BatterySaverPolicy, IPowerService};
use koy_core::settings::{AppSettings, ValueList};
use koy_core::shell::{IFullscreenDetector, IShellActionService, IWindowTracker, ShellFlyouts, WindowInfo, WindowRect};
use koy_core::start_menu::StartMenuActions;
use koy_core::time::TimeSpan;
use koy_core::tray::{ITrayService, TrayHostAction, TrayHostPolicy, TrayHostRecovery};
use koy_core::visibility::{VisibilityController, VisibilityMode};
use koy_platform::display::MonitorService;
use koy_platform::jump_list::JumpListService;
use koy_platform::power::PowerService;
use koy_platform::search::SystemCommandService;
use koy_platform::shell::{
    AppBarService, CursorHelper, FullscreenDetector, PinnedAppsService, PopupMenuProbe, RecentFilesService, ShellActionService, TaskbandFavoritesReader,
    TrayHostService, WindowStyles, WindowTracker, taskbar_service,
};
use koy_platform::start_menu::StartMenuHost;

use super::context_menu_state;
use super::dock_settings_store;
use super::dock_view_model::{DockParts, DockViewModel};
use super::dock_window::DockWindowHost;
use crate::features::Feature;
use crate::infra::dispatch;
use crate::infra::observable::{LocalEvent, SharedSettings};
use crate::infra::ui_ref::UiRef;
use crate::shared_services::SharedServices;

const EDGE_PEEK_INTERVAL: Duration = Duration::from_millis(100);
const APP_BAR_REASSERT_QUIET: Duration = Duration::from_millis(500);
const APP_BAR_SETTLE_DELAYS_MS: [u64; 3] = [1500, 3000, 5000];

struct DockWindowState {
    monitor: DisplayMonitor,
    host: Rc<DockWindowHost>,
    visibility: Arc<VisibilityController>,
    edge_tracker: EdgeRevealTracker,
    edge_sub: u64,
}

impl DockWindowState {
    fn dispose(&self) {
        let vm = self.host.vm();
        vm.edge_watch_changed.unsubscribe(self.edge_sub);
        self.host.dispose();
        vm.dispose();
        self.visibility.dispose();
    }
}

#[derive(Default)]
struct Svc {
    tracker: Option<(Arc<WindowTracker>, Subscription, Subscription)>,
    shell: Option<Arc<ShellActionService>>,
    jump_lists: Option<Arc<JumpListService>>,
    start_menu: Option<Arc<StartMenuActions>>,
    fullscreen: Option<Arc<FullscreenDetector>>,
    app_bar: Option<Arc<AppBarService>>,
    monitors: Option<(Arc<MonitorService>, Subscription)>,
    media: Option<Arc<koy_platform::media::MediaService>>,
    power: Option<(Arc<PowerService>, Subscription)>,
    tray: Option<(Arc<TrayHostService>, Subscription)>,
    controller: Option<Arc<DockController>>,
}

pub struct Inner {
    settings: RefCell<SharedSettings>,
    services: Rc<SharedServices>,
    recent_files: Arc<RecentFilesService>,
    windows: RefCell<Vec<DockWindowState>>,
    svc: RefCell<Svc>,
    edge_timer: slint::Timer,
    app_bar_settle_timer: slint::Timer,
    app_bar_settle_stage: Cell<usize>,
    last_app_bar_reassert: RefCell<HashMap<isize, Instant>>,
    tray_recovery: RefCell<TrayHostRecovery>,
    tray_retry_timer: slint::Timer,
    taskbar_hidden: Cell<bool>,
    tray_released_by_user: Cell<bool>,
    reduce_motion: Cell<bool>,
    stopping: Cell<bool>,
    last_flyout: Cell<(u32, bool)>,
    me: RefCell<Weak<Self>>,
    /// Pencere sayısı değişti (menüdeki ekran özeti).
    pub window_count_changed: LocalEvent<usize>,
}

/// Dock özelliği.
pub struct DockFeature {
    inner: Rc<Inner>,
}

impl DockFeature {
    pub fn new(settings: SharedSettings, services: Rc<SharedServices>) -> Self {
        let inner = Rc::new(Inner {
            settings: RefCell::new(settings),
            services,
            recent_files: Arc::new(RecentFilesService),
            windows: RefCell::new(Vec::new()),
            svc: RefCell::new(Svc::default()),
            edge_timer: slint::Timer::default(),
            app_bar_settle_timer: slint::Timer::default(),
            app_bar_settle_stage: Cell::new(0),
            last_app_bar_reassert: RefCell::new(HashMap::new()),
            tray_recovery: RefCell::new(TrayHostRecovery::new()),
            tray_retry_timer: slint::Timer::default(),
            taskbar_hidden: Cell::new(false),
            tray_released_by_user: Cell::new(false),
            reduce_motion: Cell::new(false),
            stopping: Cell::new(false),
            last_flyout: Cell::new((0, false)),
            me: RefCell::new(Weak::new()),
            window_count_changed: LocalEvent::default(),
        });
        *inner.me.borrow_mut() = Rc::downgrade(&inner);
        DockFeature { inner }
    }

    pub fn window_count(&self) -> usize {
        self.inner.windows.borrow().len()
    }

    pub fn window_count_changed(&self) -> &LocalEvent<usize> {
        &self.inner.window_count_changed
    }

    /// Ayarlar yerinde uygulanır; değişen alanlara göre (DockSettingsDiff) yeniden kurulum yapılır.
    pub fn apply_settings(&self, next: SharedSettings) {
        Inner::apply_settings(&self.inner, next);
    }

    /// Explorer yeniden başladı: görev çubuğu gizlemesi ve AppBar kayıtları yinelenir.
    pub fn reapply_taskbar_hiding(&self) {
        self.inner.reapply_taskbar_hiding();
    }

    /// Kullanıcı görev çubuğunu elle geri getirdi: tepsi bırakılır, AppBar onun üstüne yığılır.
    pub fn on_taskbar_restored_manually(&self) {
        self.inner.tray_released_by_user.set(true);
        self.inner.update_app_bar_ownership();
        if self.inner.svc.borrow().tray.is_some() {
            self.inner.release_tray();
        }
    }
}

impl Feature for DockFeature {
    fn start(&self) {
        Inner::start(&self.inner);
    }

    fn stop(&self) {
        self.inner.stop_core();
        // Görev çubuğu her durumda geri gelir.
        taskbar_service::restore(0, false);
    }
}

fn dpi_scale_for(m: &DisplayMonitor, edge: DockEdge) -> f64 {
    if matches!(edge, DockEdge::Left | DockEdge::Right) { m.dpi_scale_x } else { m.dpi_scale_y }
}

fn tick() -> TimeSpan {
    thread_local! { static START: Instant = Instant::now(); }
    TimeSpan::from_milliseconds_f64(START.with(|s| s.elapsed().as_secs_f64() * 1000.0))
}

impl Inner {
    fn settings(&self) -> AppSettings {
        self.settings.borrow().borrow().clone()
    }

    fn weak(&self) -> Weak<Self> {
        self.me.borrow().clone()
    }

    fn effective_proximity(&self) -> DockProximityMode {
        let s = self.settings();
        DockProximityMapping::effective(s.dock.style, s.dock.proximity)
    }

    fn start(me: &Rc<Self>) {
        if !me.windows.borrow().is_empty() {
            return;
        }
        me.stopping.set(false);
        let settings = me.settings();
        let tracker = me.services.acquire_window_tracker();
        let shell = Arc::new(ShellActionService);
        let fullscreen = Arc::new(FullscreenDetector::default());
        let monitors = Arc::new(MonitorService::default());
        monitors.start();
        let ui = UiRef::new(me);
        let mon_sub = monitors.monitors_changed().subscribe(move |_| {
            let ui = ui.clone();
            dispatch::post(move || {
                if let Some(me) = ui.upgrade() {
                    Inner::on_monitors_changed(&me);
                }
            });
        });
        // Sabitli uygulamalar: sıra ayardan, ilk açılışta Windows'un görev çubuğu sırası tohum.
        let order_settings = Arc::new(std::sync::Mutex::new(settings.dock_items.pinned_order.to_vec()));
        let load = Arc::clone(&order_settings);
        let save_ui = UiRef::new(me);
        let save_order = Arc::clone(&order_settings);
        let pinned = Arc::new(OrderedPinnedAppsService::new(
            Arc::new(PinnedAppsService),
            move || load.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            move |order: &[String]| {
                *save_order.lock().unwrap_or_else(|e| e.into_inner()) = order.to_vec();
                let (ui, order) = (save_ui.clone(), order.to_vec());
                dispatch::post(move || {
                    if let Some(me) = ui.upgrade() {
                        let s = me.settings.borrow();
                        dock_settings_store::save_pinned_order(&order, None, Some(&mut s.borrow_mut()));
                    }
                });
                Ok(())
            },
            Some(Box::new(|| Ok(TaskbandFavoritesReader::read()))),
        ));
        let controller = Arc::new(DockController::new(
            pinned,
            Arc::clone(&tracker) as Arc<dyn IWindowTracker>,
            Arc::clone(&shell) as Arc<dyn IShellActionService>,
            Some(Arc::clone(&fullscreen) as Arc<dyn IFullscreenDetector>),
            Some(TimeSpan::from_millis(120)),
            Some(dispatch::post_fn()),
            None,
        ));
        {
            let mut svc = me.svc.borrow_mut();
            svc.shell = Some(shell);
            svc.jump_lists = Some(Arc::new(JumpListService::default()));
            svc.start_menu = Some(Arc::new(StartMenuActions::new(
                Arc::new(StartMenuHost),
                Arc::new(SystemCommandService),
                Some(Arc::new(|source: &str, message: &str| {
                    koy_core::diagnostics::CrashLog::write(source, &koy_core::diagnostics::CrashError::exception("Exception", message, ""));
                })),
            )));
            svc.fullscreen = Some(fullscreen);
            svc.controller = Some(Arc::clone(&controller));
            svc.monitors = Some((Arc::clone(&monitors), mon_sub));
            // Sahiplik görev çubuğu gerçekten gizlenince (update_app_bar_ownership) verilir.
            if me.effective_proximity() == DockProximityMode::ReserveSpace {
                svc.app_bar = Some(Arc::new(AppBarService::default()));
            }
            svc.media = Some(me.services.acquire_media_service());
        }
        me.apply_window_behavior();
        if settings.appearance.battery_saver_reduce_motion {
            me.acquire_power();
        }
        for m in me.resolve_target_monitors(&monitors.get_monitors()) {
            Inner::create_dock_window(me, m);
        }
        me.apply_tray_policy();
        me.apply_taskbar_policy();
        // Ön plan ve pencere listesi.
        let ui = UiRef::new(me);
        let fg = tracker.foreground_window_changed().subscribe(move |info| {
            let (ui, info) = (ui.clone(), info.clone());
            dispatch::post(move || {
                if let Some(me) = ui.upgrade() {
                    me.on_foreground_window_changed(&info);
                }
            });
        });
        let ui = UiRef::new(me);
        let wc = tracker.windows_changed().subscribe(move |_| {
            let ui = ui.clone();
            dispatch::post(move || {
                if let Some(me) = ui.upgrade() {
                    me.on_windows_changed();
                }
            });
        });
        let was_running = tracker.is_started();
        tracker.start();
        let current = if was_running { tracker.current_foreground() } else { None };
        me.svc.borrow_mut().tracker = Some((tracker, fg, wc));
        if let Some(info) = current {
            me.on_foreground_window_changed(&info);
        }
        me.ensure_taskbar_hidden();
        controller.start();
        me.update_edge_timer();
        me.window_count_changed.invoke(&me.windows.borrow().len());
    }

    fn resolve_target_monitors(&self, monitors: &[DisplayMonitor]) -> Vec<DisplayMonitor> {
        let s = self.settings();
        MonitorPolicy::dock_monitors(&MonitorPolicy::resolve(Some(&s.displays.monitors), s.displays.new_screen_default, monitors))
    }

    fn apply_window_behavior(&self) {
        let Some(c) = self.svc.borrow().controller.clone() else { return };
        let s = self.settings();
        c.set_left_click_action(s.dock.left_click_action);
        c.set_grouping_mode(s.dock.grouping_mode);
        c.set_refresh_on_foreground_move(s.dock.window_monitor_scope == DockWindowMonitorScope::CurrentMonitor);
    }

    fn new_edge_tracker(&self) -> EdgeRevealTracker {
        let s = self.settings();
        EdgeRevealTracker::new(if self.effective_proximity() == DockProximityMode::Dodge {
            TimeSpan::ZERO
        } else {
            DockPresentationMath::edge_reveal_delay(s.dock.reveal_trigger, s.dock.reveal_delay_ms)
        })
    }

    fn create_view_model(&self, visibility: &Arc<VisibilityController>, monitor_bounds: WindowRect) -> Rc<DockViewModel> {
        let svc = self.svc.borrow();
        let tray = svc.tray.as_ref().map(|(t, _)| Arc::clone(t) as Arc<dyn ITrayService>);
        let vm = DockViewModel::new(DockParts {
            settings: Rc::clone(&self.settings.borrow()),
            controller: svc.controller.clone().expect("dock denetleyicisi"),
            shell: svc.shell.clone().expect("kabuk eylemleri") as Arc<dyn IShellActionService>,
            visibility: Some(Arc::clone(visibility)),
            status: None,
            media: svc.media.clone().map(|m| m as Arc<dyn koy_core::media::IMediaService>),
            tray,
            recent_files: Some(Arc::clone(&self.recent_files) as Arc<dyn koy_core::dock::IRecentFilesService>),
            jump_lists: svc.jump_lists.clone().map(|j| j as Arc<dyn koy_core::jump_list::IJumpListService + Send + Sync>),
        });
        drop(svc);
        vm.set_reduce_motion(self.reduce_motion.get());
        vm.set_monitor_bounds(monitor_bounds);
        vm
    }

    fn create_dock_window(me: &Rc<Self>, monitor: DisplayMonitor) {
        let s = me.settings();
        let visibility = Arc::new(VisibilityController::new(s.dock.visibility, None));
        let vm = me.create_view_model(&visibility, monitor.bounds);
        let Some(host) = DockWindowHost::new(Rc::clone(&vm)) else { return };
        *host.start_menu.borrow_mut() = me.svc.borrow().start_menu.clone();
        host.set_target_monitor(Some(monitor.clone()));
        let w = me.weak();
        let edge_sub = vm.edge_watch_changed.subscribe(move || {
            if let Some(me) = w.upgrade() {
                me.update_edge_timer();
            }
        });
        // Yerel pencere hazır: tam ekran yok sayma, kabuk kancası, AppBar kaydı, sınırlar.
        {
            let w = me.weak();
            let vis = Arc::clone(&visibility);
            let hw = Rc::downgrade(&host);
            let mon = monitor.clone();
            *host.native_ready.borrow_mut() = Some(Box::new(move |hwnd| {
                let (Some(me), Some(host)) = (w.upgrade(), hw.upgrade()) else { return };
                vis.set_notch_handle(hwnd);
                let svc = me.svc.borrow();
                if let Some(f) = &svc.fullscreen {
                    f.ignored_handles().insert(hwnd);
                }
                if let Some((t, ..)) = &svc.tracker {
                    t.register_shell_hook(hwnd);
                    let tracker = Arc::clone(t);
                    host.set_shell_hook(t.shell_hook_message(), move |wp, lp| {
                        crate::infra::trace::bump(&crate::infra::trace::DOCK_SHELL);
                        tracker.handle_shell_hook_message(wp, lp)
                    });
                }
                if let Some(ab) = &svc.app_bar {
                    let edge = me.settings().dock.edge;
                    let h = if host.actual_thickness() > 0.0 { host.reserved_height() } else { DockPositionResolver::SNAPPED_HEIGHT_DIP };
                    ab.register(hwnd, h, dpi_scale_for(&mon, edge), edge);
                }
                drop(svc);
                update_dock_bounds(&host, &vis);
            }));
        }
        {
            let w = me.weak();
            let vis = Arc::clone(&visibility);
            let hw = Rc::downgrade(&host);
            *host.bounds_changed_handler.borrow_mut() = Some(Box::new(move || {
                let (Some(me), Some(host)) = (w.upgrade(), hw.upgrade()) else { return };
                update_dock_bounds(&host, &vis);
                let svc = me.svc.borrow();
                if let Some(ab) = &svc.app_bar
                    && host.hwnd() != 0
                    && let Some(m) = host.target_monitor()
                {
                    let edge = me.settings().dock.edge;
                    ab.update_position(host.hwnd(), host.reserved_height(), dpi_scale_for(&m, edge), edge);
                }
            }));
        }
        {
            let mon = me.svc.borrow().monitors.as_ref().map(|(m, _)| Arc::clone(m));
            *host.display_change_handler.borrow_mut() = Some(Box::new(move || {
                if let Some(m) = &mon {
                    m.notify_display_changed();
                }
            }));
            let w = me.weak();
            let hw = Rc::downgrade(&host);
            *host.app_bar_pos_changed_handler.borrow_mut() = Some(Box::new(move || {
                if let (Some(me), Some(host)) = (w.upgrade(), hw.upgrade()) {
                    me.reassert_app_bar(&host, true);
                }
            }));
        }
        host.show();
        me.windows.borrow_mut().push(DockWindowState { monitor, host, visibility, edge_tracker: me.new_edge_tracker(), edge_sub });
    }

    // ---- AppBar ----

    fn reassert_app_bar(&self, host: &Rc<DockWindowHost>, only_if_needed: bool) {
        crate::infra::trace::bump(&crate::infra::trace::DOCK_APPBAR);
        let Some(ab) = self.svc.borrow().app_bar.clone() else { return };
        if host.hwnd() == 0 || self.stopping.get() {
            return;
        }
        if only_if_needed {
            let now = Instant::now();
            let mut last = self.last_app_bar_reassert.borrow_mut();
            if last.get(&host.hwnd()).is_some_and(|t| now - *t < APP_BAR_REASSERT_QUIET) {
                return;
            }
            last.insert(host.hwnd(), now);
        }
        let Some(m) = host.target_monitor() else { return };
        let edge = self.settings().dock.edge;
        let height = if host.actual_thickness() > 0.0 { host.reserved_height() } else { DockPositionResolver::SNAPPED_HEIGHT_DIP };
        ab.reassert(host.hwnd(), height, dpi_scale_for(&m, edge), only_if_needed, edge);
    }

    fn hosts(&self) -> Vec<Rc<DockWindowHost>> {
        self.windows.borrow().iter().map(|w| Rc::clone(&w.host)).collect()
    }

    fn update_app_bar_ownership(&self) {
        let Some(ab) = self.svc.borrow().app_bar.clone() else { return };
        let s = self.settings();
        let owns = AppBarOwnership::owns_bottom_edge(s.dock.hide_windows_taskbar, self.taskbar_hidden.get(), self.tray_released_by_user.get());
        if ab.owns_bottom_edge() == owns {
            return;
        }
        ab.set_owns_bottom_edge(owns);
        for h in self.hosts() {
            self.reassert_app_bar(&h, false);
        }
    }

    /// Gezgin AppBar konumunu birkaç kez geç düzeltir: kısa aralıklarla yeniden doğrulanır.
    fn schedule_app_bar_settle(&self) {
        if self.svc.borrow().app_bar.is_none() || self.stopping.get() {
            return;
        }
        self.app_bar_settle_stage.set(0);
        self.start_settle(Duration::from_millis(700));
    }

    fn start_settle(&self, after: Duration) {
        let w = self.weak();
        self.app_bar_settle_timer.start(slint::TimerMode::SingleShot, after, move || {
            if let Some(me) = w.upgrade() {
                for h in me.hosts() {
                    me.reassert_app_bar(&h, true);
                }
                let stage = me.app_bar_settle_stage.get();
                if stage < APP_BAR_SETTLE_DELAYS_MS.len() {
                    me.app_bar_settle_stage.set(stage + 1);
                    me.start_settle(Duration::from_millis(APP_BAR_SETTLE_DELAYS_MS[stage]));
                }
            }
        });
    }

    fn reapply_taskbar_hiding(&self) {
        if self.windows.borrow().is_empty() {
            return;
        }
        let s = self.settings();
        if s.dock.hide_windows_taskbar {
            taskbar_service::hide();
            if let Some((t, _)) = self.svc.borrow().tray.as_ref() {
                t.ensure_first();
            }
        }
        let Some(ab) = self.svc.borrow().app_bar.clone() else { return };
        for w in self.windows.borrow().iter() {
            if w.host.hwnd() != 0 {
                ab.reregister(w.host.hwnd(), w.host.reserved_height(), dpi_scale_for(&w.monitor, s.dock.edge), s.dock.edge);
            }
        }
    }

    fn ensure_taskbar_hidden(&self) {
        if self.stopping.get() {
            return;
        }
        if self.settings().dock.hide_windows_taskbar && !self.windows.borrow().is_empty() {
            if taskbar_service::ensure_hidden() && self.tray_released_by_user.get() {
                self.tray_released_by_user.set(false);
                self.taskbar_hidden.set(true);
                self.update_app_bar_ownership();
                self.schedule_app_bar_settle();
            }
            self.apply_tray_policy();
            if let Some((t, _)) = self.svc.borrow().tray.as_ref() {
                t.ensure_first();
            }
        }
    }

    fn apply_taskbar_policy(&self) {
        if !self.settings().dock.hide_windows_taskbar {
            return;
        }
        if !self.windows.borrow().is_empty() {
            taskbar_service::hide();
            self.taskbar_hidden.set(true);
            if let Some((t, _)) = self.svc.borrow().tray.as_ref() {
                t.ensure_first();
            }
            self.update_app_bar_ownership();
            self.schedule_app_bar_settle();
        } else if self.taskbar_hidden.get() {
            taskbar_service::restore(0, false);
            self.taskbar_hidden.set(false);
            self.update_app_bar_ownership();
        }
    }

    // ---- Tepsi barındırma ----

    fn apply_tray_policy(&self) {
        if self.stopping.get() {
            return;
        }
        let s = self.settings();
        let count = self.windows.borrow().len() as i32;
        let should_host = !self.tray_released_by_user.get() && TrayHostPolicy::should_host(s.dock.hide_windows_taskbar, s.dock.show_tray, count);
        let now = chrono::Utc::now().naive_utc();
        let (has, running) = {
            let svc = self.svc.borrow();
            (svc.tray.is_some(), svc.tray.as_ref().is_some_and(|(t, _)| t.is_running()))
        };
        let action = self.tray_recovery.borrow_mut().decide(should_host, has, running, now);
        if matches!(action, TrayHostAction::Release | TrayHostAction::Reacquire) {
            self.release_tray();
        }
        if matches!(action, TrayHostAction::Acquire | TrayHostAction::Reacquire) {
            let tray = self.services.acquire_tray_host();
            self.tray_recovery.borrow_mut().report_acquire(tray.is_some(), now);
            if let Some(t) = tray {
                let ui = UiRef::new(&self.me.borrow().upgrade().expect("dock özelliği"));
                let tw = Arc::downgrade(&t);
                let sub = t.exited().subscribe(move |_| {
                    let (ui, tw) = (ui.clone(), tw.clone());
                    dispatch::post(move || {
                        if let Some(me) = ui.upgrade() {
                            let same = me.svc.borrow().tray.as_ref().is_some_and(|(cur, _)| tw.upgrade().is_some_and(|x| Arc::ptr_eq(&x, cur)));
                            if same {
                                me.apply_tray_policy();
                            }
                        }
                    });
                });
                for h in self.hosts() {
                    h.vm().set_tray_service(Some(Arc::clone(&t) as Arc<dyn ITrayService>));
                }
                self.svc.borrow_mut().tray = Some((t, sub));
            }
        }
        let failures = self.tray_recovery.borrow().failures();
        if should_host && self.svc.borrow().tray.is_none() && failures > 0 && !self.tray_retry_timer.running() {
            let factor = 2f64.powi((failures - 1).min(4));
            let delay = TrayHostRecovery::FIRST_RETRY_DELAY.total_milliseconds() * factor + 100.0;
            let w = self.weak();
            self.tray_retry_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(delay as u64), move || {
                if let Some(me) = w.upgrade() {
                    me.apply_tray_policy();
                }
            });
        }
    }

    fn release_tray(&self) {
        for h in self.hosts() {
            h.vm().set_tray_service(None);
        }
        self.tray_retry_timer.stop();
        if let Some((t, sub)) = self.svc.borrow_mut().tray.take() {
            t.exited().unsubscribe(sub);
            self.services.release_tray_host();
        }
    }

    // ---- Güç ----

    fn acquire_power(&self) {
        let power = self.services.acquire_power_service();
        let ui = UiRef::new(&self.me.borrow().upgrade().expect("dock özelliği"));
        let sub = power.power_changed().subscribe(move |_| {
            let ui = ui.clone();
            dispatch::post(move || {
                if let Some(me) = ui.upgrade() {
                    me.apply_reduce_motion();
                }
            });
        });
        self.svc.borrow_mut().power = Some((power, sub));
        self.apply_reduce_motion();
    }

    fn apply_reduce_motion(&self) {
        let Some(p) = self.svc.borrow().power.as_ref().map(|(p, _)| Arc::clone(p)) else { return };
        let reduce = BatterySaverPolicy::should_reduce_motion(self.settings().appearance.battery_saver_reduce_motion, p.has_battery(), p.status());
        self.reduce_motion.set(reduce);
        for h in self.hosts() {
            h.vm().set_reduce_motion(reduce);
        }
    }

    fn reconcile_power(&self) {
        let enabled = self.settings().appearance.battery_saver_reduce_motion;
        let has = self.svc.borrow().power.is_some();
        if enabled && !has {
            self.acquire_power();
        } else if !enabled && has {
            if let Some((p, sub)) = self.svc.borrow_mut().power.take() {
                p.power_changed().unsubscribe(sub);
                self.services.release_power_service();
            }
            self.reduce_motion.set(false);
            for h in self.hosts() {
                h.vm().set_reduce_motion(false);
            }
        }
    }

    // ---- Ayarlar ----

    fn apply_settings(me: &Rc<Self>, next: SharedSettings) {
        if me.stopping.get() || me.svc.borrow().controller.is_none() {
            return;
        }
        let before = me.settings();
        let after = next.borrow().clone();
        let diff = DockSettingsDiff::compare(&before, &after);
        *me.settings.borrow_mut() = Rc::clone(&next);
        for h in me.hosts() {
            h.vm().adopt_settings(Rc::clone(&next));
        }
        me.apply_window_behavior();
        if diff.is_empty() {
            return;
        }
        if diff.reconcile_monitors {
            Inner::on_monitors_changed(me);
        }
        if diff.reconcile_power {
            me.reconcile_power();
        }
        if diff.rebuild_view_models {
            let n = me.windows.borrow().len();
            for i in 0..n {
                me.rebuild_view_model(i);
            }
        }
        if diff.reconcile_app_bar {
            me.reconcile_app_bar();
        }
        if diff.reconcile_tray || diff.reconcile_taskbar {
            me.apply_tray_policy();
            if after.dock.hide_windows_taskbar {
                me.apply_taskbar_policy();
            } else if me.taskbar_hidden.get() {
                taskbar_service::restore(0, false);
                me.taskbar_hidden.set(false);
            }
        }
        me.update_app_bar_ownership();
        me.update_edge_timer();
        let fg = me.svc.borrow().tracker.as_ref().and_then(|(t, ..)| t.current_foreground());
        if let Some(info) = fg.filter(|f| f.handle != 0) {
            me.recheck_foreground_state(&info);
        }
    }

    fn rebuild_view_model(&self, index: usize) {
        let (host, old_vis, monitor) = {
            let w = self.windows.borrow();
            let s = &w[index];
            (Rc::clone(&s.host), Arc::clone(&s.visibility), s.monitor.clone())
        };
        let old_vm = host.vm();
        let s = self.settings();
        let visibility = Arc::new(VisibilityController::new(s.dock.visibility, None));
        visibility.set_notch_handle(host.hwnd());
        let vm = self.create_view_model(&visibility, monitor.bounds);
        let w = self.weak();
        let edge_sub = vm.edge_watch_changed.subscribe(move || {
            if let Some(me) = w.upgrade() {
                me.update_edge_timer();
            }
        });
        old_vm.edge_watch_changed.unsubscribe(self.windows.borrow()[index].edge_sub);
        host.set_view_model(vm);
        {
            let mut ws = self.windows.borrow_mut();
            ws[index].visibility = visibility;
            ws[index].edge_tracker = self.new_edge_tracker();
            ws[index].edge_sub = edge_sub;
        }
        old_vm.dispose();
        old_vis.dispose();
    }

    fn reconcile_app_bar(&self) {
        let s = self.settings();
        if self.effective_proximity() == DockProximityMode::ReserveSpace {
            if self.svc.borrow().app_bar.is_none() {
                self.svc.borrow_mut().app_bar = Some(Arc::new(AppBarService::default()));
            }
            self.update_app_bar_ownership();
            let ab = self.svc.borrow().app_bar.clone().expect("AppBar");
            for w in self.windows.borrow().iter() {
                if w.host.hwnd() == 0 {
                    continue;
                }
                let dpi = dpi_scale_for(&w.monitor, s.dock.edge);
                let h = if w.host.actual_thickness() > 0.0 { w.host.reserved_height() } else { DockPositionResolver::SNAPPED_HEIGHT_DIP };
                ab.register(w.host.hwnd(), h, dpi, s.dock.edge);
                ab.update_position(w.host.hwnd(), h, dpi, s.dock.edge);
            }
        } else if let Some(ab) = self.svc.borrow_mut().app_bar.take() {
            for w in self.windows.borrow().iter() {
                if w.host.hwnd() != 0 {
                    ab.unregister(w.host.hwnd());
                }
            }
            ab.dispose();
        }
    }

    // ---- Ekranlar ----

    fn on_monitors_changed(me: &Rc<Self>) {
        let Some(service) = me.svc.borrow().monitors.as_ref().map(|(m, _)| Arc::clone(m)) else { return };
        let targets = me.resolve_target_monitors(&service.get_monitors());
        let removed: Vec<DockWindowState> = {
            let mut ws = me.windows.borrow_mut();
            let (keep, gone): (Vec<_>, Vec<_>) = ws.drain(..).partition(|w| targets.iter().any(|m| m.id.eq_ignore_ascii_case(&w.monitor.id)));
            *ws = keep;
            gone
        };
        for w in removed {
            let hwnd = w.host.hwnd();
            if hwnd != 0 {
                let svc = me.svc.borrow();
                if let Some(ab) = &svc.app_bar {
                    ab.unregister(hwnd);
                }
                if let Some((t, ..)) = &svc.tracker {
                    t.deregister_shell_hook(hwnd);
                }
                if let Some(f) = &svc.fullscreen {
                    f.ignored_handles().remove(&hwnd);
                }
            }
            w.dispose();
        }
        for m in targets {
            let found = me.windows.borrow().iter().position(|w| w.monitor.id.eq_ignore_ascii_case(&m.id));
            match found {
                Some(i) => {
                    let host = {
                        let mut ws = me.windows.borrow_mut();
                        ws[i].monitor = m.clone();
                        Rc::clone(&ws[i].host)
                    };
                    host.vm().set_monitor_bounds(m.bounds);
                    host.set_target_monitor(Some(m));
                    let vis = Arc::clone(&me.windows.borrow()[i].visibility);
                    update_dock_bounds(&host, &vis);
                }
                None => Inner::create_dock_window(me, m),
            }
        }
        me.tray_recovery.borrow_mut().reset_failures();
        me.apply_tray_policy();
        me.apply_taskbar_policy();
        me.update_edge_timer();
        me.window_count_changed.invoke(&me.windows.borrow().len());
    }

    // ---- Ön plan ----

    fn on_windows_changed(&self) {
        self.ensure_taskbar_hidden();
        let fg = self.svc.borrow().tracker.as_ref().and_then(|(t, ..)| t.current_foreground());
        if let Some(info) = fg.filter(|f| f.handle != 0) {
            self.recheck_foreground_state(&info);
        }
    }

    fn on_foreground_window_changed(&self, info: &WindowInfo) {
        let pid = WindowStyles::get_window_process_id(info.handle);
        if pid != std::process::id() && context_menu_state::has_active_menu() {
            // Yerel menü dışarı tıklamada sistemce kapanır; durum yalnız sıfırlanır.
            context_menu_state::set_menu_open(false);
        }
        if let Some((t, _)) = self.svc.borrow().tray.as_ref() {
            t.ensure_first();
        }
        self.recheck_foreground_state(info);
    }

    fn is_shell_flyout(&self, pid: u32) -> bool {
        if pid == 0 {
            return false;
        }
        let (last_pid, last) = self.last_flyout.get();
        if pid == last_pid {
            return last;
        }
        let name = process_name(pid);
        let result = ShellFlyouts::is_flyout_process(name.as_deref());
        self.last_flyout.set((pid, result));
        result
    }

    fn set_shell_flyout_open(&self, monitor_handle: isize) {
        for w in self.windows.borrow().iter() {
            w.host.vm().set_shell_flyout_open(monitor_handle != 0 && w.monitor.handle == monitor_handle);
        }
    }

    fn recheck_foreground_state(&self, info: &WindowInfo) {
        let Some(fullscreen) = self.svc.borrow().fullscreen.clone() else { return };
        if info.handle == 0 {
            self.set_shell_flyout_open(0);
            return;
        }
        let pid = WindowStyles::get_window_process_id(info.handle);
        if pid == std::process::id() {
            return;
        }
        let fg_mon = WindowStyles::get_window_monitor(info.handle);
        if self.is_shell_flyout(pid) {
            self.set_shell_flyout_open(fg_mon);
            self.update_edge_timer();
            return;
        }
        self.set_shell_flyout_open(0);
        let maximized = WindowStyles::is_window_maximized(info.handle);
        let tracker = self.svc.borrow().tracker.as_ref().map(|(t, ..)| Arc::clone(t));
        let mut candidates: Option<Vec<DockCandidateWindow>> = None;
        let own = std::process::id() as i32;
        for w in self.windows.borrow().iter() {
            let on_this = DockForegroundFilter::applies_to_dock(fg_mon, w.monitor.handle);
            let (bounds, monitor_maximized, active) = if on_this {
                (info.bounds, maximized, info.handle)
            } else {
                let list = candidates.get_or_insert_with(|| {
                    tracker
                        .as_ref()
                        .map(|t| {
                            t.get_open_windows()
                                .into_iter()
                                .filter(|o| o.process_id != own)
                                .map(|o| {
                                    DockCandidateWindow::with_state(
                                        o.handle,
                                        o.bounds,
                                        WindowStyles::get_window_monitor(o.handle),
                                        WindowStyles::is_window_maximized(o.handle),
                                        WindowStyles::is_window_minimized(o.handle),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                });
                DockForegroundFilter::resolve_monitor_window(w.monitor.handle, info.clone(), fg_mon, maximized, Some(list))
            };
            let is_fullscreen = fullscreen.is_fullscreen_on(w.monitor.handle);
            w.visibility.set_fullscreen(is_fullscreen);
            w.visibility.update_foreground_window(active, bounds);
            let m = &w.monitor;
            let bounds_dip = if bounds.is_empty() {
                WindowRect::EMPTY
            } else {
                WindowRect {
                    left: (bounds.left as f64 / m.dpi_scale_x) as i32,
                    top: (bounds.top as f64 / m.dpi_scale_y) as i32,
                    right: (bounds.right as f64 / m.dpi_scale_x) as i32,
                    bottom: (bounds.bottom as f64 / m.dpi_scale_y) as i32,
                }
            };
            w.host.vm().update_foreground_window(bounds_dip, monitor_maximized, is_fullscreen);
            let hwnd = w.host.hwnd();
            if !is_fullscreen
                && !monitor_maximized
                && hwnd != 0
                && info.handle != hwnd
                && !FullscreenDetector::is_screen_clipping_host(info.handle)
                && !PopupMenuProbe::any_visible_menu()
                && !context_menu_state::has_active_menu()
            {
                WindowStyles::reassert_topmost(hwnd);
            }
        }
        self.update_edge_timer();
    }

    // ---- Kenardan açma ----

    fn update_edge_timer(&self) {
        if self.windows.borrow().is_empty() {
            return;
        }
        let s = self.settings();
        let proximity = self.effective_proximity();
        let should_run = self.windows.borrow().iter().any(|w| {
            ((matches!(proximity, DockProximityMode::Dodge | DockProximityMode::AlwaysHidden) || s.dock.visibility != VisibilityMode::Fixed)
                && proximity != DockProximityMode::ReserveSpace)
                || (s.dock.edge_reveal && w.host.vm().needs_edge_watch())
        });
        if should_run {
            if !self.edge_timer.running() {
                let w = self.weak();
                self.edge_timer.start(slint::TimerMode::Repeated, EDGE_PEEK_INTERVAL, move || {
                    if let Some(me) = w.upgrade() {
                        me.check_edge_peek();
                    }
                });
            }
        } else if self.edge_timer.running() {
            self.edge_timer.stop();
        }
    }

    fn check_edge_peek(&self) {
        let Some(tracker) = self.svc.borrow().tracker.as_ref().map(|(t, ..)| Arc::clone(t)) else { return };
        let Some((x, y)) = tracker.get_cursor_position() else { return };
        let button = CursorHelper::is_any_mouse_button_down();
        let now = tick();
        let edge = self.settings().dock.edge;
        let mut ws = self.windows.borrow_mut();
        for w in ws.iter_mut() {
            let vm = w.host.vm();
            if w.visibility.is_fullscreen() {
                w.edge_tracker.reset();
                vm.set_mouse_at_bottom(false);
                continue;
            }
            let m = &w.monitor;
            let in_zone = EdgeRevealTracker::is_in_zone_with_edge(edge, x, y, m.bounds, vm.state().is_visible, m.dpi_scale_x, m.dpi_scale_y);
            let at = w.edge_tracker.update(in_zone, button, now);
            vm.set_mouse_at_bottom(at);
        }
    }

    // ---- Durdurma ----

    fn stop_core(&self) {
        self.stopping.set(true);
        self.app_bar_settle_timer.stop();
        self.edge_timer.stop();
        self.tray_retry_timer.stop();
        if self.svc.borrow().media.is_some() {
            self.svc.borrow_mut().media = None;
            self.services.release_media_service();
        }
        if self.svc.borrow().tray.is_some() {
            self.release_tray();
        }
        let mut svc = std::mem::take(&mut *self.svc.borrow_mut());
        if let Some((m, sub)) = svc.monitors.take() {
            m.monitors_changed().unsubscribe(sub);
            m.dispose();
        }
        if let Some(ab) = svc.app_bar.take() {
            for w in self.windows.borrow().iter() {
                if w.host.hwnd() != 0 {
                    ab.unregister(w.host.hwnd());
                }
            }
            ab.dispose();
        }
        if let Some((t, fg, wc)) = svc.tracker.take() {
            for w in self.windows.borrow().iter() {
                if w.host.hwnd() != 0 {
                    t.deregister_shell_hook(w.host.hwnd());
                }
            }
            t.foreground_window_changed().unsubscribe(fg);
            t.windows_changed().unsubscribe(wc);
            self.services.release_window_tracker();
        }
        let windows: Vec<DockWindowState> = self.windows.borrow_mut().drain(..).collect();
        for w in &windows {
            w.dispose();
        }
        self.window_count_changed.invoke(&0);
        if let Some((p, sub)) = svc.power.take() {
            p.power_changed().unsubscribe(sub);
            self.services.release_power_service();
        }
        if let Some(c) = svc.controller.take() {
            c.dispose();
        }
        if let Some(j) = svc.jump_lists.take() {
            koy_core::jump_list::IJumpListService::clear(j.as_ref());
        }
    }
}

/// Görünürlük denetleyicisine dock'un ekran dikdörtgeni (piksel) verilir.
fn update_dock_bounds(host: &DockWindowHost, visibility: &VisibilityController) {
    let r = host.window_rect();
    if r.right - r.left <= 0 {
        return;
    }
    visibility.set_notch_bounds(r);
}

fn process_name(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    use windows::core::PWSTR;
    // SAFETY: süreç tutamacı her yolda kapatılır; arabellek boyutu verilir.
    let path = unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size).is_ok();
        let _ = CloseHandle(process);
        ok.then(|| String::from_utf16_lossy(&buf[..size as usize]))?
    };
    std::path::Path::new(&path).file_stem().and_then(|s| s.to_str()).map(str::to_string)
}

#[allow(dead_code)]
fn _assert_list(_: ValueList<String>) {}

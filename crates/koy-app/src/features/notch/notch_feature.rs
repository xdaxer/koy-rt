//! Çentik özelliği: ekran başına bir çentik penceresi, ortak servisler ve olaylar. Karşılığı: `Features/Notch/NotchFeature.cs`.
//!
//! Bu dosya pencereleri (ekran değişince eklenip kaldırılır), görünüm modellerini, ön plan / tam ekran izlemeyi, kenara
//! dokunarak gösterme (edge peek), kısayolları (aç / kapat, arama, Raf) ve gösterge (ses, parlaklık, pil, Bluetooth)
//! olaylarını kurar. Bildirim kartları, güncelleme kartı ve sesli komutlar `notch_feature_cards.rs` içindedir.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slint::ComponentHandle;

use koy_core::bluetooth::{BluetoothHudPolicy, IBluetoothService};
use koy_core::clipboard::IClipboardHistoryService;
use koy_core::display::{DisplayMonitor, IMonitorService, MonitorPolicy, WindowPlacementMath};
use koy_core::shell::WindowRect;
use koy_core::event::Subscription;
use koy_core::lyrics::{LocalLyricsSource, LyricsCache, LyricsProvider};
use koy_core::network::INetworkService;
use koy_core::notch::{NotchEdgeLine, NotchLayout, NotchOpenBehavior, NotchPanels, NotchState, NotchStyle};
use koy_core::power::{BatteryAlertKind, BatteryNotificationPolicy, IPowerService};
use koy_core::privacy::IPrivacyMonitorService;
use koy_core::settings::SettingsStore;
use koy_core::shell::{IFullscreenDetector, IWindowTracker, WindowInfo};
use koy_core::timers::{FocusTimers, PomodoroConfig};
use koy_core::visibility::{VisibilityController, VisibilityMode};
use koy_core::weather::WeatherRefreshPolicy;
use koy_platform::audio::AudioService;
use koy_platform::bluetooth::BluetoothService;
use koy_platform::display::{BrightnessService, MonitorService};
use koy_platform::lyrics::LrclibSource;
use koy_platform::power::PowerService;
use koy_platform::shelf::ShelfService;
use koy_platform::shell::{FocusAssistService, FullscreenDetector, ShellActionService, WindowStyles, WindowTracker};
use koy_platform::weather::{PlaceNameService, WeatherService};

use super::notch_view_model::{NotchParts, NotchViewModel};
use super::notch_window::NotchWindowHost;
use super::notch_window_binding::NotchWindowBinding;
use crate::NotchWindow;
use crate::features::Feature;
use crate::features::calendar::calendar_view_model::CalendarViewModel;
use crate::features::calendar::shared_events::SharedEvents;
use crate::features::clipboard_panel::clipboard_view_model::ClipboardViewModel;
use crate::features::control_center::bluetooth_devices_view_model::BluetoothDevicesViewModel;
use crate::features::control_center::control_center_view_model::{ControlCenterFactories, ControlCenterViewModel};
use crate::features::control_center::wifi_page_view_model::WifiPageViewModel;
use crate::features::focus::focus_view_model::FocusViewModel;
use crate::features::focus::tasks_view_model::TasksViewModel;
use crate::features::home::quick_launch_view_model::QuickLaunchViewModel;
use crate::features::music::music_view_model::{MusicOptions, MusicViewModel};
use crate::features::search::search_factory;
use crate::features::search::search_view_model::SearchViewModel;
use crate::features::shelf::shelf_view_model::ShelfViewModel;
use crate::features::status::status_view_model::StatusViewModel;
use crate::features::system_info::system_stats_view_model::SystemStatsViewModel;
use crate::infra::dispatch;
use crate::infra::observable::SharedSettings;
use crate::infra::ui_ref::UiRef;
use crate::resources::theme_manager::ThemeManager;
use crate::shared_services::SharedServices;

const EDGE_PEEK_INTERVAL: Duration = Duration::from_millis(100);

/// Bir ekrandaki çentik.
pub(super) struct WindowState {
    pub monitor: Arc<Mutex<DisplayMonitor>>,
    pub host: Rc<NotchWindowHost>,
    pub binding: Rc<NotchWindowBinding>,
    pub vm: Rc<NotchViewModel>,
    pub visibility: Arc<VisibilityController>,
    pub controller: Arc<NotchController>,
    pub line: Option<Rc<super::notch_line_window::NotchLineWindow>>,
    visibility_sub: Option<Subscription>,
}

use koy_core::notch::NotchController;

impl WindowState {
    pub fn monitor(&self) -> DisplayMonitor {
        self.monitor.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn dispose(mut self) {
        if let Some(s) = self.visibility_sub.take() {
            self.visibility.changed().unsubscribe(s);
        }
        if let Some(l) = &self.line {
            l.close();
        }
        self.binding.dispose();
        self.host.dispose();
        self.vm.dispose();
        self.controller.dispose();
        self.visibility.dispose();
    }
}

#[derive(Default)]
pub(super) struct Services {
    pub focus_timers: Option<Arc<FocusTimers>>,
    pub media: Option<Arc<koy_platform::media::MediaService>>,
    pub power: Option<(Arc<PowerService>, Subscription)>,
    pub bluetooth: Option<(Arc<BluetoothService>, Subscription)>,
    pub network: Option<Arc<koy_platform::network::NetworkService>>,
    pub clipboard: Option<Arc<koy_platform::clipboard::ClipboardHistoryService>>,
    pub privacy: Option<(Arc<koy_platform::privacy::ConsentStorePrivacyService>, Subscription)>,
    pub weather: Option<Arc<WeatherService>>,
    pub place_names: Option<Arc<PlaceNameService>>,
    pub lyrics: Option<Arc<LyricsProvider>>,
    pub focus_assist: Option<Arc<FocusAssistService>>,
    pub shelf: Option<Arc<ShelfService>>,
    pub tracker: Option<(Arc<WindowTracker>, Subscription)>,
    pub monitors: Option<(Arc<MonitorService>, Subscription)>,
    pub osd_audio: Option<Arc<AudioService>>,
    pub osd_brightness: Option<Arc<BrightnessService>>,
    pub app_search: Option<Arc<koy_core::search::AppSearchSource>>,
}

pub(super) struct Inner {
    pub settings: SharedSettings,
    pub services: Rc<SharedServices>,
    pub shell_actions: Arc<ShellActionService>,
    pub shared_events: Rc<SharedEvents>,
    pub fullscreen: Arc<FullscreenDetector>,
    pub windows: RefCell<Vec<WindowState>>,
    pub svc: RefCell<Services>,
    pub battery_policy: RefCell<BatteryNotificationPolicy>,
    pub bluetooth_policy: RefCell<BluetoothHudPolicy>,
    pub edge_timer: slint::Timer,
    pub stopped: Cell<bool>,
    pub cards: super::notch_feature_cards::CardState,
}

thread_local! {
    static DEVICE_LOCATION_ATTEMPTED: Cell<bool> = const { Cell::new(false) };
}

/// Çentik özelliği. `start` pencereleri kurar, `stop` her şeyi bırakır.
pub struct NotchFeature {
    inner: Rc<Inner>,
}

impl NotchFeature {
    pub fn new(settings: SharedSettings, services: Rc<SharedServices>) -> Self {
        NotchFeature {
            inner: Rc::new(Inner {
                settings,
                services,
                shell_actions: Arc::new(ShellActionService),
                shared_events: SharedEvents::new(),
                fullscreen: Arc::new(FullscreenDetector::default()),
                windows: RefCell::new(Vec::new()),
                svc: RefCell::new(Services::default()),
                battery_policy: RefCell::new(BatteryNotificationPolicy::new()),
                bluetooth_policy: RefCell::new(BluetoothHudPolicy::new(None)),
                edge_timer: slint::Timer::default(),
                stopped: Cell::new(false),
                cards: Default::default(),
            }),
        }
    }

    /// Kısayol: imlecin bulunduğu ekrandaki çentiği açar ya da kapatır.
    pub fn toggle_expanded(&self) {
        let Some(i) = self.inner.pick_for_shortcut() else { return };
        let windows = self.inner.windows.borrow();
        let c = &windows[i].controller;
        if c.state() == NotchState::Expanded {
            c.on_click();
        } else {
            c.expand(None);
        }
    }

    /// Kısayol: aramayı açar / kapatır; `use_clipboard` ise panodaki metinle açılır.
    pub fn toggle_search(&self, use_clipboard: bool) {
        self.inner.toggle_search(use_clipboard);
    }

    /// Kısayol: çentiği Raf'a açar.
    pub fn open_shelf(&self) {
        let Some(i) = self.inner.pick_for_shortcut() else { return };
        let vm = Rc::clone(&self.inner.windows.borrow()[i].vm);
        vm.expand_to_shelf();
    }

    pub fn toggle_voice_command(&self) {
        super::notch_feature_cards::toggle_voice_command(&self.inner);
    }
}

impl Feature for NotchFeature {
    fn start(&self) {
        Inner::start(&self.inner);
    }

    fn stop(&self) {
        self.inner.stop();
    }
}

impl Inner {
    fn start(me: &Rc<Self>) {
        if !me.windows.borrow().is_empty() {
            return;
        }
        me.stopped.set(false);
        let settings = me.settings.borrow().clone();
        {
            let mut svc = me.svc.borrow_mut();
            svc.media = Some(me.services.acquire_media_service());
            svc.focus_timers = Some(Arc::new(FocusTimers::new(
                Some(PomodoroConfig::from_minutes(
                    settings.focus.pomodoro_work_minutes,
                    settings.focus.pomodoro_short_break_minutes,
                    settings.focus.pomodoro_long_break_minutes,
                )),
                None,
                None,
            )));
            // Pil uyarıları ve Bluetooth bağlantı göstergesi: olaylar arka plandan gelir, arayüze taşınır.
            let power = me.services.acquire_power_service();
            let ui = UiRef::new(me);
            let sub = power.power_changed().subscribe(move |e| {
                let ui = ui.clone();
                let e = e.clone();
                dispatch::post(move || {
                    if let Some(me) = ui.upgrade() {
                        me.on_power_changed(e);
                    }
                });
            });
            svc.power = Some((power, sub));
            let bt = me.services.acquire_bluetooth_service();
            me.bluetooth_policy.borrow_mut().update(&bt.devices());
            let ui = UiRef::new(me);
            let sub = bt.devices_changed().subscribe(move |_| {
                let ui = ui.clone();
                dispatch::post(move || {
                    if let Some(me) = ui.upgrade() {
                        me.on_bluetooth_devices_changed();
                    }
                });
            });
            svc.bluetooth = Some((bt, sub));
            svc.network = Some(me.services.acquire_network_service());
            if settings.clipboard.history_enabled {
                svc.clipboard = Some(me.services.acquire_clipboard_history());
            }
            if settings.notch.show_privacy_indicator {
                let privacy = me.services.acquire_privacy_monitor();
                let ui = UiRef::new(me);
                let sub = privacy.state_changed().subscribe(move |state| {
                    let ui = ui.clone();
                    let state = state.clone();
                    dispatch::post(move || {
                        if let Some(me) = ui.upgrade() {
                            for w in me.windows.borrow().iter() {
                                w.vm.update_privacy_state(&state);
                            }
                        }
                    });
                });
                svc.privacy = Some((privacy, sub));
            }
            svc.weather = Some(Arc::new(WeatherService::new(None, None, Some(WeatherRefreshPolicy::cache_duration(settings.weather.refresh_interval)))));
            svc.place_names = Some(Arc::new(PlaceNameService::new(None)));
            svc.lyrics = Some(Arc::new(LyricsProvider::new(
                Arc::new(LocalLyricsSource::new(LocalLyricsSource::default_directory())),
                LyricsCache::new(LyricsCache::default_directory()),
                Arc::new(LrclibSource::new(None)),
            )));
            svc.focus_assist = Some(Arc::new(FocusAssistService::default()));
            svc.shelf = Some(Arc::new(ShelfService::new(None)));
        }
        // Ön plan izleyici ve ekranlar.
        let tracker = me.services.acquire_window_tracker();
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
        let targets = me.resolve_target_monitors(&monitors.get_monitors());
        me.svc.borrow_mut().monitors = Some((Arc::clone(&monitors), mon_sub));
        for m in targets {
            Inner::create_window(me, m);
        }
        Self::start_osd_services(me);
        super::notch_feature_cards::start(me);
        let ui = UiRef::new(me);
        let fg_sub = tracker.foreground_window_changed().subscribe(move |info| {
            let ui = ui.clone();
            let info = info.clone();
            dispatch::post(move || {
                if let Some(me) = ui.upgrade() {
                    me.on_foreground_window_changed(&info);
                }
            });
        });
        let was_running = tracker.is_started();
        tracker.start();
        let current = if was_running { tracker.current_foreground() } else { None };
        me.svc.borrow_mut().tracker = Some((tracker, fg_sub));
        if let Some(info) = current {
            me.on_foreground_window_changed(&info);
        }
        me.update_edge_timer();
        if !DEVICE_LOCATION_ATTEMPTED.get()
            && koy_core::weather::WeatherLocationPolicy::should_refresh_device_location(&settings, chrono::Utc::now().fixed_offset())
        {
            DEVICE_LOCATION_ATTEMPTED.set(true);
            Self::refresh_device_location(me);
        }
    }

    /// Cihaz konumu (Windows konum servisi) arka planda okunur; başarılıysa ayara yazılır ve hava durumu yenilenir.
    fn refresh_device_location(me: &Rc<Self>) {
        let ui = UiRef::new(me);
        std::thread::spawn(move || {
            let result = koy_core::weather::ILocationService::get_location(&koy_platform::weather::LocationService, &Default::default())
                .unwrap_or_else(|_| koy_core::weather::LocationResult::of(koy_core::weather::LocationStatus::TimedOut));
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                if me.stopped.get() || !result.is_success() {
                    return;
                }
                let store = SettingsStore::new(SettingsStore::default_path());
                let updated = koy_core::weather::WeatherLocationPolicy::apply_device_location(
                    &store.load(),
                    &result,
                    chrono::Utc::now().fixed_offset(),
                );
                let _ = store.save(&updated);
                {
                    let mut s = me.settings.borrow_mut();
                    s.weather.location_source = updated.weather.location_source;
                    s.weather.latitude = updated.weather.latitude;
                    s.weather.longitude = updated.weather.longitude;
                    s.weather.city = updated.weather.city.clone();
                    s.weather.location_updated_unix = updated.weather.location_updated_unix;
                }
                for w in me.windows.borrow().iter() {
                    w.vm.status.on_location_changed();
                }
            });
        });
    }

    fn resolve_target_monitors(&self, monitors: &[DisplayMonitor]) -> Vec<DisplayMonitor> {
        let s = self.settings.borrow();
        MonitorPolicy::notch_monitors(&MonitorPolicy::resolve(Some(&s.displays.monitors), s.displays.new_screen_default, monitors))
    }

    fn create_window(me: &Rc<Self>, monitor: DisplayMonitor) {
        let settings = me.settings.borrow().clone();
        let services = Rc::clone(&me.services);
        let controller = Arc::new(NotchController::new(None));
        controller.set_open_on_hover(settings.notch.open_behavior == NotchOpenBehavior::Hover);
        controller.set_open_delay_ms(settings.notch.open_delay_ms);
        controller.set_close_delay_ms(settings.notch.close_delay_ms);
        controller.set_panel_order(Some(&settings.notch.panel_order));
        controller.set_enabled_panels(&NotchPanels::parse_enabled_with_order(
            Some(&settings.notch.enabled_panels),
            settings.clipboard.history_enabled,
            Some(&settings.notch.panel_order),
        ));
        let visibility = Arc::new(VisibilityController::new(settings.notch.visibility, None));
        visibility.set_hide_in_fullscreen(settings.notch.hide_in_fullscreen);

        let svc = me.svc.borrow();
        let media = svc.media.clone().expect("medya servisi");
        let lyrics = svc.lyrics.clone();
        let s1 = Rc::clone(&services);
        let s2 = Rc::clone(&services);
        let music = MusicViewModel::new(
            media,
            MusicOptions {
                spectrum_acquire: Some(Box::new(move || s1.acquire_spectrum_service())),
                spectrum_release: Some(Box::new(move || s2.release_spectrum_service())),
                lyrics_provider: lyrics.map(|l| l as Arc<dyn koy_core::lyrics::ILyricsProvider>),
                card_style: settings.music.card_style,
                visualizer_style: settings.music.visualizer_style,
                visualizer_color_source: settings.music.visualizer_color_source,
                visualizer_bar_count: settings.music.visualizer_bar_count,
            },
        );
        let sv = Rc::clone(&me.settings);
        music.lyrics.enabled_toggled.subscribe(move |enabled| save_lyrics_enabled(&sv, *enabled));

        let shared_monitor = Arc::new(Mutex::new(monitor.clone()));
        let (r1, r2, w1, w2, b1, b2) =
            (Rc::clone(&services), Rc::clone(&services), Rc::clone(&services), Rc::clone(&services), Rc::clone(&services), Rc::clone(&services));
        let bm = Arc::clone(&shared_monitor);
        let control_center = ControlCenterViewModel::new(
            ControlCenterFactories {
                audio: Box::new(|| Arc::new(AudioService::new())),
                radio: Box::new(move || r1.acquire_radio_service()),
                radio_release: Box::new(move || r2.release_radio_service()),
                brightness: Arc::new(move || {
                    let m = Arc::clone(&bm);
                    Arc::new(BrightnessService::new(
                        false,
                        Some(Box::new(move || Some(m.lock().unwrap_or_else(|e| e.into_inner()).clone()))),
                    ))
                }),
            },
            svc.focus_assist.clone().expect("odak yardımı servisi"),
            Rc::clone(&me.settings),
            move |is_on| WifiPageViewModel::new(Box::new(move || w1.acquire_wifi_service()), Box::new(move || w2.release_wifi_service()), is_on),
            BluetoothDevicesViewModel::new(Box::new(move || b1.acquire_bluetooth_service()), Box::new(move || b2.release_bluetooth_service())),
        );
        let network: Option<Arc<dyn INetworkService>> = svc.network.clone().map(|n| n as Arc<dyn INetworkService>);
        let status = StatusViewModel::new(
            svc.power.as_ref().map(|(p, _)| Arc::clone(p) as Arc<dyn IPowerService>).expect("güç servisi"),
            svc.weather.clone().expect("hava durumu servisi"),
            Rc::clone(&me.settings),
            network.clone(),
            svc.place_names.clone().map(|p| p as Arc<dyn koy_core::weather::IPlaceNameService>),
        );
        let tasks = TasksViewModel::new(services.tasks());
        let focus = FocusViewModel::new(svc.focus_timers.clone().expect("zamanlayıcılar"), &settings, Some(tasks));
        let shelf = ShelfViewModel::new(svc.shelf.clone().expect("raf servisi"));
        let calendar = CalendarViewModel::new(&settings, Rc::clone(&me.shared_events));
        let (st1, st2) = (Rc::clone(&services), Rc::clone(&services));
        let net = network.clone();
        let system = SystemStatsViewModel::new(
            Box::new(move || st1.acquire_system_stats(net.clone())),
            Box::new(move || st2.release_system_stats()),
            settings.general.network_speed_unit,
        );
        let quick_launch = QuickLaunchViewModel::new(services.quick_launch());
        let clipboard = svc.clipboard.clone().map(|c| ClipboardViewModel::new(c as Arc<dyn IClipboardHistoryService>));
        let privacy_state = svc.privacy.as_ref().map(|(p, _)| p.current_state());
        drop(svc);

        let vm = NotchViewModel::new(NotchParts {
            settings: Rc::clone(&me.settings),
            controller: Arc::clone(&controller),
            visibility: Arc::clone(&visibility),
            music,
            control_center,
            status,
            focus,
            shelf,
            calendar,
            system_stats: system,
            quick_launch,
            clipboard,
            shell_actions: Some(Arc::clone(&me.shell_actions) as Arc<dyn koy_core::shell::IShellActionService>),
        });
        if let Some(p) = privacy_state {
            vm.update_privacy_state(&p);
        }
        let Ok(window) = NotchWindow::new() else { return };
        ThemeManager::install(&window);
        let host = NotchWindowHost::new(Rc::clone(&vm), window);
        let binding = NotchWindowBinding::bind(&host, &vm);
        host.set_target_monitor(Some(monitor.clone()));
        {
            let mon = Arc::clone(&me.svc.borrow().monitors.as_ref().map(|(m, _)| Arc::clone(m)).expect("ekran servisi"));
            *host.display_change_handler.borrow_mut() = Some(Box::new(move || mon.notify_display_changed()));
        }
        let ui = Rc::downgrade(me);
        *host.voice_confirm_requested.borrow_mut() = Some(Box::new(move || {
            if let Some(me) = ui.upgrade() {
                super::notch_feature_cards::confirm_voice_command(&me);
            }
        }));
        let ui = Rc::downgrade(me);
        *host.voice_cancel_requested.borrow_mut() = Some(Box::new(move || {
            if let Some(me) = ui.upgrade() {
                super::notch_feature_cards::cancel_voice_command(&me);
            }
        }));
        let ui = Rc::downgrade(me);
        *host.voice_reply_requested.borrow_mut() = Some(Box::new(move || {
            if let Some(me) = ui.upgrade() {
                super::notch_feature_cards::open_voice_reply(&me);
            }
        }));
        let ui = UiRef::new(me);
        let vis = Arc::clone(&visibility);
        let visibility_sub = visibility.changed().subscribe(move |_| {
            let ui = ui.clone();
            let vis = Arc::clone(&vis);
            dispatch::post(move || {
                if let Some(me) = ui.upgrade() {
                    me.on_visibility_changed(&vis);
                }
            });
        });
        me.attach_osd(&vm);
        let line = if settings.notch.edge_line {
            super::notch_line_window::NotchLineWindow::new(monitor.clone(), settings.notch.edge_line_color, settings.notch.edge_line_thickness)
        } else {
            None
        };
        if let Some(l) = &line {
            l.show();
            // Çizgi tam ekran algılamasında yok sayılır.
            let ui = Rc::downgrade(me);
            let lw = Rc::downgrade(l);
            slint::Timer::single_shot(Duration::from_millis(200), move || {
                if let (Some(me), Some(l)) = (ui.upgrade(), lw.upgrade())
                    && l.hwnd() != 0
                {
                    me.fullscreen.ignored_handles().insert(l.hwnd());
                }
            });
        }
        // Yerel tanıtıcı gelince: görünürlük denetleyicisi ve tam ekran algılayıcı onu tanır, sınırlar hesaplanır
        // (C# `SourceInitialized`).
        {
            let ui = Rc::downgrade(me);
            let vis = Arc::clone(&visibility);
            let wh = Rc::downgrade(&host);
            let mon = monitor.clone();
            let top = NotchStyle::top_offset_dip(settings.notch.style);
            *host.native_ready.borrow_mut() = Some(Box::new(move |hwnd| {
                vis.set_notch_handle(hwnd);
                if let Some(me) = ui.upgrade() {
                    me.fullscreen.ignored_handles().insert(hwnd);
                }
                if let Some(h) = wh.upgrade() {
                    update_notch_bounds(&h.window, &vis, &mon, top);
                }
            }));
        }
        host.show();
        let top = NotchStyle::top_offset_dip(settings.notch.style);
        update_notch_bounds(&host.window, &visibility, &monitor, top);
        vm.control_center.start_radio_status();
        me.windows.borrow_mut().push(WindowState {
            monitor: shared_monitor,
            host,
            binding,
            vm,
            visibility,
            controller,
            line,
            visibility_sub: Some(visibility_sub),
        });
    }

    fn start_osd_services(me: &Rc<Self>) {
        let s = me.settings.borrow().notch.clone();
        if s.show_volume_osd {
            let audio = Arc::new(AudioService::new());
            for w in me.windows.borrow().iter() {
                w.vm.osd.attach_audio(Arc::clone(&audio) as Arc<dyn koy_core::audio::IAudioService>);
            }
            me.svc.borrow_mut().osd_audio = Some(audio);
        }
        if s.show_brightness_osd {
            // Parlaklık servisi WMI sorgusuyla kurulur: arka planda.
            let ui = UiRef::new(me);
            std::thread::spawn(move || {
                let service = Arc::new(BrightnessService::new(true, None));
                dispatch::post(move || {
                    let Some(me) = ui.upgrade() else { return };
                    if me.stopped.get() || !koy_core::display::IBrightnessService::is_supported(service.as_ref()) {
                        service.dispose();
                        return;
                    }
                    for w in me.windows.borrow().iter() {
                        w.vm.osd.attach_brightness(Arc::clone(&service) as Arc<dyn koy_core::display::IBrightnessService>);
                    }
                    me.svc.borrow_mut().osd_brightness = Some(service);
                });
            });
        }
    }

    fn attach_osd(&self, vm: &Rc<NotchViewModel>) {
        let svc = self.svc.borrow();
        if let Some(a) = &svc.osd_audio {
            vm.osd.attach_audio(Arc::clone(a) as Arc<dyn koy_core::audio::IAudioService>);
        }
        if let Some(b) = &svc.osd_brightness {
            vm.osd.attach_brightness(Arc::clone(b) as Arc<dyn koy_core::display::IBrightnessService>);
        }
    }

    fn on_monitors_changed(me: &Rc<Self>) {
        let Some(service) = me.svc.borrow().monitors.as_ref().map(|(m, _)| Arc::clone(m)) else { return };
        let targets = me.resolve_target_monitors(&service.get_monitors());
        let keys: Vec<String> = targets.iter().map(|m| m.id.to_lowercase()).collect();
        // Artık hedef olmayan ekranlardaki pencereler kapanır.
        let removed: Vec<WindowState> = {
            let mut windows = me.windows.borrow_mut();
            let (keep, gone): (Vec<_>, Vec<_>) = windows.drain(..).partition(|w| keys.contains(&w.monitor().id.to_lowercase()));
            *windows = keep;
            gone
        };
        for w in removed {
            let hwnd = w.host.hwnd();
            if hwnd != 0 {
                me.fullscreen.ignored_handles().remove(&hwnd);
            }
            if let Some(l) = w.line.as_ref().filter(|l| l.hwnd() != 0) {
                me.fullscreen.ignored_handles().remove(&l.hwnd());
            }
            w.dispose();
        }
        let top = NotchStyle::top_offset_dip(me.settings.borrow().notch.style);
        for m in targets {
            let existing = me.windows.borrow().iter().position(|w| w.monitor().id.eq_ignore_ascii_case(&m.id));
            match existing {
                Some(i) => {
                    let windows = me.windows.borrow();
                    let w = &windows[i];
                    *w.monitor.lock().unwrap_or_else(|e| e.into_inner()) = m.clone();
                    w.host.set_target_monitor(Some(m.clone()));
                    if let Some(l) = &w.line {
                        l.place(m.clone());
                    }
                    w.vm.control_center.refresh_brightness();
                    update_notch_bounds(&w.host.window, &w.visibility, &m, top);
                }
                None => Inner::create_window(me, m),
            }
        }
        me.update_edge_timer();
    }

    fn on_foreground_window_changed(&self, info: &WindowInfo) {
        let hide_in_fullscreen = self.settings.borrow().notch.hide_in_fullscreen;
        for w in self.windows.borrow().iter() {
            let is_fullscreen = self.fullscreen.is_fullscreen_on(w.monitor().handle);
            w.visibility.set_fullscreen(is_fullscreen);
            w.visibility.update_foreground_window(info.handle, info.bounds);
            let hwnd = w.host.hwnd();
            if (!is_fullscreen || !hide_in_fullscreen) && hwnd != 0 && info.handle != hwnd && !FullscreenDetector::is_screen_clipping_host(info.handle) {
                // Önce çizgi, sonra çentik: çentik çizginin üstünde kalır.
                if let Some(l) = &w.line {
                    l.reassert_topmost();
                }
                WindowStyles::reassert_topmost(hwnd);
            }
        }
        self.update_edge_timer();
    }

    fn on_visibility_changed(&self, source: &Arc<VisibilityController>) {
        let hide = self.settings.borrow().notch.hide_in_fullscreen;
        for w in self.windows.borrow().iter() {
            if Arc::ptr_eq(&w.visibility, source)
                && let Some(l) = &w.line
            {
                l.set_line_visible(NotchEdgeLine::is_visible(w.visibility.is_fullscreen(), hide));
            }
        }
        self.update_edge_timer();
    }

    fn update_edge_timer(&self) {
        let windows = self.windows.borrow();
        if windows.is_empty() {
            return;
        }
        let needs_peek = windows.iter().any(|w| {
            let v = &w.visibility;
            (!v.is_visible() || v.is_mouse_at_edge()) && !v.is_fullscreen() && v.mode() != VisibilityMode::Fixed
        });
        drop(windows);
        if needs_peek {
            if !self.edge_timer.running() {
                let me = self.self_weak();
                self.edge_timer.start(slint::TimerMode::Repeated, EDGE_PEEK_INTERVAL, move || {
                    if let Some(me) = me.upgrade() {
                        me.check_edge_peek();
                    }
                });
            }
        } else if self.edge_timer.running() {
            self.edge_timer.stop();
        }
    }

    fn self_weak(&self) -> std::rc::Weak<Self> {
        self.cards.me.borrow().clone()
    }

    /// İmleç ekranın üst kenarında, çentiğin hizasındaysa gizli çentik görünür (kenara dokunarak gösterme).
    fn check_edge_peek(&self) {
        let Some(tracker) = self.svc.borrow().tracker.as_ref().map(|(t, _)| Arc::clone(t)) else { return };
        let Some((x, y)) = tracker.get_cursor_position() else { return };
        let style = self.settings.borrow().notch.style;
        for w in self.windows.borrow().iter() {
            let vc = &w.visibility;
            if vc.is_fullscreen() {
                continue;
            }
            let m = w.monitor();
            if x < m.bounds.left || x > m.bounds.right {
                continue;
            }
            let b = vc.notch_bounds();
            let edge_bottom = m.bounds.top + NotchStyle::edge_trigger_bottom_pixels(style, m.dpi_scale_y);
            let at_edge = y >= m.bounds.top && y <= edge_bottom && (b.is_empty() || (x >= b.left - 30 && x <= b.right + 30));
            if at_edge {
                vc.set_mouse_at_edge(true);
            } else if vc.is_mouse_at_edge() && !vc.is_mouse_over() {
                vc.set_mouse_at_edge(false);
            }
        }
    }

    /// Kısayolun hedefi: imlecin bulunduğu ekrandaki çentik, yoksa ilki.
    pub(super) fn pick_for_shortcut(&self) -> Option<usize> {
        let windows = self.windows.borrow();
        if windows.is_empty() {
            return None;
        }
        let cursor = self.svc.borrow().tracker.as_ref().and_then(|(t, _)| t.get_cursor_position());
        if let Some((x, y)) = cursor
            && let Some(i) = windows.iter().position(|w| {
                let b = w.monitor().bounds;
                x >= b.left && x < b.right && y >= b.top && y < b.bottom
            })
        {
            return Some(i);
        }
        Some(0)
    }

    fn toggle_search(&self, use_clipboard: bool) {
        let Some(i) = self.pick_for_shortcut() else { return };
        let Some(tracker) = self.svc.borrow().tracker.as_ref().map(|(t, _)| Arc::clone(t)) else { return };
        let query = if use_clipboard { crate::infra::clipboard_text::get_text() } else { None };
        let (vm, host, monitor) = {
            let windows = self.windows.borrow();
            (Rc::clone(&windows[i].vm), Rc::clone(&windows[i].host), windows[i].monitor())
        };
        if vm.is_search_open() && query.is_none() {
            vm.close_search(true);
            return;
        }
        if self.fullscreen.is_fullscreen_on(monitor.handle) {
            return;
        }
        let others: Vec<Rc<NotchViewModel>> =
            self.windows.borrow().iter().filter(|w| !Rc::ptr_eq(&w.vm, &vm)).map(|w| Rc::clone(&w.vm)).collect();
        for o in others {
            o.close_search(false);
        }
        if vm.is_search_open() {
            vm.close_search(false);
        }
        let previous = tracker.current_foreground().map_or(0, |f| f.handle);
        vm.open_search(|| self.create_search(), previous, query.as_deref());
        host.focus_search();
    }

    fn create_search(&self) -> Rc<SearchViewModel> {
        let apps = {
            let mut svc = self.svc.borrow_mut();
            Arc::clone(svc.app_search.get_or_insert_with(|| {
                search_factory::create_app_source(Arc::clone(&self.shell_actions) as Arc<dyn koy_core::shell::IShellActionService>)
            }))
        };
        let svc = self.svc.borrow();
        let tracker = svc.tracker.as_ref().map(|(t, _)| Arc::clone(t) as Arc<dyn IWindowTracker>).expect("pencere izleyici");
        let clipboard = svc.clipboard.clone().map(|c| c as Arc<dyn IClipboardHistoryService>);
        drop(svc);
        let sv = Rc::clone(&self.settings);
        let settings = self.settings.borrow().clone();
        search_factory::create(&settings, apps, tracker, clipboard, Box::new(move |pinned| save_search_pinned(&sv, pinned)))
    }

    fn on_power_changed(&self, e: koy_core::power::PowerChangedEventArgs) {
        let alert = self.battery_policy.borrow_mut().update(e);
        if alert.kind == BatteryAlertKind::None {
            return;
        }
        for w in self.windows.borrow().iter() {
            w.vm.show_battery(alert.kind, alert.percent);
        }
    }

    fn on_bluetooth_devices_changed(&self) {
        let Some(bt) = self.svc.borrow().bluetooth.as_ref().map(|(b, _)| Arc::clone(b)) else { return };
        let events = self.bluetooth_policy.borrow_mut().update(&bt.devices());
        for e in events {
            for w in self.windows.borrow().iter() {
                w.vm.show_bluetooth_device(&e.name, e.connected, e.battery_percent);
            }
        }
    }

    fn stop(&self) {
        self.stopped.set(true);
        super::notch_feature_cards::stop(self);
        self.edge_timer.stop();
        let mut svc = std::mem::take(&mut *self.svc.borrow_mut());
        if let Some((m, sub)) = svc.monitors.take() {
            m.monitors_changed().unsubscribe(sub);
            m.dispose();
        }
        if let Some((t, sub)) = svc.tracker.take() {
            t.foreground_window_changed().unsubscribe(sub);
            self.services.release_window_tracker();
        }
        let windows: Vec<WindowState> = self.windows.borrow_mut().drain(..).collect();
        for w in windows {
            w.dispose();
        }
        svc.osd_audio.take();
        if let Some(b) = svc.osd_brightness.take() {
            b.dispose();
        }
        if svc.media.take().is_some() {
            self.services.release_media_service();
        }
        if let Some((p, sub)) = svc.power.take() {
            p.power_changed().unsubscribe(sub);
            self.services.release_power_service();
        }
        if let Some((b, sub)) = svc.bluetooth.take() {
            b.devices_changed().unsubscribe(sub);
            self.services.release_bluetooth_service();
        }
        if svc.network.take().is_some() {
            self.services.release_network_service();
        }
        if let Some(f) = svc.focus_assist.take() {
            f.dispose();
        }
        svc.weather.take();
        svc.shelf.take();
        svc.focus_timers.take();
        if svc.clipboard.take().is_some() {
            self.services.release_clipboard_history();
        }
        if let Some((p, sub)) = svc.privacy.take() {
            p.state_changed().unsubscribe(sub);
            self.services.release_privacy_monitor();
        }
    }
}

/// Görünürlük denetleyicisine çentiğin ekrandaki dikdörtgenini (açık genişlik, 140 DIP yükseklik) bildirir.
fn update_notch_bounds(window: &NotchWindow, visibility: &VisibilityController, monitor: &DisplayMonitor, top_offset_dip: f64) {
    let scale = window.window().scale_factor() as f64;
    let (dpi_x, dpi_y) = if scale > 0.0 { (scale, scale) } else { (monitor.dpi_scale_x, monitor.dpi_scale_y) };
    let b = monitor.bounds;
    let notch_width = NotchLayout::EXPANDED_WIDTH * dpi_x;
    let notch_height = 140.0 * dpi_y;
    let top = WindowPlacementMath::to_pixels(top_offset_dip, monitor.dpi_scale_y);
    let width = (b.right - b.left) as f64;
    let left = (b.left as f64 + (width - notch_width) / 2.0) as i32;
    let right = (left as f64 + notch_width) as i32;
    visibility.set_notch_bounds(WindowRect { left, top: b.top + top, right, bottom: (b.top as f64 + top as f64 + notch_height) as i32 });
}

fn save_lyrics_enabled(settings: &SharedSettings, enabled: bool) {
    settings.borrow_mut().music.lyrics_enabled = enabled;
    let store = SettingsStore::new(SettingsStore::default_path());
    let mut current = store.load();
    current.music.lyrics_enabled = enabled;
    let _ = store.save(&current);
}

fn save_search_pinned(settings: &SharedSettings, pinned: bool) {
    settings.borrow_mut().search.pinned = pinned;
    let store = SettingsStore::new(SettingsStore::default_path());
    let mut current = store.load();
    current.search.pinned = pinned;
    let _ = store.save(&current);
}

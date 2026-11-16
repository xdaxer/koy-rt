//! Ekran köşesi eylemleri. Yoklama yok: imleç köşeye girince fare kancası (`CornerHookService`) haber verir, bekleme
//! süresi için yalnız o an tek seferlik bir zamanlayıcı kurulur; köşeden çıkınca durur. Kanca yalnız en az bir köşeye
//! eylem bağlıyken kurulur. Kararlar Core'dadır (`HotCornerTracker`, `HotCornerMath`); burası yalnız zaman ve Win32 okur.
//! Tam ekran uygulama öndeyken ve fare tuşu basılıyken (sürükleme) hiçbir şey tetiklenmez.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use koy_core::display::{DisplayMonitor, IMonitorService};
use koy_core::input::{CornerActions, CornerHit, HotCornerTracker, ICornerHookService, InputAction, ScreenCorner, ScreenRect};
use koy_core::settings::HotCornersSettings;
use koy_core::shell::IFullscreenDetector;
use koy_platform::display::MonitorService;
use koy_platform::shell::{CornerHookService, CursorHelper, FullscreenDetector};
use windows::Win32::System::SystemInformation::GetTickCount64;

use crate::infra::dispatch;
use crate::infra::system_events::{Subscription, SystemEvents};
use crate::infra::ui_ref::UiRef;

// Zamanlayıcı ile TickCount64 çözünürlüğü farkı yüzünden bekleme bir tık eksik ölçülmesin.
const DWELL_MARGIN: Duration = Duration::from_millis(20);
const RETRY_DELAY: Duration = Duration::from_millis(50);

fn tick_ms() -> i64 {
    // SAFETY: yalnız sayaç okuma (C# Environment.TickCount64).
    unsafe { GetTickCount64() as i64 }
}

struct State {
    tracker: HotCornerTracker,
    hook: Option<Arc<CornerHookService>>,
    monitors: Option<MonitorService>,
    display_sub: Option<Subscription>,
    dwell: Option<slint::Timer>,
    settings: HotCornersSettings,
    monitor_list: Vec<DisplayMonitor>,
    hit: Option<CornerHit>,
}

struct Inner {
    run: Rc<dyn Fn(InputAction)>,
    fullscreen: FullscreenDetector,
    state: RefCell<State>,
}

pub struct HotCornerController {
    inner: Rc<Inner>,
}

impl HotCornerController {
    pub fn new(run: impl Fn(InputAction) + 'static) -> Self {
        HotCornerController {
            inner: Rc::new(Inner {
                run: Rc::new(run),
                fullscreen: FullscreenDetector::default(),
                state: RefCell::new(State {
                    tracker: HotCornerTracker::new(),
                    hook: None,
                    monitors: None,
                    display_sub: None,
                    dwell: None,
                    settings: HotCornersSettings::default(),
                    monitor_list: Vec::new(),
                    hit: None,
                }),
            }),
        }
    }

    /// Kanca kurulu mu (tanılama için).
    pub fn is_running(&self) -> bool {
        self.inner.state.borrow().hook.is_some()
    }

    /// Ayarı uygular: gerekiyorsa kancayı kurar, gerekmiyorsa kaldırır.
    pub fn apply(&self, settings: &HotCornersSettings) {
        {
            let mut st = self.inner.state.borrow_mut();
            st.settings = settings.clone();
            st.dwell = None;
            st.tracker.reset();
        }
        if CornerActions::is_active(settings) {
            start_tracking(&self.inner);
        } else {
            stop_tracking(&self.inner);
        }
    }

    pub fn dispose(&self) {
        stop_tracking(&self.inner);
    }
}

impl Drop for HotCornerController {
    fn drop(&mut self) {
        self.dispose();
    }
}

fn start_tracking(inner: &Rc<Inner>) {
    if inner.state.borrow().hook.is_some() {
        return;
    }
    let me = UiRef::new(inner);
    let display_sub = SystemEvents::on_display_settings_changed(move || {
        if let Some(inner) = me.upgrade() {
            dispatch::guard("Ekran köşesi", || refresh_screens(&inner));
        }
    });
    // Kanca olayları `post` ile arayüz iş parçacığında yükselir; jeton orada çözülür.
    let hook = Arc::new(CornerHookService::new(dispatch::post_fn()));
    hook.corner_changed().subscribe(move |hit: &Option<CornerHit>| {
        if let Some(inner) = me.upgrade() {
            on_corner_changed(&inner, *hit);
        }
    });
    {
        let mut st = inner.state.borrow_mut();
        st.monitors.get_or_insert_with(MonitorService::default);
        st.display_sub = Some(display_sub);
        st.hook = Some(hook);
    }
    refresh_screens(inner);
}

fn stop_tracking(inner: &Rc<Inner>) {
    let hook = {
        let mut st = inner.state.borrow_mut();
        st.dwell = None;
        st.display_sub = None;
        st.tracker.reset();
        st.hit = None;
        st.monitor_list.clear();
        st.hook.take()
    };
    if let Some(h) = hook {
        h.dispose();
    }
}

fn refresh_screens(inner: &Rc<Inner>) {
    let (hook, monitors) = {
        let st = inner.state.borrow();
        let (Some(hook), Some(m)) = (st.hook.clone(), st.monitors.as_ref()) else { return };
        (hook, m.get_monitors())
    };
    let rects: Vec<ScreenRect> =
        monitors.iter().map(|m| ScreenRect { left: m.bounds.left, top: m.bounds.top, width: m.bounds.width(), height: m.bounds.height() }).collect();
    inner.state.borrow_mut().monitor_list = monitors;
    hook.set_screens(&rects, CornerActions::CORNER_SIZE_PX);
}

fn action_at(settings: &HotCornersSettings, corner: ScreenCorner) -> Option<InputAction> {
    CornerActions::to_input_action(CornerActions::action_for(settings, corner))
}

fn start_dwell(inner: &Rc<Inner>, delay: Duration) {
    let weak = Rc::downgrade(inner);
    let timer = dispatch::delay(delay, move || {
        if let Some(inner) = weak.upgrade() {
            dispatch::guard("Ekran köşesi", || on_dwell_elapsed(&inner));
        }
    });
    inner.state.borrow_mut().dwell = Some(timer);
}

fn dwell_delay(dwell_ms: i32) -> Duration {
    Duration::from_millis(CornerActions::clamp_dwell(dwell_ms) as u64) + DWELL_MARGIN
}

fn on_corner_changed(inner: &Rc<Inner>, hit: Option<CornerHit>) {
    dispatch::guard("Ekran köşesi", || {
        let dwell_ms = {
            let mut st = inner.state.borrow_mut();
            st.dwell = None;
            st.tracker.reset();
            st.hit = hit;
            // Eylemi olmayan köşe "köşe değil" sayılır.
            let Some(h) = hit else { return };
            if action_at(&st.settings, h.corner).is_none() {
                return;
            }
            let dwell = st.settings.dwell_ms;
            st.tracker.update(Some(h.corner), false, tick_ms(), dwell);
            dwell
        };
        start_dwell(inner, dwell_delay(dwell_ms));
    });
}

fn on_dwell_elapsed(inner: &Rc<Inner>) {
    let (h, settings, monitor) = {
        let mut st = inner.state.borrow_mut();
        st.dwell = None;
        let Some(h) = st.hit else { return };
        if action_at(&st.settings, h.corner).is_none() {
            return;
        }
        (h, st.settings.clone(), st.monitor_list.get(h.screen_index as usize).map(|m| m.handle))
    };
    // Tam ekran öndeyse bu girişte hiçbir şey yapılmaz; köşeden çıkıp yeniden girmek gerekir.
    if settings.disable_in_fullscreen
        && let Some(handle) = monitor
        && inner.fullscreen.is_fullscreen_on(handle)
    {
        return;
    }
    // Sürükleme köşede bitebilir: tuş bırakılınca bekleme baştan sayılır. Yalnız imleç köşedeyken yeniden kurulur.
    if CursorHelper::is_any_mouse_button_down() {
        {
            let mut st = inner.state.borrow_mut();
            st.tracker.reset();
            st.tracker.update(Some(h.corner), false, tick_ms(), settings.dwell_ms);
        }
        start_dwell(inner, dwell_delay(settings.dwell_ms));
        return;
    }
    let fired = inner.state.borrow_mut().tracker.update(Some(h.corner), false, tick_ms(), settings.dwell_ms);
    match fired {
        Some(corner) => {
            if let Some(to_run) = action_at(&settings, corner) {
                // Ödünç tutulmadan çağrılır: eylem denetleyiciyi yeniden yapılandırabilir.
                let run = Rc::clone(&inner.run);
                run(to_run);
            }
        }
        None => start_dwell(inner, RETRY_DELAY),
    }
}

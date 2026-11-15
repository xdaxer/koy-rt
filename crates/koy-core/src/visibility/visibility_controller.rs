use std::sync::{Arc, Mutex, MutexGuard, Weak};

use super::visibility_mode::VisibilityMode;
use super::visibility_result::VisibilityResult;
use crate::event::Event;
use crate::shell::WindowRect;
use crate::time::{SystemTimeProvider, TimeProvider, TimeSpan, Timer};

/// Medya "göz atma" süresi: bu kadar sonra çentik yeniden moda göre karar verir.
const MEDIA_PEEK_DURATION: TimeSpan = TimeSpan::from_secs(3);

struct State {
    mode: VisibilityMode,
    notch_bounds: WindowRect,
    notch_handle: isize,
    hide_in_fullscreen: bool,
    is_fullscreen: bool,
    is_window_overlapping: bool,
    is_mouse_at_edge: bool,
    is_mouse_over: bool,
    is_expanded: bool,
    is_held: bool,
    is_visible: bool,
    is_media_peek_active: bool,
}

/// Zamanlayıcı iş parçacığıyla paylaşılan çekirdek (`Arc` + `Weak` kalıbı, PORTING.md #10).
struct Shared {
    state: Mutex<State>,
    changed: Event<VisibilityResult>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    /// Durum değiştirir; `f` true dönerse (değer gerçekten değişti) yeniden değerlendirir.
    fn update(&self, f: impl FnOnce(&mut State) -> bool) {
        let changed = f(&mut self.lock());
        if changed {
            self.evaluate(false);
        }
    }

    fn on_media_peek_timeout(&self) {
        self.lock().is_media_peek_active = false;
        self.evaluate(false);
    }

    fn evaluate(&self, force_notify: bool) {
        // Olay kilit dışında tetiklenir: işleyici denetleyiciyi yeniden sorgulayabilir.
        let result = {
            let mut s = self.lock();
            let next_visible = Self::compute_visibility(&s);
            if next_visible == s.is_visible && !force_notify {
                return;
            }

            s.is_visible = next_visible;
            VisibilityResult::new(s.is_visible, !s.is_visible)
        };
        self.changed.invoke(&result);
    }

    fn compute_visibility(s: &State) -> bool {
        // 1. Tam ekran uygulamada gizli (oyun ve video izleme konforu); kullanıcı kapatabilir.
        if s.is_fullscreen && s.hide_in_fullscreen {
            return false;
        }

        // 2. Fare çentiğin üstünde, kenarda veya çentik genişletilmiş durumdaysa görünür.
        if s.is_mouse_over || s.is_mouse_at_edge || s.is_expanded || s.is_held {
            return true;
        }

        // 3. Görünürlük moduna göre karar.
        match s.mode {
            VisibilityMode::Fixed => true,
            VisibilityMode::Smart => !s.is_window_overlapping || s.is_media_peek_active,
            VisibilityMode::Peek => s.is_media_peek_active,
        }
    }
}

/// Görünürlük durum makinesi. Çentik ve dock'un hangi koşullarda ekranda görüneceğini,
/// ne zaman gizleneceğini ve tıklama geçirgenlik durumunu belirler.
/// Tamamen saf mantıktır; Win32/WPF bağımlılığı içermez.
pub struct VisibilityController {
    shared: Arc<Shared>,
    time_provider: Arc<dyn TimeProvider>,
    media_peek_timer: Mutex<Option<Box<dyn Timer>>>,
    disposed: Mutex<bool>,
}

impl VisibilityController {
    /// `initialMode` varsayılanı `Fixed`, `timeProvider` varsayılanı `TimeProvider.System`.
    pub fn new(initial_mode: VisibilityMode, time_provider: Option<Arc<dyn TimeProvider>>) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                mode: initial_mode,
                notch_bounds: WindowRect::EMPTY,
                notch_handle: 0,
                hide_in_fullscreen: true,
                is_fullscreen: false,
                is_window_overlapping: false,
                is_mouse_at_edge: false,
                is_mouse_over: false,
                is_expanded: false,
                is_held: false,
                is_visible: false,
                is_media_peek_active: false,
            }),
            changed: Event::new(),
        });
        let controller = VisibilityController {
            shared,
            time_provider: time_provider.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
            media_peek_timer: Mutex::new(None),
            disposed: Mutex::new(false),
        };
        controller.shared.evaluate(true);
        controller
    }

    pub fn changed(&self) -> &Event<VisibilityResult> {
        &self.shared.changed
    }

    pub fn mode(&self) -> VisibilityMode {
        self.shared.lock().mode
    }

    pub fn set_mode(&self, value: VisibilityMode) {
        self.shared.update(|s| {
            if s.mode == value {
                return false;
            }
            s.mode = value;
            true
        });
    }

    pub fn notch_bounds(&self) -> WindowRect {
        self.shared.lock().notch_bounds
    }

    pub fn set_notch_bounds(&self, value: WindowRect) {
        self.shared.update(|s| {
            if s.notch_bounds == value {
                return false;
            }
            s.notch_bounds = value;
            true
        });
    }

    pub fn notch_handle(&self) -> isize {
        self.shared.lock().notch_handle
    }

    pub fn set_notch_handle(&self, value: isize) {
        self.shared.lock().notch_handle = value;
    }

    /// Tam ekran uygulamada gizlen (varsayılan). Kapalıysa tam ekranda da moda göre görünür kalır.
    pub fn hide_in_fullscreen(&self) -> bool {
        self.shared.lock().hide_in_fullscreen
    }

    pub fn set_hide_in_fullscreen(&self, value: bool) {
        self.shared.update(|s| {
            if s.hide_in_fullscreen == value {
                return false;
            }
            s.hide_in_fullscreen = value;
            true
        });
    }

    pub fn is_fullscreen(&self) -> bool {
        self.shared.lock().is_fullscreen
    }
    pub fn is_window_overlapping(&self) -> bool {
        self.shared.lock().is_window_overlapping
    }
    pub fn is_mouse_at_edge(&self) -> bool {
        self.shared.lock().is_mouse_at_edge
    }
    pub fn is_mouse_over(&self) -> bool {
        self.shared.lock().is_mouse_over
    }
    pub fn is_expanded(&self) -> bool {
        self.shared.lock().is_expanded
    }
    pub fn is_held(&self) -> bool {
        self.shared.lock().is_held
    }
    pub fn is_visible(&self) -> bool {
        self.shared.lock().is_visible
    }
    pub fn is_click_through(&self) -> bool {
        !self.is_visible()
    }
    pub fn is_media_peek_active(&self) -> bool {
        self.shared.lock().is_media_peek_active
    }

    pub fn set_fullscreen(&self, is_fullscreen: bool) {
        self.shared.update(|s| {
            if s.is_fullscreen == is_fullscreen {
                return false;
            }
            s.is_fullscreen = is_fullscreen;
            true
        });
    }

    pub fn set_window_overlapping(&self, is_overlapping: bool) {
        self.shared.update(|s| {
            if s.is_window_overlapping == is_overlapping {
                return false;
            }
            s.is_window_overlapping = is_overlapping;
            true
        });
    }

    pub fn update_foreground_window(&self, handle: isize, window_bounds: WindowRect) {
        let (notch_handle, notch_bounds) = {
            let s = self.shared.lock();
            (s.notch_handle, s.notch_bounds)
        };

        if handle == 0 || handle == notch_handle || window_bounds.is_empty() {
            self.set_window_overlapping(false);
            return;
        }

        let overlaps = !notch_bounds.is_empty() && notch_bounds.intersects(window_bounds);
        self.set_window_overlapping(overlaps);
    }

    pub fn set_mouse_at_edge(&self, at_edge: bool) {
        self.shared.update(|s| {
            if s.is_mouse_at_edge == at_edge {
                return false;
            }
            s.is_mouse_at_edge = at_edge;
            true
        });
    }

    pub fn set_mouse_over(&self, mouse_over: bool) {
        self.shared.update(|s| {
            if s.is_mouse_over == mouse_over {
                return false;
            }
            s.is_mouse_over = mouse_over;
            true
        });
    }

    pub fn set_expanded(&self, is_expanded: bool) {
        self.shared.update(|s| {
            if s.is_expanded == is_expanded {
                return false;
            }
            s.is_expanded = is_expanded;
            true
        });
    }

    /// Çentiği görünür tutar (ör. düğmeli kart süresince: Akıllı/Fareyle modunda 3 sn sonra gizlenmesin).
    /// Zamanlayıcı yok; sahibi kart kapanınca false verir. Tam ekran gizlemesini aşmaz.
    pub fn set_hold(&self, hold: bool) {
        self.shared.update(|s| {
            if s.is_held == hold {
                return false;
            }
            s.is_held = hold;
            true
        });
    }

    pub fn on_media_activity(&self) {
        self.shared.lock().is_media_peek_active = true;

        {
            let mut timer = self.media_peek_timer.lock().unwrap();
            // Eski zamanlayıcı bırakılınca (Drop) durur: `_mediaPeekTimer?.Dispose()`.
            *timer = None;
            let weak: Weak<Shared> = Arc::downgrade(&self.shared);
            *timer = Some(self.time_provider.create_timer(
                Box::new(move || {
                    if let Some(shared) = weak.upgrade() {
                        shared.on_media_peek_timeout();
                    }
                }),
                MEDIA_PEEK_DURATION,
                TimeSpan::INFINITE,
            ));
        }

        self.shared.evaluate(false);
    }

    pub fn on_media_peek_timeout(&self) {
        self.shared.on_media_peek_timeout();
    }

    pub fn dispose(&self) {
        let mut disposed = self.disposed.lock().unwrap();
        if *disposed {
            return;
        }

        *disposed = true;
        *self.media_peek_timer.lock().unwrap() = None;
    }
}

impl Drop for VisibilityController {
    fn drop(&mut self) {
        self.dispose();
    }
}

//! Köşe takibi için WH_MOUSE_LL kancası. `KeyboardHookService` gibi kendi iş parçacığında çalışır. Hiçbir olayı yutmaz;
//! her harekette yalnız köşe kutularıyla karşılaştırır, arayüze yalnız köşe değişince haber verir. İmleç durunca hiç
//! uyanmaz. Yalnız en az bir köşeye eylem bağlıyken kurulur.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::event::Event;
use koy_core::input::{CornerHit, HotCornerMath, ICornerHookService, ScreenRect};
use koy_core::search::PostFn;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{MSLLHOOKSTRUCT, WH_MOUSE_LL};

use super::hook_thread::{HookThread, call_next};

const WM_MOUSEMOVE: u32 = 0x0200;

struct Shared {
    post: PostFn,
    screens: RwLock<(Vec<ScreenRect>, i32)>,
    corner_changed: Event<Option<CornerHit>>,
}

thread_local! {
    static SHARED: RefCell<Option<Arc<Shared>>> = const { RefCell::new(None) };
    // Yalnız kanca iş parçacığı okur/yazar.
    static LAST: RefCell<Option<CornerHit>> = const { RefCell::new(None) };
}

unsafe extern "system" fn callback(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam.0 as u32 == WM_MOUSEMOVE {
        let shared = SHARED.with(|s| s.borrow().clone());
        if let Some(shared) = shared {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // SAFETY: WH_MOUSE_LL için lparam bir MSLLHOOKSTRUCT gösterir.
                let pt = unsafe { (*(lparam.0 as *const MSLLHOOKSTRUCT)).pt };
                let hit = {
                    let guard = shared.screens.read().unwrap_or_else(|e| e.into_inner());
                    HotCornerMath::hit_test_screens(pt.x, pt.y, &guard.0, guard.1)
                };
                let changed = LAST.with(|l| {
                    let mut l = l.borrow_mut();
                    if *l != hit {
                        *l = hit;
                        true
                    } else {
                        false
                    }
                });
                if changed {
                    let s = Arc::clone(&shared);
                    (shared.post)(Box::new(move || s.corner_changed.invoke(&hit)));
                }
            }));
            if result.is_err() {
                CrashLog::write("CornerHookService.Callback", &CrashError::exception("Panic", "callback panicked", ""));
            }
        }
    }
    call_next(code, wparam, lparam)
}

struct State {
    thread: Option<HookThread>,
    installed: bool,
    disposed: bool,
}

pub struct CornerHookService {
    shared: Arc<Shared>,
    state: Mutex<State>,
}

impl CornerHookService {
    /// `post`: olayları arayüz iş parçacığına taşır; kanca içinde beklememesi gerekir.
    pub fn new(post: PostFn) -> Self {
        CornerHookService {
            shared: Arc::new(Shared { post, screens: RwLock::new((Vec::new(), 0)), corner_changed: Event::new() }),
            state: Mutex::new(State { thread: None, installed: false, disposed: false }),
        }
    }

    fn stop_hook(st: &mut State) {
        if let Some(t) = st.thread.take() {
            t.stop(Duration::from_secs(2));
            st.installed = false;
        }
    }
}

impl Drop for CornerHookService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl ICornerHookService for CornerHookService {
    fn corner_changed(&self) -> &Event<Option<CornerHit>> {
        &self.shared.corner_changed
    }

    fn set_screens(&self, screens: &[ScreenRect], corner_size_px: i32) -> bool {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.disposed {
            return false;
        }
        *self.shared.screens.write().unwrap_or_else(|e| e.into_inner()) = (screens.to_vec(), corner_size_px);
        if screens.is_empty() {
            Self::stop_hook(&mut st);
            return true;
        }
        if st.thread.is_some() {
            return st.installed;
        }
        let shared = Arc::clone(&self.shared);
        let thread = HookThread::spawn("Koy kose kancasi", WH_MOUSE_LL, callback, "CornerHookService", "CornerHookService.Run", move || {
            LAST.with(|l| *l.borrow_mut() = None);
            SHARED.with(|s| *s.borrow_mut() = Some(shared));
        });
        st.installed = thread.wait_ready(Duration::from_secs(5));
        st.thread = Some(thread);
        st.installed
    }

    fn dispose(&self) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.disposed {
            return;
        }
        st.disposed = true;
        self.shared.corner_changed.clear();
        Self::stop_hook(&mut st);
    }
}

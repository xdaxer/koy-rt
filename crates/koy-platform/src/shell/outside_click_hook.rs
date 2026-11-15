//! WS_EX_NOACTIVATE pencerelerinde açılan bağlam menülerini dışarı tıklandığında kapatmak için yalnız menü açıkken
//! kurulan kısa ömürlü WH_MOUSE_LL kancası. Menü kapanınca hemen sökülür.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use koy_core::diagnostics::{CrashError, CrashLog};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{MSLLHOOKSTRUCT, WH_MOUSE_LL};

use super::hook_thread::{HookThread, call_next};

const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_MBUTTONDOWN: u32 = 0x0207;

pub type MouseDownFn = Arc<dyn Fn(i32, i32) + Send + Sync>;

thread_local! {
    static CALLBACK: RefCell<Option<MouseDownFn>> = const { RefCell::new(None) };
}

unsafe extern "system" fn callback(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && matches!(wparam.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
        // SAFETY: WH_MOUSE_LL için lparam bir MSLLHOOKSTRUCT gösterir.
        let pt = unsafe { (*(lparam.0 as *const MSLLHOOKSTRUCT)).pt };
        let f = CALLBACK.with(|c| c.borrow().clone());
        if let Some(f) = f
            && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(pt.x, pt.y))).is_err()
        {
            CrashLog::write("OutsideClickHook.Callback", &CrashError::exception("Panic", "callback panicked", ""));
        }
    }
    call_next(code, wparam, lparam)
}

struct State {
    thread: Option<HookThread>,
    disposed: bool,
}

pub struct OutsideClickHook {
    on_mouse_down: MouseDownFn,
    state: Mutex<State>,
}

impl OutsideClickHook {
    pub fn new(on_mouse_down: MouseDownFn) -> Self {
        OutsideClickHook { on_mouse_down, state: Mutex::new(State { thread: None, disposed: false }) }
    }

    pub fn start(&self) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.disposed || st.thread.is_some() {
            return;
        }
        let f = Arc::clone(&self.on_mouse_down);
        let thread = HookThread::spawn("Koy dis tiklama kancasi", WH_MOUSE_LL, callback, "OutsideClickHook", "OutsideClickHook.Run", move || {
            CALLBACK.with(|c| *c.borrow_mut() = Some(f));
        });
        thread.wait_ready(Duration::from_secs(1));
        st.thread = Some(thread);
    }

    pub fn stop(&self) {
        let thread = self.state.lock().unwrap_or_else(|e| e.into_inner()).thread.take();
        if let Some(t) = thread {
            t.stop(Duration::from_secs(1));
        }
    }

    pub fn dispose(&self) {
        {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.disposed {
                return;
            }
            st.disposed = true;
        }
        self.stop();
    }
}

impl Drop for OutsideClickHook {
    fn drop(&mut self) {
        self.dispose();
    }
}

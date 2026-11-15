//! WH_KEYBOARD_LL kancası. Kendi iş parçacığında çalışır: arayüz takılsa bile bütün sistemin klavyesi gecikmesin,
//! Windows da yavaş kancayı sessizce kaldırmasın. Karar `KeyboardHookEngine`'de, burada yalnız Windows bağlantısı var.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::event::Event;
use koy_core::search::PostFn;
use koy_core::shortcuts::{Hotkey, HookOutcome, IKeyboardHookService, KeyboardHookEngine};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY};
use windows::Win32::UI::WindowsAndMessaging::{KBDLLHOOKSTRUCT, LLKHF_INJECTED, WH_KEYBOARD_LL};

use super::hook_thread::{HookThread, call_next};

const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
// Atanmamış tuş: Win ya da Alt "tek başına basıldı" sayılmasın diye araya girer, hiçbir uygulamada iş yapmaz.
const MASK_KEY: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

type CaptureFn = Arc<dyn Fn(Hotkey) + Send + Sync>;

struct Shared {
    engine: KeyboardHookEngine,
    post: PostFn,
    on_captured: Mutex<Option<CaptureFn>>,
    hotkey_pressed: Event<i32>,
}

thread_local! {
    static SHARED: RefCell<Option<Arc<Shared>>> = const { RefCell::new(None) };
}

fn now_ms() -> i64 {
    // SAFETY: yalnız sayaç okuma (C# Environment.TickCount64).
    unsafe { GetTickCount64() as i64 }
}

fn is_physically_down(vk: i32) -> bool {
    // SAFETY: yalnız tuş durumu sorgusu.
    (unsafe { GetAsyncKeyState(vk) }) as u16 & 0x8000 != 0
}

fn send_mask_key() {
    let mut inputs = [INPUT::default(), INPUT::default()];
    inputs[0].r#type = INPUT_KEYBOARD;
    inputs[0].Anonymous.ki.wVk = MASK_KEY;
    inputs[1].r#type = INPUT_KEYBOARD;
    inputs[1].Anonymous.ki.wVk = MASK_KEY;
    inputs[1].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    // SAFETY: iki girdilik yerel dizi.
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

fn publish(shared: &Arc<Shared>, outcome: &HookOutcome) {
    if let Some(id) = outcome.fired_id {
        let s = Arc::clone(shared);
        (shared.post)(Box::new(move || s.hotkey_pressed.invoke(&id)));
    }
    if let Some(hotkey) = outcome.captured {
        let callback = shared.on_captured.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(cb) = callback {
            (shared.post)(Box::new(move || cb(hotkey)));
        }
    }
}

unsafe extern "system" fn callback(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return call_next(code, wparam, lparam);
    }
    let shared = SHARED.with(|s| s.borrow().clone());
    if let Some(shared) = shared {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: WH_KEYBOARD_LL için lparam bir KBDLLHOOKSTRUCT gösterir.
            let info = unsafe { *(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let message = wparam.0 as u32;
            let is_down = matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN);
            let known = is_down || matches!(message, WM_KEYUP | WM_SYSKEYUP);
            // Koy'un kendi gönderdiği tuşlar (Win+S, boş tuş) yeniden yorumlanmaz.
            if known && (info.flags.0 & LLKHF_INJECTED.0) == 0 {
                let outcome = shared.engine.process(info.vkCode as i32, is_down, now_ms(), Some(&is_physically_down));
                publish(&shared, &outcome);
                if outcome.mask_modifier {
                    send_mask_key();
                }
                return outcome.swallow;
            }
            false
        }));
        match result {
            Ok(true) => return LRESULT(1),
            Ok(false) => {}
            Err(_) => CrashLog::write("KeyboardHookService.Callback", &CrashError::exception("Panic", "callback panicked", "")),
        }
    }
    call_next(code, wparam, lparam)
}

struct State {
    thread: Option<HookThread>,
    installed: bool,
    has_bindings: bool,
    capturing: bool,
    disposed: bool,
}

pub struct KeyboardHookService {
    shared: Arc<Shared>,
    state: Mutex<State>,
}

impl KeyboardHookService {
    /// `post`: olayları arayüz iş parçacığına taşır; kanca içinde beklememesi gerekir.
    pub fn new(post: PostFn) -> Self {
        KeyboardHookService {
            shared: Arc::new(Shared { engine: KeyboardHookEngine::new(), post, on_captured: Mutex::new(None), hotkey_pressed: Event::new() }),
            state: Mutex::new(State { thread: None, installed: false, has_bindings: false, capturing: false, disposed: false }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ensure_running(&self, st: &mut State) -> bool {
        if st.thread.is_some() {
            return st.installed;
        }
        let shared = Arc::clone(&self.shared);
        let thread = HookThread::spawn("Koy klavye kancasi", WH_KEYBOARD_LL, callback, "KeyboardHookService", "KeyboardHookService.Run", move || {
            SHARED.with(|s| *s.borrow_mut() = Some(shared));
        });
        st.installed = thread.wait_ready(Duration::from_secs(5));
        st.thread = Some(thread);
        st.installed
    }

    fn stop_if_idle(st: &mut State) -> bool {
        if st.has_bindings || st.capturing {
            return true;
        }
        if let Some(t) = st.thread.take() {
            t.stop(Duration::from_secs(2));
            st.installed = false;
        }
        true
    }
}

impl Drop for KeyboardHookService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IKeyboardHookService for KeyboardHookService {
    fn hotkey_pressed(&self) -> &Event<i32> {
        &self.shared.hotkey_pressed
    }

    fn set_bindings(&self, bindings: &HashMap<i32, Hotkey>) -> bool {
        let mut st = self.lock();
        if st.disposed {
            return false;
        }
        self.shared.engine.set_bindings(bindings);
        st.has_bindings = !bindings.is_empty();
        if st.has_bindings { self.ensure_running(&mut st) } else { Self::stop_if_idle(&mut st) }
    }

    fn begin_capture(&self, on_captured: Box<dyn Fn(Hotkey) + Send + Sync>) -> bool {
        let mut st = self.lock();
        if st.disposed || !self.ensure_running(&mut st) {
            return false;
        }
        st.capturing = true;
        *self.shared.on_captured.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::from(on_captured));
        self.shared.engine.begin_capture(now_ms());
        true
    }

    fn end_capture(&self) {
        let mut st = self.lock();
        *self.shared.on_captured.lock().unwrap_or_else(|e| e.into_inner()) = None;
        st.capturing = false;
        self.shared.engine.end_capture();
        if !st.disposed {
            Self::stop_if_idle(&mut st);
        }
    }

    fn dispose(&self) {
        let mut st = self.lock();
        if st.disposed {
            return;
        }
        st.disposed = true;
        st.has_bindings = false;
        st.capturing = false;
        *self.shared.on_captured.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Self::stop_if_idle(&mut st);
    }
}

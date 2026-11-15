//! RegisterHotKey / UnregisterHotKey sarmalayıcısı. Kısayollar verilen pencereye WM_HOTKEY olarak gelir; pencere
//! mesajlarını dinleyen taraf (App) `handle_message` çağırır, bu tür pencere oluşturmaz.

use std::collections::HashSet;
use std::sync::Mutex;

use koy_core::event::Event;
use koy_core::shortcuts::{Hotkey, HotkeyRegistrationResult, IHotkeyService};
use windows::Win32::Foundation::{ERROR_HOTKEY_ALREADY_REGISTERED, GetLastError};
use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey};

use super::window_styles::hwnd;

pub const WM_HOTKEY: i32 = 0x0312;
// MOD_NOREPEAT: tuşa basılı tutunca eylem tekrar tekrar tetiklenmesin.
const MOD_NOREPEAT: u32 = 0x4000;

struct State {
    registered: HashSet<i32>,
    disposed: bool,
}

pub struct HotkeyService {
    hwnd: isize,
    state: Mutex<State>,
    hotkey_pressed: Event<i32>,
}

impl HotkeyService {
    pub fn new(window_handle: isize) -> Self {
        HotkeyService { hwnd: window_handle, state: Mutex::new(State { registered: HashSet::new(), disposed: false }), hotkey_pressed: Event::new() }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Pencere işlemcisinden çağrılır; WM_HOTKEY ise olayı yükseltir ve true döner.
    pub fn handle_message(&self, msg: i32, wparam: isize) -> bool {
        if msg != WM_HOTKEY {
            return false;
        }
        let id = wparam as i32;
        if self.lock().registered.contains(&id) {
            self.hotkey_pressed.invoke(&id);
        }
        true
    }
}

impl Drop for HotkeyService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IHotkeyService for HotkeyService {
    fn hotkey_pressed(&self) -> &Event<i32> {
        &self.hotkey_pressed
    }

    fn register(&self, id: i32, hotkey: Hotkey) -> HotkeyRegistrationResult {
        if self.lock().disposed || self.hwnd == 0 || !hotkey.is_valid() {
            return HotkeyRegistrationResult::Failed;
        }
        // Aynı kimlik yeniden kaydedilirse önce eskisi kaldırılır.
        self.unregister(id);
        let modifiers = HOT_KEY_MODIFIERS(hotkey.modifiers.0 as u32 | MOD_NOREPEAT);
        // SAFETY: pencere tanıtıcısı çağıranın; hata kodu hemen okunur.
        match unsafe { RegisterHotKey(Some(hwnd(self.hwnd)), id, modifiers, hotkey.virtual_key as u32) } {
            Ok(()) => {
                self.lock().registered.insert(id);
                HotkeyRegistrationResult::Success
            }
            Err(_) if unsafe { GetLastError() } == ERROR_HOTKEY_ALREADY_REGISTERED => HotkeyRegistrationResult::AlreadyInUse,
            Err(e) if e.code() == ERROR_HOTKEY_ALREADY_REGISTERED.to_hresult() => HotkeyRegistrationResult::AlreadyInUse,
            Err(_) => HotkeyRegistrationResult::Failed,
        }
    }

    fn unregister(&self, id: i32) {
        if !self.lock().registered.remove(&id) {
            return;
        }
        // SAFETY: kendi kaydımız.
        let _ = unsafe { UnregisterHotKey(Some(hwnd(self.hwnd)), id) };
    }

    fn dispose(&self) {
        let ids: Vec<i32> = {
            let mut st = self.lock();
            if st.disposed {
                return;
            }
            st.disposed = true;
            st.registered.drain().collect()
        };
        for id in ids {
            // SAFETY: kendi kayıtlarımız.
            let _ = unsafe { UnregisterHotKey(Some(hwnd(self.hwnd)), id) };
        }
    }
}

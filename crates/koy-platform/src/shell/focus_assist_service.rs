//! Odak yardımı / Rahatsız etmeyin okuması (SHQueryUserNotificationState + FocusSessionManager).
//!
//! Sapma: C#'ta `StateChanged` ilk abone eklenince `FocusSessionManager` olayına bağlanır, son abone çıkınca söker.
//! Rust `Event<T>` abone ekleme anını bildirmez; bağlantı kurucuda kurulur, `dispose`/`Drop` ile sökülür.

use std::sync::{Arc, Mutex};

use koy_core::event::Event;
use koy_core::shell::IFocusAssistService;
use windows::Foundation::TypedEventHandler;
use windows::UI::Shell::FocusSessionManager;
use windows::Win32::UI::Shell::{QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_QUIET_TIME, SHQueryUserNotificationState, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, w};

use super::shell_action_service::os_build;

fn quns() -> Option<i32> {
    // SAFETY: yalnız sorgu.
    unsafe { SHQueryUserNotificationState() }.ok().map(|s| s.0)
}

fn read_quns() -> bool {
    quns().is_some_and(|s| s == QUNS_QUIET_TIME.0 || s == QUNS_PRESENTATION_MODE.0)
}

fn read_focus_session() -> bool {
    if os_build() < 22621 {
        return false;
    }
    FocusSessionManager::IsSupported().unwrap_or(false)
        && FocusSessionManager::GetDefault().and_then(|m| m.IsFocusActive()).unwrap_or(false)
}

fn is_do_not_disturb() -> bool {
    read_quns() || read_focus_session()
}

struct Inner {
    state_changed: Event<bool>,
}

pub struct FocusAssistService {
    inner: Arc<Inner>,
    focus_manager: Mutex<Option<(FocusSessionManager, i64)>>,
}

// SAFETY: FocusSessionManager çeviktir; yalnız kilit altında kullanılır.
unsafe impl Send for FocusAssistService {}
unsafe impl Sync for FocusAssistService {}

impl Default for FocusAssistService {
    fn default() -> Self {
        let inner = Arc::new(Inner { state_changed: Event::new() });
        let mut manager = None;
        if os_build() >= 22621 && FocusSessionManager::IsSupported().unwrap_or(false) {
            // Paketsiz süreçte olay kurulamayabilir: çağıran açılışta okumaya düşer.
            if let Ok(m) = FocusSessionManager::GetDefault() {
                let weak = Arc::downgrade(&inner);
                let token = m.IsFocusActiveChanged(&TypedEventHandler::new(move |_, _| {
                    if let Some(inner) = weak.upgrade() {
                        inner.state_changed.invoke(&is_do_not_disturb());
                    }
                    Ok(())
                }));
                if let Ok(token) = token {
                    manager = Some((m, token));
                }
            }
        }
        FocusAssistService { inner, focus_manager: Mutex::new(manager) }
    }
}

impl FocusAssistService {
    pub fn dispose(&self) {
        self.inner.state_changed.clear();
        if let Some((m, token)) = self.focus_manager.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = m.RemoveIsFocusActiveChanged(token);
        }
    }
}

impl Drop for FocusAssistService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IFocusAssistService for FocusAssistService {
    fn is_active(&self) -> bool {
        quns().is_some_and(|s| s == QUNS_BUSY.0 || s == QUNS_PRESENTATION_MODE.0)
    }

    /// Odaklanma oturumu ya da sunum modu açık mı. Bilinen kısıt: Windows 11'in "Rahatsız etmeyin" düğmesi tek başına
    /// okunamıyor (belgelenmiş API yok).
    fn is_do_not_disturb(&self) -> bool {
        is_do_not_disturb()
    }

    fn state_changed(&self) -> &Event<bool> {
        &self.inner.state_changed
    }

    fn open_settings(&self) {
        // ShellExecute 32'den büyük değer döndürürse başarılı (C#: ilk Process.Start istisna atarsa ikinci sayfa).
        // SAFETY: göstergeler çağrı süresince geçerli.
        let ok = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from("ms-settings:quiethours"), None, None, SW_SHOWNORMAL) }.0 as isize > 32;
        if !ok {
            // SAFETY: aynı.
            let _ = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from("ms-settings:notifications"), None, None, SW_SHOWNORMAL) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_states() {
        let service = FocusAssistService::default();
        let _ = service.is_active();
        let _ = service.is_do_not_disturb();
        service.dispose();
    }
}

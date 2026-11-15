//! Başlat menüsü, sistem tepsisi kısayolları ve uygulama başlatma işlemlerini yöneten servis.

use koy_core::shell::IShellActionService;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_A, VK_B, VK_LWIN, VK_N,
};

use crate::search::process_start::{ShellExecuteRequest, shell_execute};

fn key(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, dwFlags: flags, ..Default::default() } } }
}

/// Windows 11 (derleme 22000+) mi: `OperatingSystem.IsWindowsVersionAtLeast(10, 0, 22000)`.
pub fn is_windows_11() -> bool {
    crate::registry::get_string(crate::registry::Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", Some("CurrentBuildNumber"), false)
        .and_then(|b| b.trim().parse::<u32>().ok())
        .is_some_and(|b| b >= 22000)
}

/// Win + tuş kombinasyonu gönderir (C# `ShellActionService.SendWinCombo`).
pub fn send_win_combo(vk: VIRTUAL_KEY) {
    let inputs = [
        key(VK_LWIN, KEYBD_EVENT_FLAGS(0)),
        key(vk, KEYBD_EVENT_FLAGS(0)),
        key(vk, KEYEVENTF_KEYUP),
        key(VK_LWIN, KEYEVENTF_KEYUP),
    ];
    // SAFETY: girdiler doğru boyutlu INPUT dizisi.
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

#[derive(Default)]
pub struct ShellActionService;

impl IShellActionService for ShellActionService {
    fn send_win_key(&self) {
        let inputs = [key(VK_LWIN, KEYBD_EVENT_FLAGS(0)), key(VK_LWIN, KEYEVENTF_KEYUP)];
        // SAFETY: yukarıdaki gibi.
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    }

    // Win + B kısayolu sistem tepsisi taşma simgesini odaklar.
    fn toggle_system_tray(&self) {
        send_win_combo(VK_B);
    }

    fn open_quick_settings(&self) {
        send_win_combo(VK_A);
    }

    fn open_notification_center(&self) {
        send_win_combo(if is_windows_11() { VK_N } else { VK_A });
    }

    fn launch(&self, path: &str, arguments: Option<&str>) -> Result<(), String> {
        if path.trim().is_empty() {
            return Ok(());
        }
        // C#: kullanıcı iptali veya yetki hatası yutulur; çağıran Err'i de yok sayar.
        shell_execute(ShellExecuteRequest { file: path, arguments: arguments.filter(|a| !a.is_empty()), ..Default::default() })
            .map_err(|code| format!("Win32 error {code}"))
    }
}

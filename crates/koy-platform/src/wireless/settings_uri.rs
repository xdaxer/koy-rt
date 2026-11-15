//! Karşılığı: `Platform/Wireless/SettingsUri.cs`.

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, w};

/// Windows Ayarlar sayfalarını (ms-settings:) açar.
pub struct SettingsUri;

impl SettingsUri {
    pub fn open(uri: &str) {
        // Ayarlar uygulaması kaldırılmış ya da politika ile engellenmiş olabilir: hata yutulur.
        let target = HSTRING::from(uri);
        // SAFETY: tüm göstergeler çağrı süresince geçerli; dönen değer yok sayılır (C#'taki boş catch).
        unsafe {
            let _ = ShellExecuteW(None, w!("open"), &target, None, None, SW_SHOWNORMAL);
        }
    }
}

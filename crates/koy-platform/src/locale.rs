//! Uygulamanın arayüz dili (C# `CultureInfo.CurrentUICulture`). Koy açılışta kullanıcının dil ayarını kurar;
//! platform servisleri (sözlük gibi) buradan okur. Kurulmamışsa Windows'un görüntü dili.

use std::sync::RwLock;

use koy_core::settings::AppLanguage;

static CURRENT: RwLock<Option<String>> = RwLock::new(None);

/// Arayüz dilini kurar (`"tr"`, `"en"` ya da `"tr-TR"` gibi kültür adı).
pub fn set_current_ui_culture(name: &str) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(name.to_string());
    }
}

/// `CurrentUICulture.TwoLetterISOLanguageName`.
pub fn current_ui_two_letter() -> String {
    let name = CURRENT.read().ok().and_then(|g| g.clone()).unwrap_or_else(AppLanguage::system_default);
    name.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase()
}

/// Windows'un kullanıcı arayüz dili (ör. `"tr-TR"`); Core'a `AppLanguage::set_system_culture` ile bildirilir.
pub fn windows_ui_culture() -> Option<String> {
    use windows::Win32::Globalization::{GetUserDefaultLocaleName, MAX_LOCALE_NAME};
    let mut buf = [0u16; MAX_LOCALE_NAME as usize];
    // SAFETY: arabellek MAX_LOCALE_NAME karakter.
    let n = unsafe { GetUserDefaultLocaleName(&mut buf) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn two_letter_from_culture_name() {
        super::set_current_ui_culture("tr-TR");
        assert_eq!(super::current_ui_two_letter(), "tr");
        assert!(super::windows_ui_culture().is_some());
    }
}

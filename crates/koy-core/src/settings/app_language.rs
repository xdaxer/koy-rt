//! Karşılığı: `koy/src/Core/Settings/AppLanguage.cs` ve `UpdateChannel.cs`.

use std::sync::RwLock;

/// Uygulama dilinin kodları ve ilk açılıştaki dil kararı.
pub struct AppLanguage;

/// Platform katmanının bildirdiği Windows görüntü dili (`CultureInfo.CurrentUICulture`); yoksa ortam değişkenleri.
static SYSTEM_CULTURE: RwLock<Option<String>> = RwLock::new(None);

impl AppLanguage {
    pub const ENGLISH: &'static str = "en";
    pub const TURKISH: &'static str = "tr";

    /// Platform katmanı Windows'un görüntü dilini bildirir (örn. `"tr-TR"`). Core Windows API çağırmaz.
    pub fn set_system_culture(name: &str) {
        if let Ok(mut guard) = SYSTEM_CULTURE.write() {
            *guard = Some(name.to_string());
        }
    }

    /// Windows'un görüntü dili. Koy kendi kültürünü kurmadan önce okunur; sonrası kullanıcının ayarını yansıtırdı.
    /// (`SystemDefault` özelliği; Rust'ta ilk çağrıda değil her çağrıda hesaplanır.)
    pub fn system_default() -> String {
        let reported = SYSTEM_CULTURE.read().ok().and_then(|g| g.clone());
        let name = reported
            .or_else(|| ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"].iter().find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty())))
            .unwrap_or_default();
        Self::from_culture(&name).to_string()
    }

    /// Türkçe sistemde Türkçe, diğer her dilde İngilizce. `culture`: `"tr-TR"` / `"tr"` gibi kültür adı.
    pub fn from_culture(culture: &str) -> &'static str {
        if Self::is_turkish(culture) { Self::TURKISH } else { Self::ENGLISH }
    }

    /// `culture.TwoLetterISOLanguageName == "tr"` (büyük/küçük harf duyarsız).
    pub fn is_turkish(culture: &str) -> bool {
        let language = culture.split(['-', '_', '.']).next().unwrap_or("");
        language.eq_ignore_ascii_case(Self::TURKISH)
    }
}

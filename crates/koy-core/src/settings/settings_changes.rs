//! Karşılığı: `koy/src/Core/Settings/SettingsChanges.cs` ve `SettingsPageReset.cs`.

use super::{
    AppSettings, AppearanceSettings, ClipboardSettings, FocusSettings, GeneralSettings, MusicSettings, NotchSettings,
    SearchSettings, ShortcutsSettings, VoiceSettings,
};

/// Varsayılandan farklı ayarları bulur ("Yalnız değişenleri göster" ve satır vurgusu için).
/// Bölüm kayıtlarının her alanı tek tek karşılaştırılır. Eşitlik kayıt eşitliğinden ve `ValueList` / `ValueDictionary`
/// eşitliğinden gelir. Anahtar biçimi "Bölüm.Alan" (örn. "Dock.Enabled").
/// Sürüm alanı (`AppSettings.Version`) ve uygulamanın kendi yazdığı durum alanları (`INTERNAL_KEYS`)
/// kullanıcı ayarı olmadığı için hiç karşılaştırılmaz.
pub struct SettingsChanges;

// İlk açılıştan sonra hep varsayılandan farklıdır ve "Varsayılana dön" bunları bilerek korur (FirstRunPolicy);
// sayılsalar General sayfası sıfırlandıktan sonra bile "değişmiş" görünürdü.
const INTERNAL_KEYS: &[&str] = &["General.FirstRunCompleted", "General.LastSeenVersion", "Weather.LocationUpdatedUnix"];

impl SettingsChanges {
    pub fn changed_keys(current: &AppSettings, defaults: &AppSettings) -> Vec<String> {
        let mut changed = Vec::new();
        // `Version` atlanır; bölümlerin hepsi kayıt (IsSection).
        for section in current.section_views(defaults) {
            for field in section.fields {
                let key = format!("{}.{}", section.name, field.name);
                if !INTERNAL_KEYS.contains(&key.as_str()) && !field.same {
                    changed.push(key);
                }
            }
        }
        changed
    }

    /// Bir sayfanın (bölümün) varsayılandan farklı en az bir ayarı var mı.
    pub fn has_changes<I>(changed_keys: I, section: &str) -> bool
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        if section.is_empty() {
            panic!("section boş olamaz (ArgumentException)");
        }

        let prefix = format!("{section}.");
        changed_keys.into_iter().any(|k| k.as_ref().starts_with(&prefix) || k.as_ref() == section)
    }
}

/// Ayarlar penceresinde sayfa başına "Varsayılana döndür" (issue #69). Yalnız verilen bölümler yeni (varsayılan)
/// kayıtla değiştirilir; diğer bölümlere dokunulmaz. Uygulamanın kendi yazdığı durumlar (ilk açılış tamamlandı,
/// son görülen sürüm) korunur, yoksa sıfırlama sihirbazı ya da "Neler Geldi" ekranını yeniden açardı.
/// Ekranlar ve Dock sayfalarında sıfırlama yoktur: görev çubuğu / AppBar etkisi uygula-geri al ile birlikte ele alınır.
pub struct SettingsPageReset;

impl SettingsPageReset {
    /// Sıfırlanabilen bölüm adları (`AppSettings` özellik adlarıyla aynı).
    pub const RESETTABLE_SECTIONS: &'static [&'static str] =
        &["General", "Appearance", "Notch", "Music", "Focus", "Search", "Clipboard", "Shortcuts", "Voice"];

    /// Verilen bölümleri varsayılana döndürür. Bilinmeyen ya da sıfırlanamaz bölüm adı hata verir (panik, `ArgumentException`).
    pub fn reset(current: &AppSettings, sections: &[&str]) -> AppSettings {
        let mut result = current.clone();
        let mut seen: Vec<&str> = Vec::new();
        for section in sections {
            if seen.contains(section) {
                continue;
            }
            seen.push(section);

            if !Self::RESETTABLE_SECTIONS.contains(section) {
                panic!("Sıfırlanamaz bölüm: {section} (sections)");
            }

            match *section {
                "General" => {
                    result.general = GeneralSettings {
                        first_run_completed: current.general.first_run_completed,
                        last_seen_version: current.general.last_seen_version.clone(),
                        ..GeneralSettings::default()
                    }
                }
                "Appearance" => result.appearance = AppearanceSettings::default(),
                "Notch" => result.notch = NotchSettings::default(),
                "Music" => result.music = MusicSettings::default(),
                "Focus" => result.focus = FocusSettings::default(),
                "Search" => result.search = SearchSettings::default(),
                "Clipboard" => result.clipboard = ClipboardSettings::default(),
                "Shortcuts" => result.shortcuts = ShortcutsSettings::default(),
                "Voice" => result.voice = VoiceSettings::default(),
                other => panic!("Eksik sıfırlama dalı: {other}"),
            }
        }

        result
    }
}

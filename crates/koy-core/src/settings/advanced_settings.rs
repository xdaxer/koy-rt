//! Karşılığı: `koy/src/Core/Settings/AdvancedSettings.cs`.

use serde_json::{Number, Value};

use super::setting_value::SettingValue;
use super::{AppLanguage, AppSettings, SettingsStore};
use crate::text;

/// Gelişmiş sayfasındaki düzenleyici türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdvancedSettingKind {
    Bool,
    Integer,
    Number,
    Text,
    Choice,
}

/// Gelişmiş sayfasında bir satır: anahtar ("Notch.ClockShowSeconds"), tür, şu anki ve varsayılan değer (değişmez kültürde metin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancedSetting {
    pub key: String,
    pub kind: AdvancedSettingKind,
    pub value: String,
    pub default: String,
    pub choices: Vec<String>,
    pub section: String,
    pub property_name: String,
}

impl AdvancedSetting {
    /// `section` / `property_name` boşsa anahtardan türetilir (ilk noktadan önce/sonra). C# varsayılanları `""`.
    pub fn new(
        key: &str,
        kind: AdvancedSettingKind,
        value: &str,
        default: &str,
        choices: Vec<String>,
        section: &str,
        property_name: &str,
    ) -> Self {
        let dot = key.find('.');
        AdvancedSetting {
            key: key.to_string(),
            kind,
            value: value.to_string(),
            default: default.to_string(),
            choices,
            section: if !section.is_empty() {
                section.to_string()
            } else {
                dot.map(|i| key[..i].to_string()).unwrap_or_default()
            },
            property_name: if !property_name.is_empty() {
                property_name.to_string()
            } else {
                dot.map(|i| key[i + 1..].to_string()).unwrap_or_else(|| key.to_string())
            },
        }
    }

    pub fn is_changed(&self) -> bool {
        self.value != self.default
    }

    /// Geçerli arayüz kültürüne göre kullanıcı dostu başlık (Türkçe/İngilizce).
    pub fn title(&self) -> String {
        self.get_friendly_title(None)
    }

    /// Kullanıcı dostu başlık için takma ad (`title` ile aynı).
    pub fn friendly_title(&self) -> String {
        self.title()
    }

    /// Geçerli arayüz kültürüne göre bölümün kullanıcı dostu başlığı.
    pub fn section_title(&self) -> String {
        AdvancedSettings::get_section_title(&self.section, None)
    }

    /// Belirtilen kültüre veya dile göre kullanıcı dostu başlık döner.
    pub fn get_friendly_title(&self, language_or_culture: Option<&str>) -> String {
        AdvancedSettings::get_friendly_title(&self.property_name, language_or_culture)
    }

    /// Belirtilen kültüre veya dile göre bölüm başlığı döner.
    pub fn get_section_title(&self, language_or_culture: Option<&str>) -> String {
        AdvancedSettings::get_section_title(&self.section, language_or_culture)
    }
}

/// Gelişmiş sayfasında bölüm bazında gruplanmış ayarlar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancedSettingGroup {
    pub section: String,
    pub title: String,
    pub items: Vec<AdvancedSetting>,
}

impl AdvancedSettingGroup {
    pub fn section_title(&self) -> &str {
        &self.title
    }

    pub fn has_changed(&self) -> bool {
        self.items.iter().any(AdvancedSetting::is_changed)
    }
}

/// Bir değerin neden yazılmadığı (App metni çevirir).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AdvancedSettingError {
    #[default]
    None,
    UnknownKey,
    InvalidValue,
}

/// Gelişmiş sayfası (issue #69): kendi arayüzü olmayan her basit ayar, ayar modelinden kendiliğinden listelenir
/// (elle tablo yok). Değer yazma JSON ağacı üzerinden yapılır ve katı okunur: tanınmayan, türüne sığmayan ya da biçimi
/// bozuk değer hiçbir zaman ayara geçmez (tek alan bile düşerse reddedilir). Alan başına sayı aralığı modelde tanımlı
/// değil; aralıklar değeri kullanan politikada kenetlenir (ör. FontScalePolicy). C#'taki yansıma burada `section!`
/// makrosunun ürettiği alan tablosundan gelir.
pub struct AdvancedSettings;

// Uygulamanın kendi yazdığı durumlar ve sürüm: kullanıcıya düzenletilmez.
const INTERNAL: &[&str] = &["General.FirstRunCompleted", "General.LastSeenVersion", "Weather.LocationUpdatedUnix"];

// Koy'un kendi verisi (sıra, klasörler) ve ekran başına ayarlar: kendi akışları var.
const SKIPPED_SECTIONS: &[&str] = &["DockItems", "Displays", "QuickLaunch"];

const SECTION_TITLES: &[(&str, &str, &str)] = &[
    ("General", "Genel", "General"),
    ("Appearance", "Görünüm", "Appearance"),
    ("Dock", "Dock", "Dock"),
    ("DockItems", "Dock Öğeleri", "Dock Items"),
    ("Notch", "Çentik", "Notch"),
    ("Music", "Müzik", "Music"),
    ("Displays", "Ekranlar", "Displays"),
    ("Shortcuts", "Kısayollar", "Shortcuts"),
    ("HotCorners", "Sıcak Köşeler", "Hot Corners"),
    ("Voice", "Sesli Komut", "Voice Command"),
    ("Search", "Arama", "Search"),
    ("Focus", "Odak", "Focus"),
    ("Weather", "Hava Durumu", "Weather"),
    ("Clipboard", "Pano", "Clipboard"),
    ("QuickLaunch", "Hızlı Başlat", "Quick Launch"),
];

const PROPERTY_TITLES: &[(&str, &str, &str)] = &[
    // Appearance
    ("BaseFontSize", "Taban Yazı Boyutu", "Base Font Size"),
    ("ClockFontFamily", "Saat Yazı Tipi", "Clock Font"),
    ("FontFamily", "Yazı Tipi Ailesi", "Font Family"),
    ("FontWeight", "Yazı Kalınlığı", "Font Weight"),
    ("IconPack", "Simge Paketi", "Icon Pack"),
    ("IconPackFolder", "Simge Paketi Klasörü", "Icon Pack Folder"),
    ("UiScalePercent", "Arayüz Ölçeği (%)", "UI Scale (%)"),
    ("UserStyleOverridesEnabled", "Kullanıcı Stil Dosyası", "User Style Overrides"),
    // General
    ("DeveloperMode", "Geliştirici Modu", "Developer Mode"),
    ("Language", "Dil", "Language"),
    ("AutoCheckUpdates", "Otomatik Güncelleme Denetimi", "Auto Check Updates"),
    ("UpdateChannel", "Güncelleme Kanalı", "Update Channel"),
    ("TemperatureUnit", "Sıcaklık Birimi", "Temperature Unit"),
    ("WindSpeedUnit", "Rüzgâr Hızı Birimi", "Wind Speed Unit"),
    ("NetworkSpeedUnit", "Ağ Hızı Birimi", "Network Speed Unit"),
    // Dock
    ("LengthPercent", "Uzunluk (%)", "Length (%)"),
    ("EdgeOffsetDip", "Kenar Uzaklığı (DIP)", "Edge Offset (DIP)"),
    ("InnerPaddingDip", "İç Dolgu (DIP)", "Inner Padding (DIP)"),
    ("IconSpacingDip", "Simge Boşluğu (DIP)", "Icon Spacing (DIP)"),
    ("IndicatorColor", "Gösterge Rengi", "Indicator Color"),
    ("IndicatorCustomColor", "Özel Gösterge Rengi", "Custom Indicator Color"),
    ("IndicatorOnePerWindow", "Pencere Başına Gösterge", "Indicator Per Window"),
    ("BadgeColor", "Rozet Rengi", "Badge Color"),
    ("BadgeCustomColor", "Özel Rozet Rengi", "Custom Badge Color"),
    ("BadgeCorner", "Rozet Köşesi", "Badge Corner"),
    ("BadgeStyle", "Rozet Stili", "Badge Style"),
    ("RevealTrigger", "Belirme Tetikleyicisi", "Reveal Trigger"),
    ("RevealDelayMs", "Belirme Gecikmesi (ms)", "Reveal Delay (ms)"),
    ("HiddenEdgeLine", "Gizli Kenar Çizgisi", "Hidden Edge Line"),
    ("GroupingMode", "Gruplama Kipi", "Grouping Mode"),
    ("WindowMonitorScope", "Pencere Monitör Kapsamı", "Window Monitor Scope"),
    ("LeftClickAction", "Sol Tık Eylemi", "Left Click Action"),
    ("LabelMode", "Etiket Kipi", "Label Mode"),
    ("NewTrayIconVisibility", "Yeni Tepsi Simgesi Görünürlüğü", "New Tray Icon Visibility"),
    // Notch
    ("OpenDelayMs", "Açılma Gecikmesi (ms)", "Open Delay (ms)"),
    ("CloseDelayMs", "Kapanma Gecikmesi (ms)", "Close Delay (ms)"),
    ("TabBarStyle", "Sekme Çubuğu Stili", "Tab Bar Style"),
    ("ClockHourFormat", "Saat Biçimi (12/24)", "Clock Format (12/24)"),
    ("ClockShowSeconds", "Saniyeyi Göster", "Show Seconds"),
    ("ClockShowDate", "Tarihi Göster", "Show Date"),
    ("ClockShowWeekNumber", "Hafta Numarasını Göster", "Show Week Number"),
    ("ClockFirstDayOfWeek", "Haftanın İlk Günü", "First Day of Week"),
    ("ClockCustomDateFormat", "Özel Tarih Biçimi", "Custom Date Format"),
    ("ClockSecondTimeZoneId", "İkinci Saat Dilimi", "Second Time Zone"),
    ("LiveBackgroundTarget", "Canlı Arka Plan Hedefi", "Live Background Target"),
    ("LiveBackgroundSource", "Canlı Arka Plan Kaynağı", "Live Background Source"),
    ("LiveBackgroundPreset", "Canlı Arka Plan Önayarı", "Live Background Preset"),
    ("LiveBackgroundFilePath", "Canlı Arka Plan Dosyası", "Live Background File"),
    ("LiveBackgroundScrimPercent", "Canlı Arka Plan Perdesi (%)", "Live Background Scrim (%)"),
    // Music
    ("VisualizerStyle", "Görselleştirici Stili", "Visualizer Style"),
    ("VisualizerColorSource", "Görselleştirici Renk Kaynağı", "Visualizer Color Source"),
    ("VisualizerBarCount", "Görselleştirici Çubuk Sayısı", "Visualizer Bar Count"),
    // Search
    ("RetainPrior", "Son Sorguyu Koru", "Retain Prior Query"),
    ("HistoryMode", "Geçmiş Kipi", "History Mode"),
    ("Pinned", "Sabitli", "Pinned"),
];

fn lookup(table: &'static [(&'static str, &'static str, &'static str)], name: &str) -> Option<(&'static str, &'static str)> {
    // OrdinalIgnoreCase sözlük.
    table.iter().find(|(k, _, _)| text::eq_ignore_case(k, name)).map(|(_, tr, en)| (*tr, *en))
}

impl AdvancedSettings {
    /// Bölümün kullanıcı dostu başlığını döner. Tanımlı değilse PascalCase kelimelerine ayrılır.
    pub fn get_section_title(section: &str, language_or_culture: Option<&str>) -> String {
        if text::is_null_or_white_space(Some(section)) {
            return String::new();
        }

        let is_turkish = is_turkish(language_or_culture);
        if let Some((tr, en)) = lookup(SECTION_TITLES, section) {
            return if is_turkish { tr } else { en }.to_string();
        }

        Self::split_pascal_case(section)
    }

    /// Ayar alanının kullanıcı dostu başlığını döner. Tanımlı değilse PascalCase kelimelerine ayrılır.
    pub fn get_friendly_title(property_name: &str, language_or_culture: Option<&str>) -> String {
        if text::is_null_or_white_space(Some(property_name)) {
            return String::new();
        }

        let mut property_name = property_name;
        if property_name.contains('.') {
            property_name = property_name.rsplit('.').next().unwrap_or(property_name);
        }

        let is_turkish = is_turkish(language_or_culture);
        if let Some((tr, en)) = lookup(PROPERTY_TITLES, property_name) {
            return if is_turkish { tr } else { en }.to_string();
        }

        Self::split_pascal_case(property_name)
    }

    /// PascalCase metni kelimelerine ayırır ("BaseFontSize" -> "Base Font Size").
    pub fn split_pascal_case(input: &str) -> String {
        if text::is_null_or_white_space(Some(input)) {
            return String::new();
        }

        let chars: Vec<char> = input.chars().collect();
        let mut sb = String::with_capacity(input.len() + 8);
        for (i, &c) in chars.iter().enumerate() {
            if i > 0 && c.is_uppercase() {
                let prev = chars[i - 1];
                if prev.is_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_uppercase() && i + 1 < chars.len() && chars[i + 1].is_lowercase())
                {
                    sb.push(' ');
                }
            }
            sb.push(c);
        }

        sb
    }

    /// Listelenecek ayarlar: `dedicated_ui_keys` içinde olmayan bool, sayı, metin ve enum alanları, bölüm ve ad sırasıyla.
    pub fn list(current: &AppSettings, dedicated_ui_keys: &[&str]) -> Vec<AdvancedSetting> {
        let defaults = AppSettings::default();
        let mut result = Vec::new();

        for section in current.section_views(&defaults) {
            if SKIPPED_SECTIONS.contains(&section.name) {
                continue;
            }

            let mut fields = section.fields;
            fields.sort_by(|a, b| a.name.cmp(b.name));
            for field in fields {
                let key = format!("{}.{}", section.name, field.name);
                if INTERNAL.contains(&key.as_str()) || dedicated_ui_keys.contains(&key.as_str()) {
                    continue;
                }

                let Some(kind) = field.kind else { continue };
                let choices = if kind == AdvancedSettingKind::Choice { field.choices } else { Vec::new() };
                result.push(AdvancedSetting::new(&key, kind, &field.value, &field.default, choices, section.name, field.name));
            }
        }

        result
    }

    /// Ayarları bölümlerine göre gruplar ve her bölüme kullanıcı dostu başlık atar.
    pub fn group_by_section(settings: &[AdvancedSetting], language_or_culture: Option<&str>) -> Vec<AdvancedSettingGroup> {
        let mut groups: Vec<AdvancedSettingGroup> = Vec::new();
        for s in settings {
            match groups.iter_mut().find(|g| g.section == s.section) {
                Some(g) => g.items.push(s.clone()),
                None => groups.push(AdvancedSettingGroup {
                    section: s.section.clone(),
                    title: Self::get_section_title(&s.section, language_or_culture),
                    items: vec![s.clone()],
                }),
            }
        }
        groups
    }

    /// Belirtilen ayarları doğrudan bölüm bazında gruplanmış liste olarak döner.
    pub fn list_grouped(
        current: &AppSettings,
        dedicated_ui_keys: &[&str],
        language_or_culture: Option<&str>,
    ) -> Vec<AdvancedSettingGroup> {
        let flat = Self::list(current, dedicated_ui_keys);
        Self::group_by_section(&flat, language_or_culture)
    }

    /// Değeri ayara yazmayı dener. Başarısızsa dönen ayar değişmemiş ayardır (C# `out updated`) ve hata türü döner.
    /// Metin değişmez kültürde okunur (ondalık ayırıcı nokta); bool için true/false, enum için ad.
    pub fn try_set(current: &AppSettings, key: &str, text_in: Option<&str>) -> (AdvancedSettingError, AppSettings) {
        let unchanged = |e: AdvancedSettingError| (e, current.clone());

        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 || INTERNAL.contains(&key) || SKIPPED_SECTIONS.contains(&parts[0]) {
            return unchanged(AdvancedSettingError::UnknownKey);
        }

        let defaults = AppSettings::default();
        let views = current.section_views(&defaults);
        let Some(section) = views.iter().find(|s| s.name == parts[0]) else {
            return unchanged(AdvancedSettingError::UnknownKey);
        };
        let Some(field) = section.fields.iter().find(|f| f.name == parts[1]) else {
            return unchanged(AdvancedSettingError::UnknownKey);
        };
        let Some(kind) = field.kind else { return unchanged(AdvancedSettingError::UnknownKey) };

        let Some(value) = to_node(kind, &field.choices, text_in.unwrap_or("").trim()) else {
            return unchanged(AdvancedSettingError::InvalidValue);
        };

        let Value::Object(mut root) = current.encode() else { return unchanged(AdvancedSettingError::UnknownKey) };
        match root.get_mut(section.name) {
            Some(Value::Object(section_node)) => {
                section_node.insert(field.name.to_string(), value);
            }
            _ => return unchanged(AdvancedSettingError::UnknownKey),
        }

        let mut dropped = 0;
        let Ok(parsed) = SettingsStore::parse_tolerant_object(root, &mut dropped) else {
            return unchanged(AdvancedSettingError::InvalidValue);
        };
        if dropped > 0 {
            return unchanged(AdvancedSettingError::InvalidValue);
        }

        (AdvancedSettingError::None, parsed.fill_missing_sections())
    }

    /// Satırı varsayılana döndürür.
    pub fn reset(current: &AppSettings, key: &str) -> (AdvancedSettingError, AppSettings) {
        let rows = Self::list(current, &[]);
        let Some(row) = rows.iter().find(|r| r.key == key) else {
            return (AdvancedSettingError::UnknownKey, current.clone());
        };

        Self::try_set(current, key, Some(&row.default))
    }
}

fn is_turkish(language_or_culture: Option<&str>) -> bool {
    match language_or_culture {
        Some(s) if !text::is_null_or_white_space(Some(s)) => text::starts_with_ignore_case(s, "tr"),
        _ => AppLanguage::is_turkish(&AppLanguage::system_default()),
    }
}

fn to_node(kind: AdvancedSettingKind, choices: &[String], text_in: &str) -> Option<Value> {
    match kind {
        AdvancedSettingKind::Bool => {
            // bool.TryParse: büyük/küçük harf duyarsız "true"/"false".
            if text::eq_ignore_case(text_in, "true") {
                Some(Value::Bool(true))
            } else if text::eq_ignore_case(text_in, "false") {
                Some(Value::Bool(false))
            } else {
                None
            }
        }
        AdvancedSettingKind::Integer => text_in.parse::<i64>().ok().map(|l| Value::Number(Number::from(l))),
        AdvancedSettingKind::Number => text_in
            .parse::<f64>()
            .ok()
            .filter(|d| d.is_finite())
            .and_then(Number::from_f64)
            .map(Value::Number),
        AdvancedSettingKind::Text => {
            if text_in.chars().any(char::is_control) { None } else { Some(Value::String(text_in.to_string())) }
        }
        // Yalnız tanımlı ad kabul edilir; sayı ("3") ya da tanımsız değer enum'a sızmasın.
        AdvancedSettingKind::Choice => choices.iter().find(|n| text::eq_ignore_case(n, text_in)).map(|n| Value::String(n.clone())),
    }
}

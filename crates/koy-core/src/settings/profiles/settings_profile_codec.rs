//! Karşılığı: `koy/src/Core/Settings/Profiles/SettingsProfileCodec.cs`.

use serde_json::{Map, Value};

use super::{ImportedProfile, ProfileImportError, ProfileImportResult, ProfileSection};
use crate::notch::NotchEdgeLine;
use crate::settings::json_tree::{self, to_pretty_string};
use crate::settings::setting_value::SettingValue;
use crate::settings::{
    AppSettings, AppearanceSettings, DockSettings, NotchSettings, SettingsStore, ValueDictionary, ValueList,
};
use crate::shortcuts::{Hotkey, ShortcutBindings, ShortcutConflicts};
use crate::timers::PomodoroConfig;

/// Ayar profili ve paketi dosyası (JSON metni): dışa aktarma, doğrulamalı okuma, seçili bölümleri uygulama.
/// Saf mantık: dosya G/Ç yok, çağıran metni okur ve yazar.
pub struct SettingsProfileCodec;

impl SettingsProfileCodec {
    pub const FORMAT_NAME: &'static str = "koy-profile";
    pub const CURRENT_FORMAT_VERSION: i32 = 1;

    /// Bu uzunluktan büyük metin ayrıştırılmadan reddedilir (karakter sayısı).
    pub const MAX_LENGTH: i32 = 1_000_000;

    pub const MAX_NAME_LENGTH: i32 = 60;

    /// Ayar listelerinin (açık paneller, Kontrol Merkezi sırası) içe aktarmada kabul edilen en çok öğe sayısı. Gerçek
    /// listeler 7 öğeyi geçmez; 1 MB'lık bir dosya yüzbinlerce öğeyi settings.json'a kalıcı yazdırabilirdi.
    pub const MAX_LIST_ITEMS: i32 = 32;

    /// Ad boş olmamalı, en fazla 60 karakter olmalı; denetim karakteri, satır/paragraf ayırıcı (U+2028/2029) ve
    /// yön geçersiz kılma karakteri (U+202A-202E, U+2066-2069) içermemeli: sonuncusu önizlemede adı yanıltıcı gösterir.
    /// ZWJ gibi diğer biçim karakterleri (emoji dizileri) serbesttir.
    pub fn is_valid_name(name: Option<&str>) -> bool {
        let Some(name) = name else { return false };
        let trimmed = name.trim();
        // C# Length: UTF-16 kod birimi sayısı.
        let length = trimmed.encode_utf16().count() as i32;
        length > 0 && length <= Self::MAX_NAME_LENGTH && !trimmed.chars().any(is_forbidden_name_char)
    }

    /// Seçili bölümleri içeren profil metnini üretir. Seçilmeyen bölümler dosyaya hiç yazılmaz. Makineye özgü
    /// bölümler `ProfileSection` içinde olmadığından dışa aktarılamaz.
    /// Geçersiz ad ya da boş bölüm listesi `ArgumentException` idi: panik.
    pub fn export(current: &AppSettings, name: &str, sections: &[ProfileSection]) -> String {
        if !Self::is_valid_name(Some(name)) {
            panic!("Profil adı boş, çok uzun ya da denetim karakteri içeriyor. (name)");
        }

        let mut chosen: Vec<ProfileSection> = Vec::new();
        for s in sections {
            if !chosen.contains(s) {
                chosen.push(*s);
            }
        }
        chosen.sort_by_key(|s| *s as i32);
        if chosen.is_empty() {
            panic!("En az bir bölüm seçilmeli. (sections)");
        }
        // Tanımsız bölüm sayısı Rust'ta temsil edilemez (C#: Enum.IsDefined denetimi).

        // Dock başlat simgesinin yolu bu makineye özgü: paylaşılan dosyaya yazılmaz (içe aktarmada yerel değer korunur).
        let shareable = AppSettings {
            appearance: AppearanceSettings {
                app_icon_overrides: ValueDictionary::new(),
                icon_pack_folder: String::new(),
                ..current.appearance.clone()
            },
            dock: DockSettings { start_icon_path: String::new(), ..current.dock.clone() },
            // Canlı arka plan dosyasının yolu da bu makineye özgü.
            notch: NotchSettings { live_background_file_path: String::new(), ..current.notch.clone() },
            ..current.clone()
        };
        let full = shareable.encode();
        let mut settings = Map::new();
        settings.insert("Version".to_string(), Value::from(AppSettings::CURRENT_VERSION));
        for section in &chosen {
            let key = section.name();
            settings.insert(key.to_string(), full.get(key).cloned().unwrap_or(Value::Null));
        }

        let section_names: Vec<Value> = chosen.iter().map(|s| Value::String(s.name().to_string())).collect();
        let mut root = Map::new();
        root.insert("Format".to_string(), Value::String(Self::FORMAT_NAME.to_string()));
        root.insert("FormatVersion".to_string(), Value::from(Self::CURRENT_FORMAT_VERSION));
        root.insert("Name".to_string(), Value::String(name.trim().to_string()));
        root.insert("Sections".to_string(), Value::Array(section_names));
        root.insert("Settings".to_string(), Value::Object(settings));
        to_pretty_string(&Value::Object(root))
    }

    /// Profil metnini doğrular. Bilinmeyen biçim, daha yeni biçim sürümü, geçersiz ad, bilinmeyen ya da makineye özgü
    /// bölüm, boş bölüm listesi ve tolerantlı okumada düzeltilmesi gereken (geçersiz değerli) alan reddedilir.
    /// Tolerantlı okumanın kaçırdığı değerler de reddedilir (`has_invalid_values`): sonsuz sayı, aralık dışı Pomodoro
    /// süresi, çözülemeyen ya da çakışan kısayol, aşırı uzun liste. (Tanımsız enum sayısı Rust'ta okunamaz: alan düşer,
    /// sayaç artar ve profil yine reddedilir.)
    /// Bölüm İÇİNDEKİ bilinmeyen anahtarlar sessizce yok sayılır: ileri sürümün eklediği alan bu sürüme taşınmaz.
    pub fn read(json: &str) -> ProfileImportResult {
        if json.len() > Self::MAX_LENGTH as usize && json.encode_utf16().count() > Self::MAX_LENGTH as usize {
            return fail(ProfileImportError::TooLarge);
        }

        // C#: yinelenen anahtar ArgumentException → MalformedJson; json_tree ayrıştırıcısı bunu doğrudan hata sayar.
        read_core(json)
    }

    /// Profilde bildirilen ve uygulandığında değişecek bölümler (içe aktarma önizlemesi). Karşılaştırma
    /// `apply` sonucuna göre yapılır; böylece yerel dosya yolu gibi korunan değerler fark sayılmaz.
    pub fn changed_sections(current: &AppSettings, profile: &ImportedProfile) -> Vec<ProfileSection> {
        profile
            .sections
            .iter()
            .copied()
            .filter(|&section| !section_eq(current, &Self::apply(current, profile, &[section]), section))
            .collect()
    }

    /// Seçili ve profilde bildirilmiş bölümleri mevcut ayarların üzerine uygular. Dosyada olmayan bölüm değişmez.
    /// Dock başlat simgesinin dosya yolu makineye özgüdür: içe aktarmada yerel değer korunur.
    /// Girdi değiştirilmez; sonuç yeni bir kayıttır ve profille nesne paylaşmaz (Rust'ta kopya zaten garanti).
    pub fn apply(current: &AppSettings, profile: &ImportedProfile, chosen: &[ProfileSection]) -> AppSettings {
        let mut result = current.clone();
        let mut seen: Vec<ProfileSection> = Vec::new();
        for section in chosen {
            if seen.contains(section) {
                continue;
            }
            seen.push(*section);

            if !profile.sections.contains(section) {
                continue;
            }

            match section {
                ProfileSection::Appearance => {
                    result.appearance = AppearanceSettings {
                        app_icon_overrides: current.appearance.app_icon_overrides.clone(),
                        icon_pack_folder: current.appearance.icon_pack_folder.clone(),
                        ..profile.settings.appearance.clone()
                    };
                }
                ProfileSection::Dock => {
                    result.dock = DockSettings {
                        start_icon_path: current.dock.start_icon_path.clone(),
                        ..profile.settings.dock.clone()
                    };
                }
                ProfileSection::Notch => {
                    result.notch = NotchSettings {
                        enabled_panels: ValueList::from_items(profile.settings.notch.enabled_panels.iter().cloned()),
                        control_center_layout: ValueList::from_items(
                            profile.settings.notch.control_center_layout.iter().cloned(),
                        ),
                        live_background_file_path: current.notch.live_background_file_path.clone(),
                        ..profile.settings.notch.clone()
                    };
                }
                ProfileSection::Music => result.music = profile.settings.music.clone(),
                ProfileSection::Shortcuts => result.shortcuts = profile.settings.shortcuts.clone(),
                ProfileSection::Focus => result.focus = profile.settings.focus.clone(),
            }
        }
        result
    }
}

fn is_forbidden_name_char(c: char) -> bool {
    let cp = c as u32;
    c.is_control()
        || cp == 0x2028 // UnicodeCategory.LineSeparator
        || cp == 0x2029 // UnicodeCategory.ParagraphSeparator
        || (0x202A..=0x202E).contains(&cp)
        || (0x2066..=0x2069).contains(&cp)
}

fn fail(error: ProfileImportError) -> ProfileImportResult {
    ProfileImportResult { error, profile: None }
}

fn string_of<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    match obj.get(key) {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}

fn try_int_of(obj: &Map<String, Value>, key: &str) -> Option<i32> {
    obj.get(key).and_then(Value::as_i64).and_then(|n| i32::try_from(n).ok())
}

fn read_core(json: &str) -> ProfileImportResult {
    let Ok(node) = json_tree::parse(json) else { return fail(ProfileImportError::MalformedJson) };

    let Value::Object(root) = node else { return fail(ProfileImportError::NotAProfile) };
    if string_of(&root, "Format") != Some(SettingsProfileCodec::FORMAT_NAME) {
        return fail(ProfileImportError::NotAProfile);
    }
    let version = match try_int_of(&root, "FormatVersion") {
        Some(v) if v >= 1 => v,
        _ => return fail(ProfileImportError::NotAProfile),
    };
    if version > SettingsProfileCodec::CURRENT_FORMAT_VERSION {
        return fail(ProfileImportError::UnsupportedFormatVersion);
    }

    let name = string_of(&root, "Name");
    if !SettingsProfileCodec::is_valid_name(name) {
        return fail(ProfileImportError::InvalidName);
    }

    let Some(Value::Array(declared)) = root.get("Sections") else { return fail(ProfileImportError::NotAProfile) };

    let mut sections: Vec<ProfileSection> = Vec::new();
    for item in declared {
        let section = match item {
            Value::String(s) => ProfileSection::try_parse(s),
            _ => None,
        };
        let Some(section) = section else { return fail(ProfileImportError::UnknownSection) };
        if !sections.contains(&section) {
            sections.push(section);
        }
    }
    if sections.is_empty() {
        return fail(ProfileImportError::NoSections);
    }

    let Some(Value::Object(settings_object)) = root.get("Settings") else { return fail(ProfileImportError::NotAProfile) };

    // Yalnız bildirilen bölümler tolerantlı okumaya girer: bildirilmeyen bir bölümdeki bozukluk profili etkilemez.
    let mut scoped = Map::new();
    scoped.insert("Version".to_string(), Value::from(AppSettings::CURRENT_VERSION));
    for section in &sections {
        let key = section.name();
        if let Some(section_node) = settings_object.get(key) {
            if !section_node.is_null() {
                scoped.insert(key.to_string(), section_node.clone());
            }
        }
    }

    let mut dropped = 0;
    // Hata yolu çözülemezse ParseTolerant onarmaz ve hata çıkar: dosya temiz biçimde reddedilir, hiçbir şey yazılmaz.
    let Ok(parsed) = SettingsStore::parse_tolerant_object(scoped, &mut dropped) else {
        return fail(ProfileImportError::InvalidValues);
    };
    // fill_missing_sections dock yerleşimini (DockLayoutLimits.Sanitize) sessizce onarır; profil bozuk değeri reddeder.
    let bad_dock_layout = sections.contains(&ProfileSection::Dock) && has_invalid_dock_layout(&parsed.dock);
    let settings = parsed.fill_missing_sections();
    if dropped > 0 || bad_dock_layout || has_invalid_values(&settings, &sections) {
        return fail(ProfileImportError::InvalidValues);
    }

    ProfileImportResult {
        error: ProfileImportError::None,
        profile: Some(ImportedProfile { name: name.unwrap_or("").trim().to_string(), sections, settings }),
    }
}

/// Tolerantlı okumanın hata saymadığı ama uygulanırsa ayarları bozacak değerler. Yalnız bildirilen bölümlere bakılır.
/// JSON sonsuz sayıyı (1e400) kabul eder; kısayol metni olduğu gibi saklanır ve geçersizse mevcut çalışan kısayolun
/// üzerine yazılıp onu sessizce öldürürdü. Yeni double / liste alanı eklenince buraya da eklenir (test bunu alan
/// tablosuyla zorlar).
fn has_invalid_values(s: &AppSettings, sections: &[ProfileSection]) -> bool {
    for section in sections {
        let bad = match section {
            ProfileSection::Appearance => !s.appearance.corner_radius.is_finite() || !s.appearance.base_font_size.is_finite(),
            ProfileSection::Dock => !s.dock.icon_size.is_finite(),
            ProfileSection::Notch => {
                s.notch.edge_line_thickness != NotchEdgeLine::clamp_thickness(s.notch.edge_line_thickness)
                    || !is_sane_list(&s.notch.enabled_panels)
                    || !is_sane_list(&s.notch.control_center_layout)
                    || !is_sane_list(&s.notch.panel_order)
                    || s.notch.home_cards.len() > SettingsProfileCodec::MAX_LIST_ITEMS as usize
            }
            ProfileSection::Music => false,
            ProfileSection::Shortcuts => has_invalid_shortcut(s),
            ProfileSection::Focus => {
                !is_pomodoro_minutes(s.focus.pomodoro_work_minutes)
                    || !is_pomodoro_minutes(s.focus.pomodoro_short_break_minutes)
                    || !is_pomodoro_minutes(s.focus.pomodoro_long_break_minutes)
            }
        };
        if bad {
            return true;
        }
    }
    false
}

fn has_invalid_dock_layout(dock: &DockSettings) -> bool {
    !dock.edge_offset_dip.is_finite() || !dock.inner_padding_dip.is_finite() || !dock.icon_spacing_dip.is_finite()
}

// null öğe Rust'ta temsil edilemez; okuma sırasında düşer ve sayaç artar (profil reddedilir).
fn is_sane_list(list: &ValueList<String>) -> bool {
    list.len() <= SettingsProfileCodec::MAX_LIST_ITEMS as usize
}

// Çalışma zamanı bu aralığa sıkıştırır ama Ayarlar sayfası ham değeri gösterir ve kaydederken geri yazar.
fn is_pomodoro_minutes(minutes: i32) -> bool {
    (PomodoroConfig::MIN_MINUTES..=PomodoroConfig::MAX_MINUTES).contains(&minutes)
}

// Ayarlar sayfasının kaydetme denetimiyle aynı kural (ShortcutConflicts.Check): boş = atanmamış, geri kalanı çözülmeli,
// sistemce ayrılmış olmamalı ve profilin kendi içinde çakışmamalı.
fn has_invalid_shortcut(s: &AppSettings) -> bool {
    let all = ShortcutBindings::read_all(s);
    for (action, text) in all.iter() {
        if crate::text::is_null_or_white_space(Some(text.as_str())) {
            continue;
        }
        let Some(hotkey) = Hotkey::try_parse(text.as_str()) else { return true };
        if ShortcutConflicts::check(hotkey, *action, &all).is_some() {
            return true;
        }
    }
    false
}

fn section_eq(a: &AppSettings, b: &AppSettings, section: ProfileSection) -> bool {
    match section {
        ProfileSection::Appearance => a.appearance == b.appearance,
        ProfileSection::Dock => a.dock == b.dock,
        ProfileSection::Notch => a.notch == b.notch,
        ProfileSection::Music => a.music == b.music,
        ProfileSection::Shortcuts => a.shortcuts == b.shortcuts,
        ProfileSection::Focus => a.focus == b.focus,
    }
}

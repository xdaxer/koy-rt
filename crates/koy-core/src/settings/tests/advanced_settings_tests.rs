//! `Settings/AdvancedSettingsTests.cs`.

use std::collections::HashMap;

use crate::settings::*;

fn rows(current: &AppSettings, dedicated: &[&str]) -> HashMap<String, AdvancedSetting> {
    AdvancedSettings::list(current, dedicated).into_iter().map(|r| (r.key.clone(), r)).collect()
}

#[test]
fn lists_simple_fields_without_dedicated_ui_and_skips_internal_ones() {
    let rows = AdvancedSettings::list(&AppSettings::default(), &["Dock.ShowTray"]);
    let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();

    assert!(keys.contains(&"Notch.ClockShowSeconds"));
    assert!(keys.contains(&"Appearance.UiScalePercent"));
    assert!(!keys.contains(&"Dock.ShowTray")); // kendi arayüzü var
    assert!(!keys.contains(&"General.FirstRunCompleted")); // uygulamanın kendi durumu
    assert!(!keys.contains(&"Version"));
    assert!(!keys.iter().any(|k| k.starts_with("DockItems.")));
    assert!(!keys.iter().any(|k| k.starts_with("Displays.")));
    assert!(rows.iter().all(|r| !r.is_changed()));
}

#[test]
fn kinds_and_choices_come_from_the_model() {
    let rows = rows(&AppSettings::default(), &[]);

    assert_eq!(AdvancedSettingKind::Bool, rows["Notch.ClockShowSeconds"].kind);
    assert_eq!(AdvancedSettingKind::Integer, rows["Appearance.UiScalePercent"].kind);
    assert_eq!(AdvancedSettingKind::Number, rows["Appearance.BaseFontSize"].kind);
    assert_eq!(AdvancedSettingKind::Text, rows["Notch.ClockCustomDateFormat"].kind);
    assert_eq!(AdvancedSettingKind::Choice, rows["Notch.ClockHourFormat"].kind);
    assert!(rows["Notch.ClockHourFormat"].choices.contains(&"Hour24".to_string()));
}

#[test]
fn valid_values_are_written_and_read_back() {
    let cases = [
        ("Notch.ClockShowSeconds", "true"),
        ("Appearance.UiScalePercent", "120"),
        ("Appearance.BaseFontSize", "14.5"),
        ("Notch.ClockHourFormat", "hour24"),
        ("Notch.ClockCustomDateFormat", "dd.MM.yyyy"),
    ];
    for (key, text) in cases {
        let (error, updated) = AdvancedSettings::try_set(&AppSettings::default(), key, Some(text));

        assert_eq!(AdvancedSettingError::None, error, "{key}={text}");
        let list = AdvancedSettings::list(&updated, &[]);
        let row = list.iter().find(|r| r.key == key).unwrap();
        assert!(row.is_changed(), "{key}={text}");
        assert!(row.value.eq_ignore_ascii_case(text), "{key}={text}: {}", row.value);
    }
}

#[test]
fn invalid_values_are_never_written() {
    let cases = [
        ("Notch.ClockShowSeconds", "yes"),
        ("Appearance.UiScalePercent", "12.5"),
        ("Appearance.UiScalePercent", "99999999999"),
        ("Appearance.BaseFontSize", "NaN"),
        ("Appearance.BaseFontSize", "14,5"),
        ("Notch.ClockHourFormat", "3"),
        ("Notch.ClockHourFormat", "Hour13"),
        ("Notch.ClockCustomDateFormat", "a\nb"),
    ];
    for (key, text) in cases {
        let current = AppSettings::default();

        let (error, updated) = AdvancedSettings::try_set(&current, key, Some(text));

        assert_eq!(AdvancedSettingError::InvalidValue, error, "{key}={text}");
        assert_eq!(current, updated, "{key}={text}");
    }
}

#[test]
fn unknown_or_protected_keys_are_rejected() {
    for key in ["Nope.Thing", "Notch", "General.FirstRunCompleted", "DockItems.PinnedOrder"] {
        assert_eq!(AdvancedSettingError::UnknownKey, AdvancedSettings::try_set(&AppSettings::default(), key, Some("1")).0, "{key}");
    }
}

#[test]
fn writing_one_field_keeps_every_other_setting() {
    let current = AppSettings {
        general: GeneralSettings { language: "en".to_string(), first_run_completed: true, ..Default::default() },
        ..Default::default()
    };

    let (_, updated) = AdvancedSettings::try_set(&current, "Notch.ClockShowSeconds", Some("true"));

    assert_eq!(current.general, updated.general);
    assert_eq!(current.dock, updated.dock);
    assert!(updated.notch.clock_show_seconds);
}

#[test]
fn reset_returns_a_row_to_its_default() {
    let (_, changed) = AdvancedSettings::try_set(&AppSettings::default(), "Appearance.UiScalePercent", Some("130"));

    let (error, reset) = AdvancedSettings::reset(&changed, "Appearance.UiScalePercent");

    assert_eq!(AdvancedSettingError::None, error);
    assert_eq!(AppSettings::default().appearance.ui_scale_percent, reset.appearance.ui_scale_percent);
}

#[test]
fn advanced_setting_populates_section_and_property_name_correctly() {
    let rows = AdvancedSettings::list(&AppSettings::default(), &[]);
    let font_row = rows.iter().find(|r| r.key == "Appearance.BaseFontSize").unwrap();

    assert_eq!("Appearance", font_row.section);
    assert_eq!("BaseFontSize", font_row.property_name);
    assert_eq!("Taban Yazı Boyutu", font_row.get_friendly_title(Some("tr")));
    assert_eq!("Base Font Size", font_row.get_friendly_title(Some("en")));
    assert_eq!("Görünüm", font_row.get_section_title(Some("tr")));
    assert_eq!("Appearance", font_row.get_section_title(Some("en")));
}

#[test]
fn section_titles_match_expected_translations() {
    let cases = [
        ("Appearance", "Görünüm", "Appearance"),
        ("Dock", "Dock", "Dock"),
        ("Notch", "Çentik", "Notch"),
        ("Music", "Müzik", "Music"),
        ("General", "Genel", "General"),
    ];
    for (section, expected_tr, expected_en) in cases {
        assert_eq!(expected_tr, AdvancedSettings::get_section_title(section, Some("tr")));
        assert_eq!(expected_en, AdvancedSettings::get_section_title(section, Some("en")));
    }
}

#[test]
fn property_titles_match_expected_translations() {
    let cases = [
        ("BaseFontSize", "Taban Yazı Boyutu", "Base Font Size"),
        ("ClockFontFamily", "Saat Yazı Tipi", "Clock Font"),
        ("FontFamily", "Yazı Tipi Ailesi", "Font Family"),
        ("FontWeight", "Yazı Kalınlığı", "Font Weight"),
        ("IconPack", "Simge Paketi", "Icon Pack"),
        ("IconPackFolder", "Simge Paketi Klasörü", "Icon Pack Folder"),
        ("UiScalePercent", "Arayüz Ölçeği (%)", "UI Scale (%)"),
        ("UserStyleOverridesEnabled", "Kullanıcı Stil Dosyası", "User Style Overrides"),
        ("DeveloperMode", "Geliştirici Modu", "Developer Mode"),
    ];
    for (property, expected_tr, expected_en) in cases {
        assert_eq!(expected_tr, AdvancedSettings::get_friendly_title(property, Some("tr")));
        assert_eq!(expected_en, AdvancedSettings::get_friendly_title(property, Some("en")));
        // Also supports full key format e.g. "Appearance.BaseFontSize"
        assert_eq!(expected_tr, AdvancedSettings::get_friendly_title(&format!("Section.{property}"), Some("tr")));
    }
}

#[test]
fn fallback_formatter_splits_pascal_case_into_words() {
    let cases = [
        ("SomeFutureCustomSetting", "Some Future Custom Setting"),
        ("EnableFastRenderingMode", "Enable Fast Rendering Mode"),
        ("Simple", "Simple"),
    ];
    for (input, expected) in cases {
        assert_eq!(expected, AdvancedSettings::split_pascal_case(input));
        // When untranslated, GetFriendlyTitle falls back to SplitPascalCase
        assert_eq!(expected, AdvancedSettings::get_friendly_title(input, Some("tr")));
        assert_eq!(expected, AdvancedSettings::get_friendly_title(input, Some("en")));
    }
}

#[test]
fn group_by_section_and_list_grouped_organize_items_by_section() {
    let groups = AdvancedSettings::list_grouped(&AppSettings::default(), &[], Some("tr"));

    assert!(!groups.is_empty());
    let appearance_group = groups.iter().find(|g| g.section == "Appearance").expect("Appearance grubu");
    assert_eq!("Görünüm", appearance_group.title);
    assert_eq!("Görünüm", appearance_group.section_title());
    assert!(appearance_group.items.iter().any(|i| i.property_name == "BaseFontSize"));
}

#[test]
fn developer_mode_setting_defaults_to_false_and_is_mapped_in_settings_key_map() {
    let mut general = GeneralSettings::default();
    assert!(!general.developer_mode);

    general.developer_mode = true;
    assert!(general.developer_mode);

    assert!(SettingsKeyMap::v3_to_v4().contains_key("DeveloperMode"));
    let (section, name) = &SettingsKeyMap::v3_to_v4()["DeveloperMode"];
    assert_eq!("General", section);
    assert_eq!("DeveloperMode", name);
}

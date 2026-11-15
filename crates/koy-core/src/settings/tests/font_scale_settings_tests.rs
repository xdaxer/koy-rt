//! `FontScalePolicyTests.cs` ve `Appearance/UserStyleOverridesTests.cs` içinde Settings'e bağlı olan testler.

use super::{TempDir, load_json};
use crate::appearance::FontScalePolicy;
use crate::settings::*;

#[test]
fn default_appearance_font_values_are_safe() {
    let appearance = AppearanceSettings::default();

    assert_eq!("", appearance.font_family);
    assert_eq!("", appearance.clock_font_family);
    assert_eq!(FontScalePolicy::DEFAULT_BASE_FONT_SIZE, appearance.base_font_size);
    assert_eq!(FontScalePolicy::DEFAULT_FONT_WEIGHT, appearance.font_weight);
    assert_eq!(FontScalePolicy::DEFAULT_UI_SCALE, appearance.ui_scale_percent);
}

#[test]
fn font_settings_survive_a_save_and_load_round_trip() {
    let dir = TempDir::unique("koy-fonts-");
    let store = SettingsStore::new(dir.file("settings.json"));
    let settings = AppSettings {
        appearance: AppearanceSettings {
            font_family: "Cascadia Mono".to_string(),
            clock_font_family: "Consolas".to_string(),
            base_font_size: 15.0,
            font_weight: 600,
            ui_scale_percent: 125,
            ..Default::default()
        },
        ..Default::default()
    };

    store.save(&settings).unwrap();
    let loaded = store.load();

    assert_eq!(settings.appearance, loaded.appearance);
}

#[test]
fn missing_font_fields_in_file_read_as_safe_defaults() {
    let read = load_json(r#"{ "Appearance": { "Theme": "Dark" } }"#);

    assert_eq!("", read.appearance.font_family);
    assert_eq!(FontScalePolicy::DEFAULT_UI_SCALE, read.appearance.ui_scale_percent);
    assert_eq!(FontScalePolicy::DEFAULT_BASE_FONT_SIZE, read.appearance.base_font_size);
}

#[test]
fn user_style_setting_is_off_by_default_and_survives_a_save_and_load_round_trip() {
    assert!(!AppSettings::default().appearance.user_style_overrides_enabled);

    let dir = TempDir::unique("koy-userstyle-");
    let store = SettingsStore::new(dir.file("settings.json"));
    let settings = AppSettings {
        appearance: AppearanceSettings { user_style_overrides_enabled: true, ..Default::default() },
        ..Default::default()
    };

    store.save(&settings).unwrap();

    assert!(store.load().appearance.user_style_overrides_enabled);
}

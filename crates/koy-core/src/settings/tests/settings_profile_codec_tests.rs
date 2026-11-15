//! `Settings/SettingsProfileCodecTests.cs`.

use serde_json::Value;

use crate::appearance::{AppTheme, IconPack};
use crate::dock::DockStyleMode;
use crate::notch::{LiveBackgroundSourceKind, LiveBackgroundTarget};
use crate::settings::setting_value::FieldView;
use crate::settings::*;

fn source() -> AppSettings {
    AppSettings {
        general: GeneralSettings { language: "tr".to_string(), first_run_completed: true, ..Default::default() },
        appearance: AppearanceSettings { theme: AppTheme::Light, corner_radius: 17.0, ..Default::default() },
        dock: DockSettings {
            style: DockStyleMode::FullWidth,
            start_icon_path: r"C:\Users\ornek\simge.png".to_string(),
            ..Default::default()
        },
        music: MusicSettings { show_waveform: false, ..Default::default() },
        ..Default::default()
    }
}

fn profile(json: &str) -> ImportedProfile {
    SettingsProfileCodec::read(json).profile.expect("profil geçerli olmalı")
}

fn node(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}

#[test]
fn export_then_read_round_trips_the_selected_sections() {
    let json = SettingsProfileCodec::export(&source(), "Sunum", &[ProfileSection::Appearance, ProfileSection::Dock]);

    let result = SettingsProfileCodec::read(&json);

    assert!(result.is_valid());
    let p = result.profile.unwrap();
    assert_eq!("Sunum", p.name);
    assert_eq!(vec![ProfileSection::Appearance, ProfileSection::Dock], p.sections);
    assert_eq!(source().appearance, p.settings.appearance);
    assert_eq!(DockSettings { start_icon_path: String::new(), ..source().dock }, p.settings.dock);
}

#[test]
fn export_clears_the_local_start_icon_path_from_the_file() {
    let json = SettingsProfileCodec::export(&source(), "Sunum", &[ProfileSection::Dock]);

    assert_eq!("", node(&json)["Settings"]["Dock"]["StartIconPath"].as_str().unwrap());
    assert!(!json.contains("simge.png"));
    assert!(!json.contains("ornek"));
}

#[test]
fn export_writes_no_unselected_or_machine_specific_section() {
    let json = SettingsProfileCodec::export(&source(), "Sunum", &[ProfileSection::Appearance]);

    assert!(json.contains("\"Appearance\""));
    assert!(!json.contains("\"General\""));
    assert!(!json.contains("\"FirstRunCompleted\""));
    assert!(!json.contains("\"Dock\""));
    assert!(!json.contains("\"Displays\""));
    assert!(!json.contains("\"Weather\""));
}

#[test]
fn export_sorts_sections_and_removes_duplicates() {
    let json = SettingsProfileCodec::export(&source(), "P", &[ProfileSection::Dock, ProfileSection::Appearance, ProfileSection::Dock]);

    assert_eq!(vec![ProfileSection::Appearance, ProfileSection::Dock], profile(&json).sections);
}

#[test]
fn export_rejects_an_invalid_name() {
    for name in ["", "   ", "a\nb"] {
        let result = std::panic::catch_unwind(|| SettingsProfileCodec::export(&AppSettings::default(), name, &[ProfileSection::Appearance]));
        assert!(result.is_err(), "{name:?}");
    }
}

#[test]
fn export_rejects_a_name_longer_than_the_limit_and_accepts_the_limit() {
    let max = SettingsProfileCodec::MAX_NAME_LENGTH as usize;
    assert!(SettingsProfileCodec::is_valid_name(Some(&"a".repeat(max))));
    assert!(!SettingsProfileCodec::is_valid_name(Some(&"a".repeat(max + 1))));
}

#[test]
fn export_rejects_an_empty_section_list() {
    let result = std::panic::catch_unwind(|| SettingsProfileCodec::export(&AppSettings::default(), "P", &[]));
    assert!(result.is_err());
}

#[test]
fn read_rejects_malformed_json() {
    assert_eq!(ProfileImportError::MalformedJson, SettingsProfileCodec::read("{").error);
}

#[test]
fn read_rejects_content_that_is_not_a_profile() {
    let cases = [
        "[]",
        "{\"Version\":4}",
        "{\"Format\":\"baska\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[\"Dock\"],\"Settings\":{}}",
        "{\"Format\":\"koy-profile\",\"Name\":\"A\",\"Sections\":[\"Dock\"],\"Settings\":{}}",
        "{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[\"Dock\"]}",
    ];
    for json in cases {
        assert_eq!(ProfileImportError::NotAProfile, SettingsProfileCodec::read(json).error, "{json}");
    }
}

#[test]
fn read_rejects_a_newer_format_version() {
    let json = "{\"Format\":\"koy-profile\",\"FormatVersion\":2,\"Name\":\"A\",\"Sections\":[\"Dock\"],\"Settings\":{}}";

    assert_eq!(ProfileImportError::UnsupportedFormatVersion, SettingsProfileCodec::read(json).error);
}

#[test]
fn read_rejects_an_invalid_name() {
    for json_name in ["\"\"", "\"a\\u0007b\"", "5"] {
        let json = format!("{{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":{json_name},\"Sections\":[\"Dock\"],\"Settings\":{{}}}}");

        assert_eq!(ProfileImportError::InvalidName, SettingsProfileCodec::read(&json).error, "{json_name}");
    }
}

#[test]
fn read_rejects_unknown_or_machine_specific_sections() {
    for sections in ["[\"DockItems\"]", "[\"General\"]", "[\"Displays\"]", "[\"Weather\"]", "[\"Bogus\"]", "[5]"] {
        let json = format!("{{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":{sections},\"Settings\":{{}}}}");

        assert_eq!(ProfileImportError::UnknownSection, SettingsProfileCodec::read(&json).error, "{sections}");
    }
}

#[test]
fn read_rejects_an_empty_section_list() {
    let json = "{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[],\"Settings\":{}}";

    assert_eq!(ProfileImportError::NoSections, SettingsProfileCodec::read(json).error);
}

#[test]
fn read_rejects_an_invalid_value_in_a_declared_section() {
    let json = "{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[\"Dock\"],\"Settings\":{\"Dock\":{\"Style\":\"Bogus\"}}}";

    assert_eq!(ProfileImportError::InvalidValues, SettingsProfileCodec::read(json).error);
}

#[test]
fn read_ignores_an_invalid_value_in_a_section_the_file_does_not_declare() {
    let json = "{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[\"Dock\"],\"Settings\":{\"Dock\":{},\"General\":{\"Language\":5}}}";

    let result = SettingsProfileCodec::read(json);

    assert!(result.is_valid());
    assert_eq!(DockSettings::default(), result.profile.unwrap().settings.dock);
}

#[test]
fn read_ignores_unknown_keys_inside_a_declared_section() {
    let json = "{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[\"Dock\"],\"Settings\":{\"Dock\":{\"Gelecek\":true}}}";

    let result = SettingsProfileCodec::read(json);

    assert!(result.is_valid());
    assert_eq!(DockSettings::default(), result.profile.unwrap().settings.dock);
}

#[test]
fn read_fills_a_declared_section_missing_from_the_settings_with_defaults() {
    let json = "{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[\"Music\"],\"Settings\":{}}";

    assert_eq!(MusicSettings::default(), profile(json).settings.music);
}

#[test]
fn read_rejects_text_over_the_length_limit_before_parsing() {
    let oversized = " ".repeat(SettingsProfileCodec::MAX_LENGTH as usize + 1);

    assert_eq!(ProfileImportError::TooLarge, SettingsProfileCodec::read(&oversized).error);
}

#[test]
fn apply_changes_only_the_chosen_sections_the_file_declares() {
    let p = profile(&SettingsProfileCodec::export(&source(), "P", &[ProfileSection::Appearance, ProfileSection::Dock]));
    let current = AppSettings { appearance: AppearanceSettings { theme: AppTheme::Dark, ..Default::default() }, ..Default::default() };

    let applied = SettingsProfileCodec::apply(&current, &p, &[ProfileSection::Appearance]);

    assert_eq!(AppTheme::Light, applied.appearance.theme);
    assert_eq!(current.dock, applied.dock);
    assert_eq!(current.general, applied.general);
}

#[test]
fn apply_does_not_import_a_section_the_file_does_not_declare() {
    let p = profile(&SettingsProfileCodec::export(&source(), "P", &[ProfileSection::Appearance]));
    let current = AppSettings::default();

    let applied = SettingsProfileCodec::apply(&current, &p, &[ProfileSection::Appearance, ProfileSection::Dock]);

    assert_eq!(current.dock, applied.dock);
    assert_eq!(AppTheme::Light, applied.appearance.theme);
}

#[test]
fn apply_keeps_the_local_start_icon_path() {
    let p = profile(&SettingsProfileCodec::export(&source(), "P", &[ProfileSection::Dock]));
    let current = AppSettings { dock: DockSettings { start_icon_path: r"D:\yerel.png".to_string(), ..Default::default() }, ..Default::default() };

    let applied = SettingsProfileCodec::apply(&current, &p, &[ProfileSection::Dock]);

    assert_eq!(DockStyleMode::FullWidth, applied.dock.style);
    assert_eq!(r"D:\yerel.png", applied.dock.start_icon_path);
}

#[test]
fn export_clears_live_background_file_path_but_keeps_other_live_background_choices() {
    let mut src = source();
    src.notch.live_background_target = LiveBackgroundTarget::Both;
    src.notch.live_background_source = LiveBackgroundSourceKind::LocalFile;
    src.notch.live_background_file_path = r"C:\Users\ornek\loop.gif".to_string();

    let json = SettingsProfileCodec::export(&src, "P", &[ProfileSection::Notch]);

    let n = node(&json);
    assert_eq!("", n["Settings"]["Notch"]["LiveBackgroundFilePath"].as_str().unwrap());
    assert_eq!("Both", n["Settings"]["Notch"]["LiveBackgroundTarget"].as_str().unwrap());
}

#[test]
fn apply_keeps_the_local_live_background_file_path() {
    let mut src = source();
    src.notch.live_background_target = LiveBackgroundTarget::Notch;
    let p = profile(&SettingsProfileCodec::export(&src, "P", &[ProfileSection::Notch]));
    let current = AppSettings {
        notch: NotchSettings { live_background_file_path: r"D:\yerel.gif".to_string(), ..Default::default() },
        ..Default::default()
    };

    let applied = SettingsProfileCodec::apply(&current, &p, &[ProfileSection::Notch]);

    assert_eq!(LiveBackgroundTarget::Notch, applied.notch.live_background_target);
    assert_eq!(r"D:\yerel.gif", applied.notch.live_background_file_path);
}

#[test]
fn export_clears_local_app_icon_override_paths_from_the_file() {
    let mut src = source();
    src.appearance.app_icon_overrides.insert("notepad", r"C:\Users\ornek\not.png".to_string());

    let json = SettingsProfileCodec::export(&src, "Sunum", &[ProfileSection::Appearance]);

    assert!(!json.contains("not.png"));
    assert!(!json.contains("ornek"));
}

#[test]
fn export_clears_the_local_icon_pack_folder_and_apply_keeps_the_local_one() {
    let mut src = source();
    src.appearance.icon_pack = IconPack::Folder;
    src.appearance.icon_pack_folder = r"C:\Users\ornek\Paket".to_string();

    let json = SettingsProfileCodec::export(&src, "Sunum", &[ProfileSection::Appearance]);
    assert!(!json.contains("ornek"));

    let current = AppSettings {
        appearance: AppearanceSettings { icon_pack_folder: r"D:\yerel".to_string(), ..Default::default() },
        ..Default::default()
    };
    let applied = SettingsProfileCodec::apply(&current, &profile(&json), &[ProfileSection::Appearance]);

    assert_eq!(IconPack::Folder, applied.appearance.icon_pack);
    assert_eq!(r"D:\yerel", applied.appearance.icon_pack_folder);
}

#[test]
fn apply_keeps_the_local_app_icon_overrides() {
    let p = profile(&SettingsProfileCodec::export(&source(), "P", &[ProfileSection::Appearance]));
    let mut current = AppSettings::default();
    current.appearance.app_icon_overrides.insert("notepad", r"D:\yerel.png".to_string());

    let applied = SettingsProfileCodec::apply(&current, &p, &[ProfileSection::Appearance]);

    assert_eq!(AppTheme::Light, applied.appearance.theme);
    assert_eq!(r"D:\yerel.png", applied.appearance.app_icon_overrides["notepad"]);
}

#[test]
fn apply_does_not_mutate_the_current_settings() {
    let p = profile(&SettingsProfileCodec::export(&source(), "P", &[ProfileSection::Appearance]));
    let current = AppSettings::default();
    let snapshot = current.clone();

    let _ = SettingsProfileCodec::apply(&current, &p, &[ProfileSection::Appearance]);

    assert_eq!(snapshot, current);
}

#[test]
fn changed_sections_lists_declared_sections_that_differ_and_is_empty_after_apply() {
    let p = profile(&SettingsProfileCodec::export(&source(), "P", &[ProfileSection::Appearance, ProfileSection::Dock]));
    let current = AppSettings::default();

    assert_eq!(vec![ProfileSection::Appearance, ProfileSection::Dock], SettingsProfileCodec::changed_sections(&current, &p));

    let applied = SettingsProfileCodec::apply(&current, &p, &[ProfileSection::Appearance, ProfileSection::Dock]);

    assert!(SettingsProfileCodec::changed_sections(&applied, &p).is_empty());
}

#[test]
fn changed_sections_ignores_sections_the_file_does_not_declare() {
    let p = profile(&SettingsProfileCodec::export(&source(), "P", &[ProfileSection::Music]));

    assert_eq!(vec![ProfileSection::Music], SettingsProfileCodec::changed_sections(&AppSettings::default(), &p));
}

// ---- Tolerantlı okumanın kaçırdığı değerler ----

fn profile_json(section: &str, section_json: &str) -> String {
    format!(
        "{{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[\"{section}\"],\"Settings\":{{\"{section}\":{section_json}}}}}"
    )
}

/// Taşınabilir bölümlerin (`ProfileSection`) alan görünümleri: yeni alanı doğrulamaya eklemeyen olursa test düşer.
fn portable_fields() -> Vec<(&'static str, FieldView)> {
    let d = AppSettings::default();
    let views = d.section_views(&d);
    let mut result = Vec::new();
    for section in ProfileSection::ALL {
        let view = views.iter().find(|v| v.name == section.name()).unwrap();
        for field in &view.fields {
            result.push((section.name(), field.clone()));
        }
    }
    result
}

#[test]
fn read_rejects_an_undefined_enum_number_in_every_portable_enum_field() {
    let mut checked_fields = 0;
    for (section, field) in portable_fields() {
        if field.kind != Some(AdvancedSettingKind::Choice) {
            continue;
        }
        for value in ["999", "-1", "\"999\""] {
            let result = SettingsProfileCodec::read(&profile_json(section, &format!("{{\"{}\":{value}}}", field.name)));
            assert_eq!(ProfileImportError::InvalidValues, result.error, "{section}.{}={value} kabul edildi", field.name);
        }
        checked_fields += 1;
    }
    assert!(checked_fields >= 14, "{checked_fields}");
}

#[test]
fn read_accepts_every_default_value_of_every_portable_section() {
    let json = SettingsProfileCodec::export(&AppSettings::default(), "P", &ProfileSection::ALL);

    assert!(SettingsProfileCodec::read(&json).is_valid());
}

#[test]
fn read_rejects_an_infinite_number_in_every_portable_double_field() {
    let mut checked_fields = 0;
    for (section, field) in portable_fields() {
        if field.kind != Some(AdvancedSettingKind::Number) {
            continue;
        }
        // 1e400 System.Text.Json'da hata vermeden +Infinity okunur.
        let result = SettingsProfileCodec::read(&profile_json(section, &format!("{{\"{}\":1e400}}", field.name)));
        assert_eq!(ProfileImportError::InvalidValues, result.error, "{section}.{} sonsuz değeri kabul etti", field.name);
        checked_fields += 1;
    }
    assert!(checked_fields >= 2, "{checked_fields}");
}

#[test]
fn read_rejects_oversized_lists_and_null_items_in_every_portable_list_field() {
    let oversized = format!("[{}]", vec!["\"x\""; SettingsProfileCodec::MAX_LIST_ITEMS as usize + 1].join(","));
    let mut checked_fields = 0;
    for (section, field) in portable_fields() {
        if !field.is_string_list {
            continue;
        }
        for list in [oversized.as_str(), "[null,\"home\"]"] {
            let result = SettingsProfileCodec::read(&profile_json(section, &format!("{{\"{}\":{list}}}", field.name)));
            assert_eq!(ProfileImportError::InvalidValues, result.error, "{section}.{} geçersiz listeyi kabul etti", field.name);
        }
        checked_fields += 1;
    }
    assert!(checked_fields >= 2, "{checked_fields}");
}

#[test]
fn read_accepts_a_list_at_the_item_limit() {
    let list = format!("[{}]", vec!["\"home\""; SettingsProfileCodec::MAX_LIST_ITEMS as usize].join(","));

    assert!(SettingsProfileCodec::read(&profile_json("Notch", &format!("{{\"EnabledPanels\":{list}}}"))).is_valid());
}

#[test]
fn read_rejects_pomodoro_minutes_outside_the_runtime_range() {
    for minutes in ["0", "-3", "181", "2147483647"] {
        for field in ["PomodoroWorkMinutes", "PomodoroShortBreakMinutes", "PomodoroLongBreakMinutes"] {
            assert_eq!(
                ProfileImportError::InvalidValues,
                SettingsProfileCodec::read(&profile_json("Focus", &format!("{{\"{field}\":{minutes}}}"))).error,
                "{field}={minutes}"
            );
        }
    }
}

#[test]
fn read_accepts_pomodoro_minutes_at_the_range_edges() {
    for minutes in ["1", "180"] {
        assert!(SettingsProfileCodec::read(&profile_json("Focus", &format!("{{\"PomodoroWorkMinutes\":{minutes}}}"))).is_valid(), "{minutes}");
    }
}

#[test]
fn read_rejects_an_unparsable_reserved_or_duplicate_shortcut() {
    let cases = [
        "{\"ToggleNotch\":\"zzz\"}",
        "{\"ToggleNotch\":\"Win+L\"}",
        "{\"ToggleNotch\":\"Ctrl+Alt+Del\"}",
        "{\"ToggleNotch\":\"Ctrl+Alt+R\"}",
        "{\"ToggleNotch\":\"Ctrl+Alt+X\",\"OpenShelf\":\"ctrl+alt+x\"}",
    ];
    for shortcuts in cases {
        assert_eq!(ProfileImportError::InvalidValues, SettingsProfileCodec::read(&profile_json("Shortcuts", shortcuts)).error, "{shortcuts}");
    }
}

#[test]
fn read_accepts_cleared_and_valid_shortcuts() {
    let result = SettingsProfileCodec::read(&profile_json("Shortcuts", "{\"ToggleNotch\":\"\",\"OpenShelf\":\"Ctrl+Shift+F9\"}"));

    assert!(result.is_valid());
    assert_eq!("", result.profile.unwrap().settings.shortcuts.toggle_notch);
}

#[test]
fn read_does_not_validate_values_of_sections_the_file_does_not_declare() {
    let json = "{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[\"Music\"],\"Settings\":{\"Dock\":{\"Style\":999},\"Shortcuts\":{\"ToggleNotch\":\"zzz\"}}}";

    assert!(SettingsProfileCodec::read(json).is_valid());
}

#[test]
fn read_rejects_duplicate_keys_without_throwing() {
    let cases = [
        "{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Name\":\"A\",\"Sections\":[\"Dock\"],\"Settings\":{\"Dock\":{},\"Dock\":{}}}",
        "{\"Format\":\"koy-profile\",\"FormatVersion\":1,\"Format\":\"koy-profile\",\"Name\":\"A\",\"Sections\":[\"Dock\"],\"Settings\":{}}",
    ];
    for json in cases {
        assert_eq!(ProfileImportError::MalformedJson, SettingsProfileCodec::read(json).error, "{json}");
    }
}

#[test]
fn names_with_line_separators_or_bidi_overrides_are_invalid() {
    for code_point in [0x202E_u32, 0x2066, 0x2028, 0x2029] {
        let name = format!("a{}b", char::from_u32(code_point).unwrap());
        assert!(!SettingsProfileCodec::is_valid_name(Some(&name)), "{code_point:X}");
    }
}

#[test]
fn names_with_emoji_joiners_stay_valid() {
    // Erkek + ZWJ (U+200D) + dizüstü bilgisayar: tek görünen emoji; ZWJ yasak sayılmamalı.
    let name = format!("{}{}{} Is", char::from_u32(0x1F468).unwrap(), '\u{200D}', char::from_u32(0x1F4BB).unwrap());

    assert!(SettingsProfileCodec::is_valid_name(Some(&name)));
}

#[test]
fn apply_result_shares_no_mutable_state_with_the_profile() {
    let p = profile(&SettingsProfileCodec::export(&source(), "P", &[ProfileSection::Notch, ProfileSection::Appearance]));
    let mut first = SettingsProfileCodec::apply(&AppSettings::default(), &p, &[ProfileSection::Notch, ProfileSection::Appearance]);

    // Uygulama ayarları yerinde değiştirir (ControlCenterViewModel): profil etkilenmemeli.
    first.notch.control_center_layout = ValueList::from(["donotdisturb"]);
    first.notch.enabled_panels.clear();
    first.notch.enabled = !first.notch.enabled;
    first.appearance.theme = AppTheme::Dark;

    assert_eq!(NotchSettings::default(), p.settings.notch);
    assert_eq!(AppTheme::Light, p.settings.appearance.theme);
    let second = SettingsProfileCodec::apply(&AppSettings::default(), &p, &[ProfileSection::Notch]);
    assert_eq!(NotchSettings::default(), second.notch);
}

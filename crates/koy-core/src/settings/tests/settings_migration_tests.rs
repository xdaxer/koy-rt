//! `SettingsMigrationTests.cs`.

use serde_json::Value;

use super::{FIXTURE_V3, TempDir, load_json};
use crate::display::{MonitorSettings, NewScreenMode};
use crate::settings::*;

#[test]
fn version2_primary_or_all_becomes_all_screens_open() {
    for target in ["Primary", "All", ""] {
        let migrated = load_json(&format!("{{ \"Version\": 2, \"MonitorTarget\": \"{target}\" }}"));

        assert_eq!(AppSettings::CURRENT_VERSION, migrated.version, "{target}");
        assert!(migrated.displays.monitors.is_empty(), "{target}");
        assert_eq!(NewScreenMode::NotchAndDock, migrated.displays.new_screen_default, "{target}");
    }
}

#[test]
fn version2_specific_monitor_keeps_only_that_screen_open() {
    let migrated = load_json(r#"{ "Version": 2, "MonitorTarget": "\\\\.\\DISPLAY2" }"#);

    assert!(migrated.displays.monitors[r"\\.\DISPLAY2"].notch_enabled);
    assert!(migrated.displays.monitors[r"\\.\DISPLAY2"].dock_enabled);
    assert_eq!(NewScreenMode::None, migrated.displays.new_screen_default);
}

#[test]
fn version2_specific_monitor_replaces_an_existing_record_of_the_same_screen() {
    let migrated = load_json(
        r#"
        { "Version": 2, "MonitorTarget": "ofis",
          "Monitors": { "OFIS": { "NotchEnabled": false, "DockEnabled": false, "Name": "Eski" } } }
        "#,
    );

    assert_eq!(1, migrated.displays.monitors.len());
    let (_, record) = migrated.displays.monitors.iter().next().unwrap();
    assert!(record.notch_enabled && record.dock_enabled);
}

#[test]
fn current_version_is_untouched() {
    let Value::Object(mut root) =
        serde_json::from_str::<Value>(r#"{ "Version": 4, "MonitorTarget": "Elle", "NewScreenDefault": "NotchOnly" }"#).unwrap()
    else {
        panic!("nesne bekleniyordu");
    };

    let original = SettingsMigration::migrate(&mut root);

    assert_eq!(AppSettings::CURRENT_VERSION, original);
    assert_eq!(Some("Elle"), root.get("MonitorTarget").and_then(Value::as_str));
    assert_eq!(Some("NotchOnly"), root.get("NewScreenDefault").and_then(Value::as_str));
    assert!(root.get("Monitors").is_none());
}

#[test]
fn missing_version_is_treated_as_the_pre_v4_default_and_returns_original_version() {
    let Value::Object(mut root) = serde_json::from_str::<Value>(r#"{ "DockEnabled": true }"#).unwrap() else {
        panic!("nesne bekleniyordu");
    };

    assert_eq!(3, SettingsMigration::migrate(&mut root));
}

#[test]
fn migration_preserves_other_settings() {
    let migrated = load_json(r#"{ "Version": 2, "Language": "en", "DockIconSize": 55 }"#);

    assert_eq!("en", migrated.general.language);
    assert_eq!(55.0, migrated.dock.icon_size);
}

#[test]
fn store_migrates_a_version2_file_on_load() {
    let dir = TempDir::new("koy-mig-");
    let path = dir.file("settings.json");
    std::fs::write(&path, "{ \"Version\": 2, \"MonitorTarget\": \"\\\\\\\\.\\\\DISPLAY2\" }").unwrap();

    let loaded = SettingsStore::new(&path).load();

    assert_eq!(AppSettings::CURRENT_VERSION, loaded.version);
    assert!(loaded.displays.monitors.contains_key(r"\\.\DISPLAY2"));
}

#[test]
fn monitor_settings_round_trip_through_json_including_disconnected_ones() {
    let dir = TempDir::new("koy-mig-");
    let store = SettingsStore::new(dir.file("settings.json"));
    let mut monitors = ValueDictionary::new();
    monitors.insert("kimlik-1", MonitorSettings { notch_enabled: true, dock_enabled: false, name: Some("Ofis".to_string()) });
    monitors.insert("kimlik-2", MonitorSettings { notch_enabled: false, dock_enabled: true, name: None });
    let settings = AppSettings {
        displays: DisplaysSettings { new_screen_default: NewScreenMode::None, monitors },
        ..Default::default()
    };
    store.save(&settings).unwrap();

    let loaded = store.load();

    assert_eq!(settings, loaded);
    assert_eq!(Some("Ofis".to_string()), loaded.displays.monitors["kimlik-1"].name);
    assert_eq!(NewScreenMode::None, loaded.displays.new_screen_default);
}

#[test]
fn golden_v3_fixture_has_every_field_different_from_the_default() {
    // Altın dosya her alanı varsayılandan farklı tutmalı; yeni alan eklenirse dosya da güncellenmeli.
    let loaded = load_json(FIXTURE_V3);
    let defaults = AppSettings::default();

    // Version zaten güncel; Language varsayılanı makinenin diline bağlı (aynı değere denk gelebilir). Bölümler bütün
    // olarak karşılaştırılır (C# yansımasıyla aynı: her AppSettings özelliği).
    let sections = [
        ("General", loaded.general != defaults.general),
        ("Appearance", loaded.appearance != defaults.appearance),
        ("Dock", loaded.dock != defaults.dock),
        ("DockItems", loaded.dock_items != defaults.dock_items),
        ("Notch", loaded.notch != defaults.notch),
        ("Music", loaded.music != defaults.music),
        ("Displays", loaded.displays != defaults.displays),
        ("Shortcuts", loaded.shortcuts != defaults.shortcuts),
        ("HotCorners", loaded.hot_corners != defaults.hot_corners),
        ("Voice", loaded.voice != defaults.voice),
        ("Search", loaded.search != defaults.search),
        ("Focus", loaded.focus != defaults.focus),
        ("Weather", loaded.weather != defaults.weather),
        ("Clipboard", loaded.clipboard != defaults.clipboard),
        ("QuickLaunch", loaded.quick_launch != defaults.quick_launch),
    ];
    for (name, different) in sections {
        assert!(different, "{name} altın dosyada varsayılan değerde.");
    }
}

#[test]
fn golden_v3_fixture_survives_migration_save_and_reload_unchanged() {
    let dir = TempDir::new("koy-mig-");
    let path = dir.file("settings.json");
    std::fs::write(&path, FIXTURE_V3).unwrap();
    let store = SettingsStore::new(&path);
    let migrated = store.load();

    store.save(&migrated).unwrap();

    // Yazılan dosya v4'tür ve yeniden okununca hiçbir değer kaybolmaz ya da kaymaz.
    assert_eq!(AppSettings::CURRENT_VERSION, migrated.version);
    assert_eq!(migrated, store.load());
}

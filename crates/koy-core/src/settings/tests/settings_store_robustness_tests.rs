//! `Settings/SettingsStoreRobustnessTests.cs`: bozuk ya da eksik dosyalar, göç yedeği ve kayıt/okuma döngüsü.

use super::{FIXTURE_V3, TempDir, load_json};
use crate::dock::{DockItemKind, DockItemRules};
use crate::notch::HomeCard;
use crate::settings::*;

#[test]
fn dock_items_beyond_max_item_count_are_cut_on_load_keeping_order() {
    // 40 öğe: yüklenen model yalnız ilk 32'yi, aynı sırayla tutmalı (Normalize çağrılmadan da).
    let items: Vec<String> = (0..40).map(|i| format!(r#"{{ "Kind": "app", "Id": "app{i}.lnk" }}"#)).collect();
    let loaded = load_json(&format!(r#"{{ "Version": 4, "DockItems": {{ "Items": [{}] }} }}"#, items.join(",")));

    let max = DockItemRules::MAX_ITEM_COUNT as usize;
    assert_eq!(max, loaded.dock_items.items.len());
    let expected: Vec<String> = (0..max).map(|i| format!("app{i}.lnk")).collect();
    let actual: Vec<String> = loaded.dock_items.items.iter().map(|e| e.id.clone()).collect();
    assert_eq!(expected, actual);
}

#[test]
fn unknown_value_in_an_enum_list_drops_only_that_value() {
    let loaded = load_json(
        r#"
        { "Version": 4, "Notch": { "HideInFullscreen": false, "HomeCards": ["Weather", "Gelecek", "Music"] } }
        "#,
    );

    assert!(!loaded.notch.hide_in_fullscreen);
    assert_eq!(vec![HomeCard::Weather, HomeCard::Music], loaded.notch.home_cards);
}

#[test]
fn null_dock_item_entries_are_dropped_before_the_cap() {
    // Boş girdiler geçerli öğenin yerini yememeli; listede null kalırsa okuyan her tüketici çöker.
    let nulls = vec!["null"; DockItemRules::MAX_ITEM_COUNT as usize].join(",");
    let loaded = load_json(&format!(r#"{{ "Version": 4, "DockItems": {{ "Items": [{nulls}, {{ "Kind": "app", "Id": "a.lnk" }}, null] }} }}"#));

    assert_eq!(1, loaded.dock_items.items.len());
    assert_eq!("a.lnk", loaded.dock_items.items[0].id);
}

#[test]
fn dock_item_entry_with_null_fields_does_not_break_normalize() {
    let loaded = load_json(
        r#"{ "Version": 4, "DockItems": { "Items": [{ "Kind": null, "Id": null, "Command": null }, { "Kind": "spacer", "Id": null }] } }"#,
    );

    let specs = DockItemRules::normalize(&loaded.dock_items.items, DockItemRules::MAX_ITEM_COUNT);

    assert_eq!(1, specs.len());
    assert_eq!(DockItemKind::Spacer, specs[0].kind);
}

#[test]
fn null_sections_are_replaced_with_defaults() {
    let loaded = load_json(r#"{ "Version": 4, "Dock": null, "Notch": null, "DockItems": null }"#);

    assert!(loaded.dock.enabled);
    assert!(loaded.notch.enabled);
    assert!(loaded.dock_items.pinned_order.is_empty());
    assert!(loaded.dock_items.folders.is_empty());
}

#[test]
fn null_lists_inside_sections_become_empty_lists() {
    let loaded = load_json(r#"{ "Version": 4, "DockItems": { "PinnedOrder": null, "Folders": null }, "QuickLaunch": { "Items": null } }"#);

    assert!(loaded.dock_items.pinned_order.is_empty());
    assert!(loaded.dock_items.folders.is_empty());
    assert!(loaded.quick_launch.items.is_empty());
}

#[test]
fn unknown_top_level_key_is_ignored_without_losing_other_values() {
    let loaded = load_json(r#"{ "Version": 4, "Gelecek": 1, "Dock": { "Enabled": false } }"#);

    assert!(!loaded.dock.enabled);
    assert!(loaded.dock.hide_windows_taskbar);
}

#[test]
fn migration_backup_is_written_once_and_never_overwritten() {
    let dir = TempDir::new("koy-robust-");
    let path = dir.file("settings.json");
    let original = FIXTURE_V3;
    std::fs::write(&path, original).unwrap();

    SettingsStore::new(&path).load();
    let backup = SettingsStore::new(&path).backup_path().to_path_buf();
    assert!(backup.exists());
    assert_eq!(original, std::fs::read_to_string(&backup).unwrap());

    // Yeni bir v3 dosyası gelse de (örn. eski sürüm yeniden yazdı) mevcut yedek üzerine yazılmaz.
    std::fs::write(&path, r#"{ "Version": 3, "Language": "en" }"#).unwrap();
    SettingsStore::new(&path).load();

    assert_eq!(original, std::fs::read_to_string(&backup).unwrap());
}

#[test]
fn no_backup_is_written_for_a_current_version_file() {
    let dir = TempDir::new("koy-robust-");
    let path = dir.file("settings.json");
    std::fs::write(&path, r#"{ "Version": 4 }"#).unwrap();

    SettingsStore::new(&path).load();

    assert!(!SettingsStore::new(&path).backup_path().exists());
}

#[test]
fn full_golden_settings_survive_save_and_load() {
    let dir = TempDir::new("koy-robust-");
    let path = dir.file("settings.json");
    let original = load_json(FIXTURE_V3);
    let store = SettingsStore::new(&path);

    store.save(&original).unwrap();
    let reloaded = store.load();

    assert_eq!(original, reloaded);
}

#[test]
fn saved_file_is_version_4_with_nested_sections() {
    let dir = TempDir::new("koy-robust-");
    let path = dir.file("settings.json");
    SettingsStore::new(&path).save(&load_json(FIXTURE_V3)).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"Version\": 4"));
    assert!(text.contains("\"Dock\": {"));
    let node: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(!node.as_object().unwrap().contains_key("DockEnabled"));
}

#[test]
fn invalid_enum_value_resets_only_its_own_field() {
    let loaded = load_json(r#"{ "Version": 4, "General": { "Language": "tr" }, "Dock": { "Enabled": false, "Style": "Bogus" } }"#);

    assert_eq!("tr", loaded.general.language);
    assert!(!loaded.dock.enabled);
    assert_eq!(DockSettings::default().style, loaded.dock.style);
}

#[test]
fn invalid_value_in_a_migrated_v3_file_keeps_the_other_settings() {
    let loaded = load_json(r#"{ "Version": 3, "Language": "tr", "NotchEnabled": false, "DockStyle": "Bogus" }"#);

    assert_eq!("tr", loaded.general.language);
    assert!(!loaded.notch.enabled);
    assert_eq!(DockSettings::default().style, loaded.dock.style);
}

#[test]
fn invalid_value_inside_a_dictionary_resets_that_dictionary_but_keeps_other_sections() {
    // Sınırlama: JsonException.Path sözlüğün kendisini gösterir (girdiyi değil), bu yüzden sözlüğün tamamı sıfırlanır.
    // Diğer bölümler (burada Language) korunur.
    let loaded = load_json(
        r#"{ "Version": 4, "General": { "Language": "tr" }, "Displays": { "Monitors": { "DISPLAY1.x": { "NotchEnabled": "yanlis" } } } }"#,
    );

    assert_eq!("tr", loaded.general.language);
    assert!(loaded.displays.monitors.is_empty());
}

#[test]
fn invalid_list_entry_drops_only_that_entry_and_keeps_the_section() {
    // Tek bozuk klasör kaydı, aynı bölümdeki sabitlenmiş sırayı ve diğer klasörleri silmemeli.
    let loaded = load_json(
        r#"
        { "Version": 4,
          "DockItems": { "PinnedOrder": ["code.exe"],
                         "Folders": [ { "Id": "a", "Name": "A", "Items": [] }, { "Id": 5, "Name": "B", "Items": [] }, { "Id": "c", "Name": "C", "Items": [] } ] } }
        "#,
    );

    assert_eq!(["code.exe"], loaded.dock_items.pinned_order);
    let ids: Vec<&str> = loaded.dock_items.folders.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(vec!["a", "c"], ids);
}

#[test]
fn invalid_value_inside_a_nested_list_drops_the_innermost_entry() {
    let loaded = load_json(
        r#"
        { "Version": 4,
          "DockItems": { "Folders": [ { "Id": "a", "Name": "A", "Items": ["x.exe", 7, "y.exe"] } ] } }
        "#,
    );

    assert_eq!(1, loaded.dock_items.folders.len());
    assert_eq!(vec!["x.exe".to_string(), "y.exe".to_string()], loaded.dock_items.folders[0].items);
}

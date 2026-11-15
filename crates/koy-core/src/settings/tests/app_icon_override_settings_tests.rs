//! `AppIconOverrideTests.cs` içinde Settings'e bağlı olan testler (appearance modülü bunları atlamıştı).

use super::{TempDir, load_json};
use crate::appearance::{AppIconOverrides, IconChoice, IconPack, IconResolver, IconSourceKind};
use crate::settings::*;

fn default_choice() -> IconChoice {
    IconChoice::new(IconSourceKind::Default, None)
}

#[test]
fn lookup_is_case_insensitive_on_the_key() {
    let mut map = ValueDictionary::<String>::new();
    assert!(AppIconOverrides::try_set(&mut map, "Notepad.lnk", "note.png"));

    assert_eq!(Some("note.png".to_string()), AppIconOverrides::lookup(Some(&map), Some("notepad.LNK")));
    assert_eq!(None, AppIconOverrides::lookup(Some(&map), Some("calc.lnk")));
}

#[test]
fn invalid_set_leaves_the_map_unchanged() {
    let mut map = ValueDictionary::<String>::new();
    map.insert("calc.lnk", "calc.png".to_string());

    assert!(!AppIconOverrides::try_set(&mut map, "calc.lnk", "run.exe"));
    assert!(!AppIconOverrides::try_set(&mut map, "", "ok.png"));

    assert_eq!(1, map.len());
    assert_eq!("calc.png", map["calc.lnk"]);
}

#[test]
fn reset_removes_the_override_regardless_of_key_case() {
    let mut map = ValueDictionary::<String>::new();
    AppIconOverrides::try_set(&mut map, "Notepad.lnk", "note.png");

    AppIconOverrides::reset(&mut map, "NOTEPAD.lnk");

    assert!(map.is_empty());
}

#[test]
fn lookup_ignores_a_stored_value_that_became_invalid() {
    // Elle düzenlenmiş settings.json: geçersiz değer haritada olsa bile çözümlemede yok sayılır.
    let mut map = ValueDictionary::<String>::new();
    map.insert("calc.lnk", r"C:\Tools\calc.exe".to_string());

    assert_eq!(None, AppIconOverrides::lookup(Some(&map), Some("calc.lnk")));
}

#[test]
fn resolve_prefers_app_override_then_pack_then_default() {
    let mut overrides = ValueDictionary::<String>::new();
    overrides.insert("notepad.lnk", "note.png".to_string());

    let app = IconResolver::resolve(Some("Notepad.lnk"), Some(&overrides), IconPack::Folder, Some("notepad.svg"));
    assert_eq!(IconChoice::new(IconSourceKind::AppOverride, Some("note.png".to_string())), app);

    let pack = IconResolver::resolve(Some("calc.lnk"), Some(&overrides), IconPack::Folder, Some("calc.svg"));
    assert_eq!(IconChoice::new(IconSourceKind::Pack, Some("calc.svg".to_string())), pack);

    let fallback = IconResolver::resolve(Some("calc.lnk"), Some(&overrides), IconPack::Folder, None);
    assert_eq!(default_choice(), fallback);
}

#[test]
fn null_app_key_skips_the_override_map() {
    let mut overrides = ValueDictionary::<String>::new();
    overrides.insert("calc.lnk", "calc.png".to_string());

    let choice = IconResolver::resolve(None, Some(&overrides), IconPack::Builtin, None);

    assert_eq!(default_choice(), choice);
}

#[test]
fn icon_settings_default_to_builtin_and_no_overrides() {
    let settings = AppSettings::default();

    assert_eq!(IconPack::Builtin, settings.appearance.icon_pack);
    assert!(settings.appearance.app_icon_overrides.is_empty());
}

fn with_overrides(entries: &[(&str, &str)]) -> AppSettings {
    let mut overrides = ValueDictionary::new();
    for (k, v) in entries {
        overrides.insert(k, v.to_string());
    }
    AppSettings { appearance: AppearanceSettings { app_icon_overrides: overrides, ..Default::default() }, ..Default::default() }
}

#[test]
fn override_maps_with_same_entries_in_any_case_and_order_are_equal() {
    let a = with_overrides(&[("A.lnk", "a.png"), ("b.lnk", "b.png")]);
    let b = with_overrides(&[("b.LNK", "b.png"), ("a.lnk", "a.png")]);
    let c = with_overrides(&[("a.lnk", "other.png"), ("b.lnk", "b.png")]);

    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn icon_settings_survive_a_save_and_load_round_trip() {
    let dir = TempDir::unique("koy-icon-tests-");
    let path = dir.file("settings.json");
    let store = SettingsStore::new(&path);
    let mut settings = with_overrides(&[("Notepad.lnk", r"C:\icons\note.png")]);
    settings.appearance.icon_pack = IconPack::Folder;

    store.save(&settings).unwrap();
    let loaded = SettingsStore::new(&path).load();

    assert_eq!(IconPack::Folder, loaded.appearance.icon_pack);
    assert_eq!(r"C:\icons\note.png", loaded.appearance.app_icon_overrides["notepad.lnk"]);
}

#[test]
fn null_override_map_in_the_file_is_restored_to_an_empty_map() {
    for json in [r#"{ "Appearance": { "AppIconOverrides": null } }"#, r#"{ "Appearance": null }"#] {
        let loaded = load_json(json);

        assert!(loaded.appearance.app_icon_overrides.is_empty(), "{json}");
        assert_eq!(None, AppIconOverrides::lookup(Some(&loaded.appearance.app_icon_overrides), Some("calc.lnk")));
    }
}

#[test]
fn undefined_icon_pack_number_resolves_to_the_default_icon() {
    // Elle yazılmış sayı ("IconPack": 7) enum'a olduğu gibi girer (C#); Rust'ta tanımsız sayı okunamaz, alan varsayılana
    // (Builtin) döner. Çözümleme paketi yok sayıp yerleşik simgeye düşmeli.
    let loaded = load_json(r#"{ "Appearance": { "IconPack": 7 } }"#);

    let choice = IconResolver::resolve(Some("calc.lnk"), None, loaded.appearance.icon_pack, Some("calc.svg"));

    assert_eq!(default_choice(), choice);
}

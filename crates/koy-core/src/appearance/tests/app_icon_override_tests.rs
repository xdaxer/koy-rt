//! Simge paketi, uygulama başına simge değişimi ve hangisinin kazandığı (AGENTS: dosya okuma ve çizim Core dışında).
//!
//! Atlananlar (Settings modülü taşınınca eklenecek; `AppSettings`/`SettingsStore` ister):
//! `Icon_settings_default_to_builtin_and_no_overrides`, `Override_maps_with_same_entries_in_any_case_and_order_are_equal`,
//! `Icon_settings_survive_a_save_and_load_round_trip`, `Null_override_map_in_the_file_is_restored_to_an_empty_map`,
//! `Undefined_icon_pack_number_resolves_to_the_default_icon`.
//! `Lookup_ignores_a_null_stored_value`: Rust sözlüğünde değer `String` (null olamaz), test anlamsız.

use crate::appearance::{AppIconOverrides, IconChoice, IconOverrideMap, IconPack, IconResolver, IconSourceKind};
use crate::text;

/// `ValueDictionary<string>` karşılığı test sözlüğü: anahtarlar büyük/küçük harf duyarsız, ekleme sırası korunur.
#[derive(Default)]
struct ValueDictionary(Vec<(String, String)>);

impl ValueDictionary {
    fn with(entries: &[(&str, &str)]) -> Self {
        let mut d = ValueDictionary::default();
        for (k, v) in entries {
            d.set_value(k, v.to_string());
        }
        d
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

impl IconOverrideMap for ValueDictionary {
    fn get_value(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| text::eq_ignore_case(k, key)).map(|(_, v)| v.as_str())
    }

    fn set_value(&mut self, key: &str, value: String) {
        match self.0.iter_mut().find(|(k, _)| text::eq_ignore_case(k, key)) {
            Some(entry) => entry.1 = value,
            None => self.0.push((key.to_string(), value)),
        }
    }

    fn remove_value(&mut self, key: &str) {
        self.0.retain(|(k, _)| !text::eq_ignore_case(k, key));
    }
}

fn default_choice() -> IconChoice {
    IconChoice::new(IconSourceKind::Default, None)
}

#[test]
fn key_must_be_a_plain_shortcut_name() {
    let rows: [(Option<&str>, bool); 9] = [
        (Some("Notepad.lnk"), true),
        (Some("  Chrome  "), true),
        (Some(""), false),
        (Some("   "), false),
        (None, false),
        (Some("a\\b.lnk"), false),
        (Some("a/b.lnk"), false),
        (Some("c:x"), false),
        (Some("bad\nname"), false),
    ];
    for (key, expected) in rows {
        assert_eq!(expected, AppIconOverrides::is_valid_key(key), "{key:?}");
    }
}

#[test]
fn key_length_is_capped() {
    let max = AppIconOverrides::MAX_LENGTH as usize;
    assert!(AppIconOverrides::is_valid_key(Some(&"a".repeat(max))));
    assert!(!AppIconOverrides::is_valid_key(Some(&"a".repeat(max + 1))));
}

#[test]
fn value_must_be_an_image_path_or_name_and_never_a_script() {
    let rows: Vec<(Option<&str>, bool, Option<&str>)> = vec![
        (Some("icon.png"), true, Some("icon.png")),
        (Some("ICON.SVG"), true, Some("ICON.SVG")),
        (Some("  icon.ico  "), true, Some("icon.ico")),
        (Some("C:\\Users\\me\\icons\\app.png"), true, Some("C:\\Users\\me\\icons\\app.png")),
        (Some("C:\\Users\\me\\..\\evil.png"), false, None),
        (Some("..\\icons\\app.png"), false, None),
        (Some("icons\\app.png"), false, None),
        (Some("C:\\app.exe"), false, None),
        (Some("C:\\run.ps1"), false, None),
        (Some("icon.png|calc"), false, None),
        (Some("//server/share/a.png"), false, None),
        (Some("\\\\server\\share\\a.png"), false, None),
        (Some("\\\\?\\C:\\a.png"), false, None),
        (Some("\\\\.\\C:\\a.png"), false, None),
        (Some("C:\\x.exe:evil.png"), false, None),
        (Some("C:\\a.png:x.png"), false, None),
        (Some("C:\\.\\a.png"), false, None),
        (Some("C:\\icons\\.\\app.png"), false, None),
        (Some("C:/icons/app.png"), false, None),
        (Some("C:app.png"), false, None),
        (Some("con.png"), false, None),
        (Some("NUL.svg"), false, None),
        (Some("com1.ico"), false, None),
        (Some("LPT9.tar.png"), false, None),
        (Some("COM\u{00B9}.png"), false, None),
        (Some("C:\\icons\\aux.png"), false, None),
        (Some("C:\\nul\\a.png"), false, None),
        (Some("C:\\icons.\\a.png"), false, None),
        (Some("C:\\icons \\a.png"), false, None),
        (Some("C:\\...\\a.png"), false, None),
        (Some("C:\\icons\\\\a.png"), false, None),
        (Some("console.png"), true, Some("console.png")),
        (Some("com10.png"), true, Some("com10.png")),
        (Some("my.con.png"), true, Some("my.con.png")),
        (Some("C:\\icons/mixed.png"), true, Some("C:\\icons/mixed.png")),
        (Some(""), false, None),
        (None, false, None),
    ];
    for (value, expected, normalized) in rows {
        let result = AppIconOverrides::try_validate_source(value);

        assert_eq!(expected, result.is_some(), "{value:?}");
        if expected {
            assert_eq!(normalized, result.as_deref(), "{value:?}");
        }
    }
}

#[test]
fn pack_entry_must_be_a_bare_file_name() {
    let rows = [
        ("calc.svg", true),
        ("C:\\icons\\calc.svg", false),
        ("\\\\server\\share\\calc.svg", false),
        ("//server/share/calc.svg", false),
        ("..\\calc.svg", false),
        ("icons/calc.svg", false),
        ("calc.svg:evil.png", false),
        ("C:\\icons\\.\\calc.svg", false),
    ];
    for (value, expected) in rows {
        assert_eq!(expected, AppIconOverrides::try_validate_pack_file(Some(value)).is_some(), "{value}");
    }
}

#[test]
fn device_named_pack_file_falls_back_to_default() {
    let choice = IconResolver::resolve(Some("calc.lnk"), None, IconPack::Folder, Some("nul.png"));

    assert_eq!(default_choice(), choice);
}

#[test]
fn lookup_is_case_insensitive_on_the_key() {
    let mut map = ValueDictionary::default();
    assert!(AppIconOverrides::try_set(&mut map, "Notepad.lnk", "note.png"));

    assert_eq!(Some("note.png".to_string()), AppIconOverrides::lookup(Some(&map), Some("notepad.LNK")));
    assert_eq!(None, AppIconOverrides::lookup(Some(&map), Some("calc.lnk")));
}

#[test]
fn invalid_set_leaves_the_map_unchanged() {
    let mut map = ValueDictionary::with(&[("calc.lnk", "calc.png")]);

    assert!(!AppIconOverrides::try_set(&mut map, "calc.lnk", "run.exe"));
    assert!(!AppIconOverrides::try_set(&mut map, "", "ok.png"));

    assert_eq!(1, map.len());
    assert_eq!(Some("calc.png"), map.get_value("calc.lnk"));
}

#[test]
fn reset_removes_the_override_regardless_of_key_case() {
    let mut map = ValueDictionary::default();
    AppIconOverrides::try_set(&mut map, "Notepad.lnk", "note.png");

    AppIconOverrides::reset(&mut map, "NOTEPAD.lnk");

    assert_eq!(0, map.len());
}

#[test]
fn lookup_ignores_a_stored_value_that_became_invalid() {
    // Elle düzenlenmiş settings.json: geçersiz değer haritada olsa bile çözümlemede yok sayılır.
    let map = ValueDictionary::with(&[("calc.lnk", "C:\\Tools\\calc.exe")]);

    assert_eq!(None, AppIconOverrides::lookup(Some(&map), Some("calc.lnk")));
}

#[test]
fn resolve_prefers_app_override_then_pack_then_default() {
    let overrides = ValueDictionary::with(&[("notepad.lnk", "note.png")]);

    let app = IconResolver::resolve(Some("Notepad.lnk"), Some(&overrides), IconPack::Folder, Some("notepad.svg"));
    assert_eq!(IconChoice::new(IconSourceKind::AppOverride, Some("note.png".to_string())), app);

    let pack = IconResolver::resolve(Some("calc.lnk"), Some(&overrides), IconPack::Folder, Some("calc.svg"));
    assert_eq!(IconChoice::new(IconSourceKind::Pack, Some("calc.svg".to_string())), pack);

    let fallback = IconResolver::resolve(Some("calc.lnk"), Some(&overrides), IconPack::Folder, None);
    assert_eq!(default_choice(), fallback);
}

#[test]
fn pack_entry_with_absolute_path_falls_back_to_default() {
    let choice = IconResolver::resolve(Some("calc.lnk"), None, IconPack::Folder, Some("C:\\Tools\\calc.svg"));

    assert_eq!(default_choice(), choice);
}

#[test]
fn builtin_pack_ignores_the_pack_file() {
    let choice = IconResolver::resolve(Some("calc.lnk"), None, IconPack::Builtin, Some("calc.svg"));

    assert_eq!(default_choice(), choice);
}

#[test]
fn invalid_pack_file_falls_back_to_default() {
    let choice = IconResolver::resolve(Some("calc.lnk"), None, IconPack::Folder, Some("..\\calc.svg"));

    assert_eq!(default_choice(), choice);
}

#[test]
fn null_app_key_skips_the_override_map() {
    let overrides = ValueDictionary::with(&[("calc.lnk", "calc.png")]);

    let choice = IconResolver::resolve(None, Some(&overrides), IconPack::Builtin, None);

    assert_eq!(default_choice(), choice);
}

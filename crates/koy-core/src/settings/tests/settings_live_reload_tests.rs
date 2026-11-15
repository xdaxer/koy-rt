//! `Settings/SettingsLiveReloadTests.cs`: canlı yeniden yükleme için katı okuma ve kendi kaydını tanıma (issue #69).

use std::time::{Duration, SystemTime};

use super::TempDir;
use crate::settings::*;

fn new_store(dir: &TempDir) -> SettingsStore {
    SettingsStore::new(dir.file("settings.json"))
}

#[test]
fn own_save_is_recognized_and_external_edit_is_not() {
    let dir = TempDir::unique("koy-live-");
    let store = new_store(&dir);
    store.save(&AppSettings::default()).unwrap();
    assert!(store.is_own_write());
    // Aynı dosyaya bakan başka bir örnek de kendi kaydını tanır (yol başına tutulur).
    assert!(new_store(&dir).is_own_write());

    let path = dir.file("settings.json");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text).unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
    drop(file);

    assert!(!store.is_own_write());
}

#[test]
fn strict_read_accepts_a_valid_current_file() {
    let dir = TempDir::unique("koy-live-");
    let store = new_store(&dir);
    let settings = AppSettings { general: GeneralSettings { language: "en".to_string(), ..Default::default() }, ..Default::default() };
    store.save(&settings).unwrap();

    let read = store.try_read_strict().expect("geçerli dosya");
    assert_eq!("en", read.general.language);
}

#[test]
fn strict_read_rejects_broken_unknown_or_old_files() {
    let cases = [
        "{ not json",
        "[]",
        r#"{ "Version": 4, "Dock": { "Style": "Bogus" } }"#,
        r#"{ "Version": 3, "DockEnabled": true }"#,
    ];
    for json in cases {
        let dir = TempDir::new("koy-live-");
        std::fs::write(dir.file("settings.json"), json).unwrap();

        assert!(new_store(&dir).try_read_strict().is_none(), "{json}");
    }
}

#[test]
fn strict_read_of_missing_file_is_false() {
    let dir = TempDir::unique("koy-live-");
    assert!(new_store(&dir).try_read_strict().is_none());
}

//! `Settings/AppLanguageTests.cs`.

use super::TempDir;
use crate::settings::*;

#[test]
fn first_run_language_follows_the_system() {
    let cases = [("tr-TR", "tr"), ("tr", "tr"), ("en-US", "en"), ("de-DE", "en"), ("ar-SA", "en")];
    for (culture, expected) in cases {
        assert_eq!(expected, AppLanguage::from_culture(culture), "{culture}");
    }
}

#[test]
fn saved_language_is_kept() {
    for saved in ["tr", "en"] {
        let dir = TempDir::unique("koy-lang-");
        let store = SettingsStore::new(dir.file("settings.json"));
        store.save(&AppSettings { general: GeneralSettings { language: saved.to_string(), ..Default::default() }, ..Default::default() }).unwrap();

        assert_eq!(saved, store.load().general.language);
    }
}

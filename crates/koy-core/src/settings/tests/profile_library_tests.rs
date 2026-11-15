//! `Settings/ProfileLibraryTests.cs`.

use super::TempDir;
use crate::appearance::AppTheme;
use crate::settings::*;

fn source() -> AppSettings {
    AppSettings {
        appearance: AppearanceSettings { theme: AppTheme::Light, corner_radius: 17.0, ..Default::default() },
        music: MusicSettings { show_waveform: false, ..Default::default() },
        ..Default::default()
    }
}

fn one(name: &str, id: &str) -> Vec<SavedProfile> {
    let result = ProfileLibrary::create(&[], &source(), name, &[ProfileSection::Appearance, ProfileSection::Music], id);
    assert!(result.is_success());
    result.profiles
}

#[test]
fn create_stores_a_valid_codec_text_with_the_chosen_sections() {
    let list = one("Work", "a");

    assert_eq!(1, list.len());
    let saved = &list[0];
    assert_eq!("a", saved.id);
    assert_eq!("Work", saved.name);
    assert_eq!(vec![ProfileSection::Appearance, ProfileSection::Music], ProfileLibrary::sections_of(saved));
}

#[test]
fn create_rejects_invalid_names() {
    for name in ["", "   ", "bad\nname"] {
        let result = ProfileLibrary::create(&[], &source(), name, &[ProfileSection::Dock], "x");

        assert_eq!(ProfileLibraryError::InvalidName, result.error, "{name:?}");
        assert!(result.profiles.is_empty());
    }
}

#[test]
fn create_rejects_no_sections() {
    assert_eq!(ProfileLibraryError::NoSections, ProfileLibrary::create(&[], &source(), "A", &[], "x").error);
}

#[test]
fn create_rejects_a_duplicate_name_ignoring_case() {
    let result = ProfileLibrary::create(&one("Work", "a"), &source(), " work ", &[ProfileSection::Dock], "b");

    assert_eq!(ProfileLibraryError::DuplicateName, result.error);
    assert_eq!(1, result.profiles.len());
}

#[test]
fn create_stops_at_the_profile_limit() {
    let full: Vec<SavedProfile> =
        (0..ProfileLibrary::MAX_PROFILES).map(|i| SavedProfile::new(&format!("id{i}"), &format!("P{i}"), "x")).collect();

    assert_eq!(ProfileLibraryError::LimitReached, ProfileLibrary::create(&full, &source(), "New", &[ProfileSection::Dock], "n").error);
}

#[test]
fn find_prefers_id_then_matches_name_ignoring_case() {
    let list = one("Work", "id-1");

    assert_eq!("id-1", ProfileLibrary::find(&list, Some("id-1")).unwrap().id);
    assert_eq!("id-1", ProfileLibrary::find(&list, Some("  WORK ")).unwrap().id);
    assert!(ProfileLibrary::find(&list, Some("Play")).is_none());
    assert!(ProfileLibrary::find(&list, None).is_none());
    assert!(ProfileLibrary::find(&list, Some(" ")).is_none());
}

#[test]
fn apply_layers_only_the_profile_sections_over_current_settings() {
    let list = one("Work", "a");
    let current = AppSettings {
        general: GeneralSettings { language: "en".to_string(), ..Default::default() },
        appearance: AppearanceSettings { theme: AppTheme::Dark, ..Default::default() },
        music: MusicSettings { show_waveform: true, ..Default::default() },
        notch: NotchSettings { enabled: false, ..Default::default() },
        ..Default::default()
    };

    let result = ProfileLibrary::apply(&list, &current, Some("Work"));

    assert!(result.is_success());
    let settings = result.settings.unwrap();
    assert_eq!(AppTheme::Light, settings.appearance.theme);
    assert!(!settings.music.show_waveform);
    assert_eq!("en", settings.general.language);
    assert!(!settings.notch.enabled);
    assert_eq!(vec![ProfileSection::Appearance, ProfileSection::Music], result.changed_sections);
}

#[test]
fn apply_reports_no_changed_sections_when_settings_already_match() {
    let result = ProfileLibrary::apply(&one("Work", "a"), &source(), Some("a"));

    assert!(result.is_success());
    assert!(result.changed_sections.is_empty());
}

#[test]
fn apply_unknown_profile_is_not_found() {
    let result = ProfileLibrary::apply(&one("Work", "a"), &source(), Some("nope"));

    assert_eq!(ProfileLibraryError::NotFound, result.error);
    assert!(result.settings.is_none());
}

#[test]
fn apply_corrupt_stored_text_is_rejected_without_settings() {
    let list = vec![SavedProfile::new("a", "Work", "not json")];

    let result = ProfileLibrary::apply(&list, &source(), Some("a"));

    assert_eq!(ProfileLibraryError::CorruptProfile, result.error);
    assert!(result.settings.is_none());
}

#[test]
fn apply_does_not_modify_the_input_settings() {
    let current = AppSettings::default();

    ProfileLibrary::apply(&one("Work", "a"), &current, Some("a"));

    assert_eq!(AppSettings::default(), current);
}

#[test]
fn rename_keeps_the_id_and_updates_the_name_inside_the_text() {
    let result = ProfileLibrary::rename(&one("Work", "id-1"), "id-1", "Office");

    assert!(result.is_success());
    let profile = result.profile.unwrap();
    assert_eq!("id-1", profile.id);
    assert_eq!("Office", profile.name);
    assert_eq!("Office", SettingsProfileCodec::read(&profile.json).profile.unwrap().name);
}

#[test]
fn rename_to_its_own_name_with_different_case_is_allowed() {
    assert!(ProfileLibrary::rename(&one("Work", "id-1"), "id-1", "WORK").is_success());
}

#[test]
fn rename_rejects_a_taken_name_an_invalid_name_and_an_unknown_id() {
    let two = ProfileLibrary::create(&one("Work", "a"), &source(), "Play", &[ProfileSection::Dock], "b").profiles;

    assert_eq!(ProfileLibraryError::DuplicateName, ProfileLibrary::rename(&two, "b", "work").error);
    assert_eq!(ProfileLibraryError::InvalidName, ProfileLibrary::rename(&two, "b", " ").error);
    assert_eq!(ProfileLibraryError::NotFound, ProfileLibrary::rename(&two, "zzz", "X").error);
}

#[test]
fn duplicate_adds_a_copy_with_a_unique_name_and_new_id() {
    let once = ProfileLibrary::duplicate(&one("Work", "a"), "a", "b");
    let twice = ProfileLibrary::duplicate(&once.profiles, "a", "c");

    assert_eq!("Work (2)", once.profile.unwrap().name);
    let copy = twice.profile.unwrap();
    assert_eq!("Work (3)", copy.name);
    assert_eq!(3, twice.profiles.len());
    assert_eq!(ProfileLibrary::sections_of(&twice.profiles[0]), ProfileLibrary::sections_of(&copy));
}

#[test]
fn unique_name_keeps_the_result_within_the_name_limit() {
    let long_name = "x".repeat(SettingsProfileCodec::MAX_NAME_LENGTH as usize);
    let list = ProfileLibrary::create(&[], &source(), &long_name, &[ProfileSection::Dock], "a").profiles;

    let unique = ProfileLibrary::unique_name(&list, &long_name);

    assert!(SettingsProfileCodec::is_valid_name(Some(&unique)));
    assert!(unique.ends_with(" (2)"));
}

#[test]
fn delete_removes_only_the_target() {
    let two = ProfileLibrary::create(&one("Work", "a"), &source(), "Play", &[ProfileSection::Dock], "b").profiles;

    let result = ProfileLibrary::delete(&two, "a");

    assert_eq!(1, result.profiles.len());
    assert_eq!("Play", result.profiles[0].name);
    assert_eq!(ProfileLibraryError::NotFound, ProfileLibrary::delete(&two, "zzz").error);
}

#[test]
fn add_imported_resolves_a_name_clash_instead_of_overwriting() {
    let file = SettingsProfileCodec::export(&source(), "Work", &[ProfileSection::Appearance]);
    let imported = SettingsProfileCodec::read(&file).profile.unwrap();

    let result = ProfileLibrary::add_imported(&one("Work", "a"), &imported, "b");

    assert!(result.is_success());
    assert_eq!(2, result.profiles.len());
    let profile = result.profile.unwrap();
    assert_eq!("Work (2)", profile.name);
    assert_eq!(vec![ProfileSection::Appearance], ProfileLibrary::sections_of(&profile));
}

#[test]
fn operations_do_not_mutate_the_input_list() {
    let list = one("Work", "a");

    ProfileLibrary::delete(&list, "a");
    ProfileLibrary::duplicate(&list, "a", "b");

    assert_eq!(1, list.len());
}

#[test]
fn store_round_trips_through_a_file_and_drops_broken_entries() {
    let dir = TempDir::unique("koy-profiles-");
    // crash.log'a yazmamak için hata geri çağrısı sessiz (C# testleri TestLogRedirect kullanır).
    let quiet: crate::settings::json_list_file::JsonErrorCallback = std::sync::Arc::new(|_: &str, _: &dyn std::error::Error| {});
    let store = ProfileStore::with_error_handler(dir.file("profiles.json"), Some(quiet));
    assert!(store.load().is_empty());

    let list = one("Work", "a");
    assert!(store.save(&list));
    assert_eq!(list, store.load());

    std::fs::write(
        store.path(),
        r#"[{"Id":"","Name":"x","Json":"x"},{"Id":"k","Name":"ok","Json":"x"},{"Id":"k","Name":"dup","Json":"x"}]"#,
    )
    .unwrap();
    let loaded = store.load();
    assert_eq!(1, loaded.len());
    assert_eq!("ok", loaded[0].name);

    std::fs::write(store.path(), "garbage").unwrap();
    assert!(store.load().is_empty());
}

#[test]
fn store_ignores_null_entries_in_the_file() {
    let dir = TempDir::new("koy-profiles-");
    let store = ProfileStore::new(dir.file("profiles.json"));
    std::fs::write(store.path(), r#"[null,{"Id":"k","Name":"ok","Json":"x"}]"#).unwrap();

    let loaded = store.load();

    assert_eq!(1, loaded.len());
    assert_eq!("ok", loaded[0].name);
}

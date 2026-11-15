//! `Settings/SettingsPageResetTests.cs`.

use crate::appearance::AnimationSpeed;
use crate::settings::*;

fn customized() -> AppSettings {
    AppSettings {
        general: GeneralSettings {
            language: "en".to_string(),
            first_run_completed: true,
            last_seen_version: "0.2.7".to_string(),
            auto_check_updates: false,
            ..Default::default()
        },
        appearance: AppearanceSettings { animation_speed: AnimationSpeed::Fast, ..Default::default() },
        notch: NotchSettings { show_volume_osd: false, ..Default::default() },
        music: MusicSettings { show_waveform: false, ..Default::default() },
        dock: DockSettings { enabled: false, show_tray: false, ..Default::default() },
        ..Default::default()
    }
}

#[test]
fn resets_only_the_given_sections() {
    let current = customized();

    let reset = SettingsPageReset::reset(&current, &["Notch", "Music"]);

    assert_eq!(NotchSettings::default(), reset.notch);
    assert_eq!(MusicSettings::default(), reset.music);
    assert_eq!(current.appearance, reset.appearance);
    assert_eq!(current.dock, reset.dock);
    assert_eq!(current.general, reset.general);
}

#[test]
fn general_reset_keeps_first_run_and_last_seen_version() {
    let reset = SettingsPageReset::reset(&customized(), &["General"]);

    assert!(reset.general.first_run_completed);
    assert_eq!("0.2.7", reset.general.last_seen_version);
    assert_eq!(GeneralSettings::default().language, reset.general.language);
    assert!(reset.general.auto_check_updates);
}

#[test]
fn reset_page_reports_no_changes_for_that_section() {
    let reset = SettingsPageReset::reset(&customized(), &["General", "Appearance"]);
    let changed = SettingsChanges::changed_keys(&reset, &AppSettings::default());

    assert!(!SettingsChanges::has_changes(&changed, "General"));
    assert!(!SettingsChanges::has_changes(&changed, "Appearance"));
    assert!(SettingsChanges::has_changes(&changed, "Dock"));
}

#[test]
fn dock_and_display_sections_cannot_be_reset() {
    for section in ["Dock", "Displays", "DockItems", "Nope"] {
        let result = std::panic::catch_unwind(|| SettingsPageReset::reset(&AppSettings::default(), &[section]));
        assert!(result.is_err(), "{section}");
    }
}

#[test]
fn every_resettable_section_is_a_real_settings_section() {
    let defaults = AppSettings::default();
    let names: Vec<&str> = defaults.section_views(&defaults).iter().map(|s| s.name).collect();
    for section in SettingsPageReset::RESETTABLE_SECTIONS {
        assert!(names.contains(section), "{section}");
        // Her biri gerçekten sıfırlanır (eşleşmede eksik dal kalmasın).
        let reset = SettingsPageReset::reset(&defaults, &[section]);
        assert_eq!(defaults, reset, "{section}");
    }
}

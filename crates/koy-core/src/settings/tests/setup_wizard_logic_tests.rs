//! `Settings/SetupWizardLogicTests.cs`.

use super::TempDir;
use crate::appearance::{AnimationSpeed, AppTheme};
use crate::settings::*;

#[test]
fn app_settings_default_first_run_completed_is_false() {
    let settings = AppSettings::default();

    assert!(!settings.general.first_run_completed);
    assert_eq!(AppTheme::Dark, settings.appearance.theme);
    assert_eq!(12.0, settings.appearance.corner_radius);
    assert_eq!(AnimationSpeed::Normal, settings.appearance.animation_speed);
    assert!(settings.appearance.respect_reduce_motion);
    assert!(settings.dock.hide_windows_taskbar);
    assert_eq!(crate::visibility::VisibilityMode::Fixed, settings.notch.visibility);
    assert!(settings.notch.hide_in_fullscreen);
    assert!(settings.general.auto_check_updates);
    assert_eq!(UpdateChannel::Stable, settings.general.update_channel);
}

#[test]
fn wizard_card_selection_configures_features_correctly() {
    // (kart, çentik açık mı, dock açık mı)
    let cases = [(0, true, true), (1, true, false), (2, false, true)]; // çentik ve dock / yalnız çentik / yalnız dock
    for (card_index, expected_notch, expected_dock) in cases {
        let dir = TempDir::unique("koy-wizard-tests-");
        let store = SettingsStore::new(dir.file("settings.json"));
        let initial = store.load();
        assert!(!initial.general.first_run_completed);

        let notch_enabled = matches!(card_index, 0 | 1);
        let dock_enabled = matches!(card_index, 0 | 2);
        let configured = AppSettings {
            general: GeneralSettings { first_run_completed: true, language: "tr".to_string(), ..initial.general.clone() },
            notch: NotchSettings { enabled: notch_enabled, ..initial.notch.clone() },
            dock: DockSettings { enabled: dock_enabled, hide_windows_taskbar: dock_enabled, ..initial.dock.clone() },
            ..initial.clone()
        };

        store.save(&configured).unwrap();
        let loaded = store.load();

        assert!(loaded.general.first_run_completed, "{card_index}");
        assert_eq!(expected_notch, loaded.notch.enabled, "{card_index}");
        assert_eq!(crate::visibility::VisibilityMode::Fixed, loaded.notch.visibility, "{card_index}");
        assert!(loaded.notch.hide_in_fullscreen, "{card_index}");
        assert_eq!(expected_dock, loaded.dock.enabled, "{card_index}");
        assert_eq!(dock_enabled, loaded.dock.hide_windows_taskbar, "{card_index}");
    }
}

#[test]
fn app_settings_equality_checks_all_new_properties() {
    let s1 = AppSettings::default();
    let s2 = AppSettings::default();
    assert_eq!(s1, s2);

    let modified_first_run = AppSettings { general: GeneralSettings { first_run_completed: true, ..s1.general.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_first_run);

    let modified_theme = AppSettings { appearance: AppearanceSettings { theme: AppTheme::Light, ..s1.appearance.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_theme);

    let modified_radius = AppSettings { appearance: AppearanceSettings { corner_radius: 6.0, ..s1.appearance.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_radius);

    let modified_anim =
        AppSettings { appearance: AppearanceSettings { animation_speed: AnimationSpeed::Fast, ..s1.appearance.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_anim);

    let modified_motion =
        AppSettings { appearance: AppearanceSettings { respect_reduce_motion: false, ..s1.appearance.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_motion);

    let modified_taskbar = AppSettings { dock: DockSettings { hide_windows_taskbar: false, ..s1.dock.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_taskbar);

    let modified_updates = AppSettings { general: GeneralSettings { auto_check_updates: false, ..s1.general.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_updates);

    let modified_channel =
        AppSettings { general: GeneralSettings { update_channel: UpdateChannel::Preview, ..s1.general.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_channel);

    let modified_saver_motion =
        AppSettings { appearance: AppearanceSettings { battery_saver_reduce_motion: false, ..s1.appearance.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_saver_motion);

    let modified_osd = AppSettings { notch: NotchSettings { show_volume_osd: false, ..s1.notch.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_osd);

    let modified_pomodoro = AppSettings { focus: FocusSettings { pomodoro_work_minutes: 50, ..s1.focus.clone() }, ..s1.clone() };
    assert_ne!(s1, modified_pomodoro);
}

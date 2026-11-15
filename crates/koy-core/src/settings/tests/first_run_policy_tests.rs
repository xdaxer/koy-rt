//! `Settings/FirstRunPolicyTests.cs`.

use crate::dock::DockProximityMode;
use crate::settings::*;

#[test]
fn sihirbaz_tamamlanmadiysa_dock_kapali_kalir() {
    let settings = AppSettings::default(); // FirstRunCompleted = false, DockEnabled varsayılan true

    let result = FirstRunPolicy::apply_safe_defaults_if_incomplete(&settings);

    assert!(!result.dock.enabled);
    assert!(!result.dock.hide_windows_taskbar);
    assert!(result.notch.enabled);
}

#[test]
fn sihirbaz_tamamlandiysa_ayarlar_degismez() {
    let settings = AppSettings {
        general: GeneralSettings { first_run_completed: true, ..Default::default() },
        dock: DockSettings { enabled: true, hide_windows_taskbar: true, ..Default::default() },
        ..Default::default()
    };

    let result = FirstRunPolicy::apply_safe_defaults_if_incomplete(&settings);

    // C# `Assert.Same`: aynı nesne; Rust'ta değer eşitliği.
    assert_eq!(settings, result);
}

#[test]
fn orijinal_ayar_nesnesini_degistirmez() {
    let settings = AppSettings::default();

    FirstRunPolicy::apply_safe_defaults_if_incomplete(&settings);

    assert!(settings.dock.enabled);
}

#[test]
fn create_reset_keeps_first_run_completed_and_restores_defaults() {
    let current = AppSettings {
        general: GeneralSettings { first_run_completed: true, language: "en".to_string(), ..Default::default() },
        notch: NotchSettings { enabled: false, ..Default::default() },
        dock: DockSettings { proximity: DockProximityMode::Dodge, ..Default::default() },
        ..Default::default()
    };

    let reset = FirstRunPolicy::create_reset(&current);

    assert!(reset.general.first_run_completed);
    assert_eq!(
        AppSettings { general: GeneralSettings { first_run_completed: true, ..Default::default() }, ..Default::default() },
        reset
    );
}

#[test]
fn create_reset_does_not_mark_unfinished_setup_as_completed() {
    let reset = FirstRunPolicy::create_reset(&AppSettings {
        general: GeneralSettings { first_run_completed: false, ..Default::default() },
        ..Default::default()
    });

    assert!(!reset.general.first_run_completed);
}

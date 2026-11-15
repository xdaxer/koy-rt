//! `Appearance/MotionTogglesTests.cs` içinde Settings'e bağlı olan testler (+ gerçek `MotionSettings` ile `MotionToggles`).

use super::{TempDir, load_json};
use crate::appearance::{AppTheme, MotionKind, MotionPolicy};
use crate::settings::*;

/// Her tür için ilgili anahtarı kapatır: anahtar adı türle birebir aynı, değişen yer tek yer.
fn with_only(disabled: MotionKind) -> MotionSettings {
    let mut m = MotionSettings::default();
    match disabled {
        MotionKind::NotchOpenClose => m.notch_open_close = false,
        MotionKind::DockMagnify => m.dock_magnify = false,
        MotionKind::FolderPanel => m.folder_panel = false,
        MotionKind::Osd => m.osd = false,
        MotionKind::BadgeBounce => m.badge_bounce = false,
        MotionKind::TabTransitions => m.tab_transitions = false,
    }
    m
}

#[test]
fn with_default_toggles_every_animation_gets_the_global_scale() {
    for kind in MotionKind::ALL {
        assert_eq!(1.0, MotionPolicy::scale_for(kind, &MotionSettings::default(), 1.0), "{kind:?}");
    }
}

#[test]
fn a_disabled_toggle_makes_only_its_own_animation_instant() {
    for kind in MotionKind::ALL {
        let toggles = with_only(kind);

        assert_eq!(0.0, MotionPolicy::scale_for(kind, &toggles, 1.0), "{kind:?}");
        for other in MotionKind::ALL.into_iter().filter(|k| *k != kind) {
            assert_eq!(1.0, MotionPolicy::scale_for(other, &toggles, 1.0), "{kind:?} / {other:?}");
        }
    }
}

#[test]
fn missing_motion_section_defaults_to_everything_on() {
    let loaded = load_json(r#"{ "Version": 4, "Appearance": { "Theme": "Light" } }"#);

    assert!(loaded.appearance.motion.osd);
    assert!(loaded.appearance.motion.notch_open_close);
}

#[test]
fn null_motion_section_is_replaced_with_defaults() {
    let loaded = load_json(r#"{ "Version": 4, "Appearance": { "Motion": null } }"#);

    assert!(loaded.appearance.motion.dock_magnify);
}

#[test]
fn a_saved_toggle_is_read_back_after_a_save_and_load_round_trip() {
    let dir = TempDir::unique("koy-motion-");
    let store = SettingsStore::new(dir.file("settings.json"));
    let original = AppSettings {
        appearance: AppearanceSettings { motion: MotionSettings { osd: false, ..Default::default() }, ..Default::default() },
        ..Default::default()
    };
    store.save(&original).unwrap();

    let reloaded = store.load();

    assert!(!reloaded.appearance.motion.osd);
    assert!(reloaded.appearance.motion.tab_transitions);
    assert_eq!(original, reloaded);
}

#[test]
fn an_invalid_motion_value_resets_only_that_flag() {
    let loaded = load_json(r#"{ "Version": 4, "Appearance": { "Theme": "Light", "Motion": { "Osd": "yanlis", "DockMagnify": false } } }"#);

    assert!(loaded.appearance.motion.osd);
    assert!(!loaded.appearance.motion.dock_magnify);
    assert_eq!(AppTheme::Light, loaded.appearance.theme);
}

#[test]
fn changing_a_motion_toggle_makes_settings_unequal() {
    let changed = AppSettings {
        appearance: AppearanceSettings { motion: MotionSettings { osd: false, ..Default::default() }, ..Default::default() },
        ..Default::default()
    };

    assert_ne!(AppSettings::default(), changed);
}

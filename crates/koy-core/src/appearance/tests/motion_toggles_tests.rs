//! Animasyon başına açma/kapama (issue #79): kapalı animasyon anında biter, genel kurallar her zaman üstündedir.
//!
//! Atlananlar (Settings modülü taşınınca eklenecek): `Missing_motion_section_defaults_to_everything_on`,
//! `Null_motion_section_is_replaced_with_defaults`, `A_saved_toggle_is_read_back_after_a_save_and_load_round_trip`,
//! `An_invalid_motion_value_resets_only_that_flag`, `Changing_a_motion_toggle_makes_settings_unequal`
//! (hepsi `SettingsFile`/`SettingsStore`/`AppSettings` ister).

use crate::appearance::{MotionKind, MotionPolicy, MotionToggles};

/// `Koy.Core.Settings.MotionSettings` yerine geçen test sahtesi: varsayılan hepsi açık.
#[derive(Clone, Copy)]
struct MotionSettings {
    notch_open_close: bool,
    dock_magnify: bool,
    folder_panel: bool,
    osd: bool,
    badge_bounce: bool,
    tab_transitions: bool,
}

impl Default for MotionSettings {
    fn default() -> Self {
        MotionSettings {
            notch_open_close: true,
            dock_magnify: true,
            folder_panel: true,
            osd: true,
            badge_bounce: true,
            tab_transitions: true,
        }
    }
}

impl MotionToggles for MotionSettings {
    fn notch_open_close(&self) -> bool {
        self.notch_open_close
    }
    fn dock_magnify(&self) -> bool {
        self.dock_magnify
    }
    fn folder_panel(&self) -> bool {
        self.folder_panel
    }
    fn osd(&self) -> bool {
        self.osd
    }
    fn badge_bounce(&self) -> bool {
        self.badge_bounce
    }
    fn tab_transitions(&self) -> bool {
        self.tab_transitions
    }
}

/// Her tür için ilgili anahtarı kapatır: anahtar adı türle birebir aynı, değişen yer tek yer.
fn with_only(disabled: MotionKind) -> MotionSettings {
    let d = MotionSettings::default();
    match disabled {
        MotionKind::NotchOpenClose => MotionSettings { notch_open_close: false, ..d },
        MotionKind::DockMagnify => MotionSettings { dock_magnify: false, ..d },
        MotionKind::FolderPanel => MotionSettings { folder_panel: false, ..d },
        MotionKind::Osd => MotionSettings { osd: false, ..d },
        MotionKind::BadgeBounce => MotionSettings { badge_bounce: false, ..d },
        MotionKind::TabTransitions => MotionSettings { tab_transitions: false, ..d },
    }
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
fn a_zero_global_scale_wins_over_enabled_toggles() {
    assert_eq!(0.0, MotionPolicy::scale_for(MotionKind::Osd, &MotionSettings::default(), 0.0));
}

#[test]
fn a_nan_global_scale_means_no_animation() {
    assert_eq!(0.0, MotionPolicy::scale_for(MotionKind::Osd, &MotionSettings::default(), f64::NAN));
}

#[test]
fn scaled_speed_is_kept_when_the_toggle_is_on() {
    assert_eq!(1.5, MotionPolicy::scale_for(MotionKind::NotchOpenClose, &MotionSettings::default(), 1.5));
}

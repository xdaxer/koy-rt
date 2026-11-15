//! Karşılığı: `HotCornerActionsTests.cs` (HotCornerActionsTests ve HotCornerTrackerTests).

use std::collections::HashSet;

use crate::input::*;
use crate::settings::{AppSettings, HotCornersSettings, SettingsStore};
use crate::shortcuts::{ShortcutAction, ShortcutBindings};

#[test]
fn defaults_bind_nothing_and_keep_tracking_off() {
    let settings = AppSettings::default().hot_corners;

    assert!(!CornerActions::is_active(&settings));
    assert!(CornerActions::to_bindings(&settings).is_empty());
    assert!(settings.disable_in_fullscreen);
    assert_eq!(CornerActions::DEFAULT_DWELL_MS, settings.dwell_ms);
}

#[test]
fn tracking_turns_on_with_one_bound_corner_and_off_again() {
    let on = CornerActions::with(&HotCornersSettings::default(), ScreenCorner::BottomRight, CornerAction::OpenSearch);
    assert!(CornerActions::is_active(&on));

    let off = CornerActions::with(&on, ScreenCorner::BottomRight, CornerAction::None);
    assert!(!CornerActions::is_active(&off));
}

#[test]
fn corner_actions_map_to_existing_input_actions() {
    let cases = [
        (CornerAction::OpenNotch, InputAction::OpenNotch),
        (CornerAction::OpenSearch, InputAction::OpenSearch),
        (CornerAction::OpenKoyWindow, InputAction::OpenKoyWindow),
        (CornerAction::ToggleDock, InputAction::ToggleDock),
    ];
    for (action, expected) in cases {
        assert_eq!(Some(expected), CornerActions::to_input_action(action), "{action:?}");
    }
}

#[test]
fn none_and_undefined_values_map_to_nothing() {
    assert_eq!(None, CornerActions::to_input_action(CornerAction::None));
    // `(CornerAction)999` Rust'ta temsil edilemez; bkz. tests/mod.rs.
}

#[test]
fn to_bindings_builds_one_hot_corner_binding_per_bound_corner_with_clamped_dwell() {
    let settings = HotCornersSettings {
        top_left: CornerAction::OpenNotch,
        bottom_left: CornerAction::ToggleDock,
        dwell_ms: 5,
        ..HotCornersSettings::default()
    };

    let bindings = CornerActions::to_bindings(&settings);

    assert_eq!(2, bindings.len());
    let InputTrigger::HotCorner(first) = bindings[0].trigger else {
        panic!("HotCornerTrigger bekleniyordu: {:?}", bindings[0].trigger);
    };
    assert_eq!(ScreenCorner::TopLeft, first.corner);
    assert_eq!(CornerActions::MIN_DWELL_MS, first.dwell_ms);
    assert_eq!(InputAction::ToggleDock, bindings[1].action);
}

#[test]
fn to_bindings_passes_the_input_binding_validator() {
    let settings = HotCornersSettings {
        top_left: CornerAction::OpenNotch,
        top_right: CornerAction::OpenSearch,
        bottom_left: CornerAction::OpenKoyWindow,
        bottom_right: CornerAction::ToggleDock,
        ..HotCornersSettings::default()
    };

    let report = InputBindingValidator::validate(&CornerActions::to_bindings(&settings), &HashSet::new(), None);

    assert!(report.issues.is_empty());
    assert_eq!(4, report.accepted.len());
}

#[test]
fn clamp_dwell_stays_in_range() {
    let cases = [(-5, CornerActions::MIN_DWELL_MS), (400, 400), (99999, CornerActions::MAX_DWELL_MS)];
    for (input, expected) in cases {
        assert_eq!(expected, CornerActions::clamp_dwell(input), "{input}");
    }
}

#[test]
fn settings_survive_a_save_and_load_round_trip() {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("koy-corners-{nanos:x}-{}", std::process::id()));
    let result = std::panic::catch_unwind(|| {
        let store = SettingsStore::new(dir.join("settings.json"));
        let settings = AppSettings {
            hot_corners: HotCornersSettings {
                top_right: CornerAction::OpenSearch,
                dwell_ms: 700,
                disable_in_fullscreen: false,
                ..HotCornersSettings::default()
            },
            ..AppSettings::default()
        };
        let _ = store.save(&settings);

        let loaded = store.load();

        assert_eq!(CornerAction::OpenSearch, loaded.hot_corners.top_right);
        assert_eq!(CornerAction::None, loaded.hot_corners.top_left);
        assert_eq!(700, loaded.hot_corners.dwell_ms);
        assert!(!loaded.hot_corners.disable_in_fullscreen);
    });
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
fn open_koy_window_shortcut_is_unassigned_by_default_and_counts_as_default() {
    let settings = AppSettings::default();

    assert_eq!("", ShortcutBindings::get_text(&settings, ShortcutAction::OpenKoyWindow));
    assert!(ShortcutBindings::is_default(ShortcutAction::OpenKoyWindow, ""));
    assert!(!ShortcutBindings::is_default(ShortcutAction::OpenKoyWindow, "Ctrl+Alt+W"));

    let changed = ShortcutBindings::with_text(&settings, ShortcutAction::OpenKoyWindow, "Ctrl+Alt+W");
    assert_eq!("Ctrl+Alt+W", changed.shortcuts.open_koy_window);
    assert_eq!(settings.shortcuts.toggle_notch, changed.shortcuts.toggle_notch);
}

// ---------------------------------------------------------------------------------------------------------------------
// HotCornerTrackerTests
// ---------------------------------------------------------------------------------------------------------------------

const DWELL: i32 = 400;

#[test]
fn fires_once_after_the_dwell_and_not_again_while_staying() {
    let mut tracker = HotCornerTracker::new();

    assert_eq!(None, tracker.update(Some(ScreenCorner::TopLeft), false, 1000, DWELL));
    assert_eq!(None, tracker.update(Some(ScreenCorner::TopLeft), false, 1300, DWELL));
    assert_eq!(Some(ScreenCorner::TopLeft), tracker.update(Some(ScreenCorner::TopLeft), false, 1400, DWELL));
    assert_eq!(None, tracker.update(Some(ScreenCorner::TopLeft), false, 5000, DWELL));
}

#[test]
fn leaving_and_reentering_rearms_and_restarts_the_dwell() {
    let mut tracker = HotCornerTracker::new();
    tracker.update(Some(ScreenCorner::TopLeft), false, 0, DWELL);
    assert!(tracker.update(Some(ScreenCorner::TopLeft), false, 500, DWELL).is_some());

    assert_eq!(None, tracker.update(None, false, 600, DWELL));
    assert_eq!(None, tracker.update(Some(ScreenCorner::TopLeft), false, 700, DWELL));
    assert_eq!(None, tracker.update(Some(ScreenCorner::TopLeft), false, 1000, DWELL));
    assert!(tracker.update(Some(ScreenCorner::TopLeft), false, 1100, DWELL).is_some());
}

#[test]
fn a_brief_pass_through_never_fires() {
    let mut tracker = HotCornerTracker::new();

    assert_eq!(None, tracker.update(Some(ScreenCorner::BottomRight), false, 0, DWELL));
    assert_eq!(None, tracker.update(Some(ScreenCorner::BottomRight), false, 120, DWELL));
    assert_eq!(None, tracker.update(None, false, 240, DWELL));
    assert_eq!(None, tracker.update(Some(ScreenCorner::BottomRight), false, 360, DWELL));
}

#[test]
fn switching_to_another_corner_restarts_the_dwell() {
    let mut tracker = HotCornerTracker::new();
    tracker.update(Some(ScreenCorner::TopLeft), false, 0, DWELL);

    assert_eq!(None, tracker.update(Some(ScreenCorner::TopRight), false, 450, DWELL));
    assert_eq!(None, tracker.update(Some(ScreenCorner::TopRight), false, 800, DWELL));
    assert_eq!(Some(ScreenCorner::TopRight), tracker.update(Some(ScreenCorner::TopRight), false, 850, DWELL));
}

#[test]
fn suppression_resets_the_dwell_so_it_never_fires_through_a_drag_or_fullscreen() {
    let mut tracker = HotCornerTracker::new();
    tracker.update(Some(ScreenCorner::TopLeft), false, 0, DWELL);

    assert_eq!(None, tracker.update(Some(ScreenCorner::TopLeft), true, 450, DWELL));
    assert_eq!(None, tracker.update(Some(ScreenCorner::TopLeft), false, 500, DWELL));
    assert_eq!(None, tracker.update(Some(ScreenCorner::TopLeft), false, 850, DWELL));
    assert!(tracker.update(Some(ScreenCorner::TopLeft), false, 900, DWELL).is_some());
}

#[test]
fn out_of_range_dwell_is_clamped() {
    let mut tracker = HotCornerTracker::new();
    tracker.update(Some(ScreenCorner::TopLeft), false, 0, -1);

    assert_eq!(None, tracker.update(Some(ScreenCorner::TopLeft), false, (CornerActions::MIN_DWELL_MS - 1) as i64, -1));
    assert!(tracker.update(Some(ScreenCorner::TopLeft), false, CornerActions::MIN_DWELL_MS as i64, -1).is_some());
}

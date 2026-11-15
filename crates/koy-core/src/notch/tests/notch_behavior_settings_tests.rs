use crate::notch::{IdleDisplay, IdleDisplayMode, NotchController, NotchOpenBehavior, NotchState};
use crate::settings::{AppSettings, MusicSettings, NotchSettings, SettingsStore};
use crate::visibility::{VisibilityController, VisibilityMode};

#[test]
fn idle_display_maps_mode_to_clock_and_battery() {
    // halka pil ve gün/saat
    let rows = [
        (IdleDisplayMode::ClockAndBattery, true, true),
        (IdleDisplayMode::ClockOnly, true, false),
        (IdleDisplayMode::Nothing, false, false),
        (IdleDisplayMode::RingClockDay, true, true),
    ];
    for (mode, clock, battery) in rows {
        assert_eq!(clock, IdleDisplay::show_clock(mode), "{mode:?}");
        assert_eq!(battery, IdleDisplay::show_battery(mode), "{mode:?}");
    }
}

#[test]
fn idle_display_shows_radio_icons_only_in_clock_and_battery_mode() {
    // halka modunda Peek'te
    let rows = [
        (IdleDisplayMode::ClockAndBattery, true),
        (IdleDisplayMode::ClockOnly, false),
        (IdleDisplayMode::Nothing, false),
        (IdleDisplayMode::RingClockDay, true),
    ];
    for (mode, radio) in rows {
        assert_eq!(radio, IdleDisplay::show_radio(mode), "{mode:?}");
    }
}

#[test]
fn hover_open_behavior_expands_immediately_on_mouse_enter() {
    let controller = NotchController::new(None);
    controller.set_open_on_hover(true);

    controller.on_mouse_enter();

    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn hover_open_behavior_collapses_after_mouse_leave_timeout() {
    let controller = NotchController::new(None);
    controller.set_open_on_hover(true);
    controller.on_mouse_enter();

    controller.on_mouse_leave();
    controller.on_mouse_leave_timeout();

    assert_eq!(NotchState::Collapsed, controller.state());
}

#[test]
fn click_open_behavior_only_peeks_on_hover() {
    let controller = NotchController::new(None);
    controller.set_open_on_hover(false);

    controller.on_mouse_enter();
    assert_eq!(NotchState::Peek, controller.state());

    controller.on_click();
    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn default_open_behavior_setting_is_hover() {
    assert_eq!(NotchOpenBehavior::Hover, AppSettings::default().notch.open_behavior);
}

#[test]
fn saved_click_choice_is_kept() {
    let path = std::env::temp_dir().join(format!("koy-open-{}.json", std::process::id()));
    let store = SettingsStore::new(path.clone());
    store.save(&AppSettings { notch: NotchSettings { open_behavior: NotchOpenBehavior::Click, ..NotchSettings::default() }, ..AppSettings::default() }).unwrap();
    assert_eq!(NotchOpenBehavior::Click, store.load().notch.open_behavior);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn fullscreen_hides_by_default() {
    let visibility = VisibilityController::new(VisibilityMode::Fixed, None);

    visibility.set_fullscreen(true);

    assert!(!visibility.is_visible());
}

#[test]
fn fullscreen_keeps_notch_visible_when_hiding_is_turned_off() {
    let visibility = VisibilityController::new(VisibilityMode::Fixed, None);
    visibility.set_hide_in_fullscreen(false);

    visibility.set_fullscreen(true);

    assert!(visibility.is_visible());
}

#[test]
fn turning_hiding_off_while_fullscreen_shows_the_notch_again() {
    let visibility = VisibilityController::new(VisibilityMode::Fixed, None);
    visibility.set_fullscreen(true);
    assert!(!visibility.is_visible());

    visibility.set_hide_in_fullscreen(false);

    assert!(visibility.is_visible());
}

#[test]
fn behavior_settings_round_trip_through_the_store() {
    let dir = std::env::temp_dir().join(format!("koy-notch-{}", std::process::id()));
    let store = SettingsStore::new(dir.join("settings.json"));
    let settings = AppSettings {
        notch: NotchSettings { hide_in_fullscreen: false, idle_display: IdleDisplayMode::Nothing, ..NotchSettings::default() },
        music: MusicSettings { show_waveform: false, ..MusicSettings::default() },
        ..AppSettings::default()
    };

    store.save(&settings).unwrap();
    let loaded = store.load();

    assert!(!loaded.notch.hide_in_fullscreen);
    assert!(!loaded.music.show_waveform);
    assert_eq!(IdleDisplayMode::Nothing, loaded.notch.idle_display);
    assert_eq!(settings, loaded);
    let _ = std::fs::remove_dir_all(&dir);
}

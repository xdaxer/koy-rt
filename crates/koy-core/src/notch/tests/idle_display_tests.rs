use crate::notch::{IdleDisplay, IdleDisplayMode};

use IdleDisplayMode::{ClockAndBattery, ClockOnly, Nothing, RingClockDay};

fn check(f: impl Fn(IdleDisplayMode) -> bool, rows: [(IdleDisplayMode, bool); 4]) {
    for (mode, expected) in rows {
        assert_eq!(expected, f(mode), "{mode:?}");
    }
}

#[test]
fn show_radio_returns_expected_for_each_mode() {
    check(IdleDisplay::show_radio, [(ClockAndBattery, true), (ClockOnly, false), (Nothing, false), (RingClockDay, true)]);
}

#[test]
fn show_clock_returns_expected_for_each_mode() {
    check(IdleDisplay::show_clock, [(ClockAndBattery, true), (ClockOnly, true), (Nothing, false), (RingClockDay, true)]);
}

#[test]
fn show_battery_returns_expected_for_each_mode() {
    check(IdleDisplay::show_battery, [(ClockAndBattery, true), (ClockOnly, false), (Nothing, false), (RingClockDay, true)]);
}

#[test]
fn show_peek_details_returns_expected_for_each_mode() {
    check(IdleDisplay::show_peek_details, [(ClockAndBattery, true), (ClockOnly, false), (Nothing, false), (RingClockDay, true)]);
}

#[test]
fn show_radio_when_collapsed_hides_icons_in_ring_mode() {
    // halka modunda simgeler yalnız Peek'te
    check(IdleDisplay::show_radio_when_collapsed, [(ClockAndBattery, true), (ClockOnly, false), (Nothing, false), (RingClockDay, false)]);
}

#[test]
fn show_collapsed_date_respects_mode_and_setting() {
    // halka modunda tarih yalnız Peek'te
    let rows = [
        (ClockAndBattery, false, false),
        (ClockAndBattery, true, true),
        (ClockOnly, true, false),
        (Nothing, true, false),
        (RingClockDay, true, false),
    ];
    for (mode, clock_show_date, expected) in rows {
        assert_eq!(expected, IdleDisplay::show_collapsed_date(mode, clock_show_date), "{mode:?} {clock_show_date}");
    }
}

#[test]
fn is_ring_only_for_ring_mode() {
    check(IdleDisplay::is_ring, [(ClockAndBattery, false), (ClockOnly, false), (Nothing, false), (RingClockDay, true)]);
}

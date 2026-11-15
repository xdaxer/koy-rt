//! Karşılığı: `Clock/SecondClockTests.cs`.

use chrono::{NaiveDate, TimeZone, Utc};

use crate::clock::*;
use crate::time::{DateTimeOffset, TimeSpan};

fn noon() -> DateTimeOffset {
    Utc.with_ymd_and_hms(2026, 11, 12, 12, 0, 0).unwrap().fixed_offset()
}

#[test]
fn missing_or_invalid_id_gives_no_second_clock_and_never_throws() {
    let invariant = Culture::invariant();
    for id in [None, Some(""), Some("   "), Some("Not/A_Real_Zone"), Some("../../etc/passwd"), Some("Bad\0Id")] {
        assert!(SecondClock::resolve(id).is_none(), "{id:?}");
        assert!(SecondClock::read_by_id(id, noon(), "HH:mm", &invariant, None).is_none(), "{id:?}");
    }
}

#[test]
fn utc_id_resolves_on_every_os() {
    let reading = SecondClock::read_by_id(Some("UTC"), noon(), "HH:mm", &Culture::invariant(), None).unwrap();

    assert_eq!("12:00", reading.time);
    assert_eq!("UTC", reading.label);
}

#[test]
fn custom_zone_shifts_the_time_and_labels_the_offset() {
    let zone = TimeZoneInfo::create_custom_time_zone("test-plus-9", TimeSpan::from_hours(9), "Test", "Test");

    let reading = SecondClock::read(&zone, noon(), "HH:mm", &Culture::invariant(), None);

    assert_eq!("21:00", reading.time);
    assert_eq!("UTC+9", reading.label);
    assert_eq!("UTC+9 21:00", reading.text());
}

#[test]
fn custom_label_replaces_the_offset_label() {
    let zone = TimeZoneInfo::create_custom_time_zone("test-minus-5", TimeSpan::from_hours(-5), "Test", "Test");

    let reading = SecondClock::read(&zone, noon(), "h:mm tt", &Culture::invariant(), Some("  NYC "));

    assert_eq!("NYC", reading.label);
    assert_eq!("7:00 AM", reading.time);
}

#[test]
fn time_crosses_midnight_correctly() {
    let zone = TimeZoneInfo::create_custom_time_zone("test-plus-14", TimeSpan::from_hours(14), "Test", "Test");
    let late = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(23, 30, 0).unwrap().and_utc().fixed_offset();

    assert_eq!("13:30", SecondClock::read(&zone, late, "HH:mm", &Culture::invariant(), None).time);
}

#[test]
fn offset_label_is_compact() {
    let rows = [
        (0.0, "UTC"),
        (3.0, "UTC+3"),
        (-5.0, "UTC-5"),
        (5.5, "UTC+5:30"),
        (-9.5, "UTC-9:30"),
        (12.75, "UTC+12:45"),
    ];
    for (hours, expected) in rows {
        assert_eq!(expected, SecondClock::offset_label(TimeSpan::from_hours_f64(hours)), "{hours}");
    }
}

#[test]
fn reading_without_label_has_only_the_time() {
    assert_eq!("09:00", SecondClockReading::new("", "09:00").text());
}

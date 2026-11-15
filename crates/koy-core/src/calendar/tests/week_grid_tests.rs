//! Karşılığı: `WeekGridTests.cs`.

use chrono::{Datelike, Duration, NaiveDate};

use crate::calendar::*;
use crate::clock::DayOfWeek;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

// 12 Kasım 2026 Perşembe.
fn thursday() -> NaiveDate {
    date(2026, 11, 12)
}

#[test]
fn week_starts_on_the_chosen_first_day() {
    let rows = [
        (DayOfWeek::Monday, 9),
        (DayOfWeek::Sunday, 8),
        (DayOfWeek::Saturday, 7),
        (DayOfWeek::Thursday, 12),
        (DayOfWeek::Friday, 6),
    ];
    for (first, expected_day) in rows {
        let start = WeekGrid::start(thursday(), first);

        assert_eq!(first, DayOfWeek::of(start));
        assert_eq!(expected_day, start.day());
    }
}

#[test]
fn days_are_seven_consecutive_and_contain_the_anchor() {
    for first in [DayOfWeek::Monday, DayOfWeek::Sunday, DayOfWeek::Saturday] {
        let days = WeekGrid::days(thursday(), first);

        assert_eq!(7, days.len());
        assert_eq!(first, DayOfWeek::of(days[0]));
        assert!(days.contains(&thursday()));
        for i in 1..7 {
            assert_eq!(days[i - 1] + Duration::days(1), days[i]);
        }
    }
}

#[test]
fn every_day_of_a_week_maps_to_the_same_start() {
    let start = WeekGrid::start(thursday(), DayOfWeek::Sunday);

    for i in 0..7 {
        assert_eq!(start, WeekGrid::start(start + Duration::days(i), DayOfWeek::Sunday));
    }
}

#[test]
fn week_can_cross_month_and_year_boundaries() {
    let days = WeekGrid::days(date(2026, 1, 1), DayOfWeek::Monday);

    assert_eq!(date(2025, 12, 29), days[0]);
    assert_eq!(date(2026, 1, 4), days[6]);
}

#[test]
fn agenda_week_defaults_to_monday_and_follows_the_first_day() {
    let today = thursday();
    let monday = EventAgenda::for_week(&[], thursday(), today);
    let saturday = EventAgenda::for_week_with_first_day(&[], thursday(), today, DayOfWeek::Saturday);

    assert_eq!(DayOfWeek::Monday, DayOfWeek::of(monday[0].date));
    assert_eq!(DayOfWeek::Saturday, DayOfWeek::of(saturday[0].date));
    assert_eq!(1, saturday.iter().filter(|d| d.is_today).count());
    assert_eq!(date(2026, 11, 7), EventAgenda::week_start_with_first_day(thursday(), DayOfWeek::Saturday));
}

#[test]
fn agenda_week_keeps_events_on_their_days_for_any_first_day() {
    let start = date(2026, 11, 8).and_hms_opt(10, 0, 0).unwrap();
    let ev = CalendarEvent::new("1", "Toplantı", start, 30);

    let week = EventAgenda::for_week_with_first_day(&[ev], thursday(), thursday(), DayOfWeek::Sunday);

    assert_eq!(date(2026, 11, 8), week[0].date);
    assert_eq!(1, week[0].events.len());
    assert!(week.iter().skip(1).all(|d| d.events.is_empty()));
}

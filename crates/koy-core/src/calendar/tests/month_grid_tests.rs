//! Karşılığı: `MonthGridTests.cs`.

use chrono::NaiveDate;

use crate::calendar::*;
use crate::clock::DayOfWeek;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn build_always_returns_42_days() {
    let grid = MonthGrid::build(2026, 11, Some(date(2026, 11, 4)));
    assert_eq!(42, grid.len());
}

#[test]
fn build_first_day_is_monday() {
    let grid = MonthGrid::build(2026, 11, Some(date(2026, 11, 4)));
    assert_eq!(DayOfWeek::Monday, DayOfWeek::of(grid[0].date));
}

#[test]
fn build_correctly_flags_current_month_and_today() {
    let today = date(2026, 11, 4);
    let grid = MonthGrid::build(2026, 11, Some(today));

    let today_item = grid.iter().find(|d| d.is_today).expect("bugün ızgarada olmalı");
    assert_eq!(today, today_item.date);
    assert!(today_item.is_current_month);

    // November 2026 starts on Sunday (1st Nov 2026 is Sunday)
    // So offset is 6 days (26 Oct 2026 is Monday)
    assert!(!grid[0].is_current_month);
    assert_eq!(date(2026, 10, 26), grid[0].date);
}

#[test]
fn build_starts_on_the_chosen_first_day() {
    let rows = [
        (DayOfWeek::Sunday, 2026, 11, 1), // 1 Kasım 2026 Pazar: ızgara ayın ilk günüyle başlar
        (DayOfWeek::Saturday, 2026, 10, 31),
        (DayOfWeek::Monday, 2026, 10, 26),
    ];
    for (first_day, y, m, d) in rows {
        let grid = MonthGrid::build_with_first_day(2026, 11, Some(date(2026, 11, 4)), first_day);

        assert_eq!(first_day, DayOfWeek::of(grid[0].date));
        assert_eq!(date(y, m, d), grid[0].date);
        assert!(grid.iter().any(|c| c.date == date(2026, 11, 30)));
    }
}

#[test]
fn weekday_order_rotates_from_first_day() {
    assert_eq!(
        vec![
            DayOfWeek::Sunday,
            DayOfWeek::Monday,
            DayOfWeek::Tuesday,
            DayOfWeek::Wednesday,
            DayOfWeek::Thursday,
            DayOfWeek::Friday,
            DayOfWeek::Saturday
        ],
        MonthGrid::weekday_order(DayOfWeek::Sunday)
    );
    assert_eq!(DayOfWeek::Friday, MonthGrid::weekday_order(DayOfWeek::Saturday)[6]);
}

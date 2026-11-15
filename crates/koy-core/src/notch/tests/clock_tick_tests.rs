use chrono::{NaiveDate, NaiveDateTime, Timelike};

use crate::notch::ClockTick;
use crate::time::TimeSpan;

fn dt(h: u32, m: u32, s: u32, ms: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 11).unwrap().and_hms_milli_opt(h, m, s, ms).unwrap()
}

#[test]
fn waits_until_just_after_the_next_minute() {
    let now = dt(19, 21, 45, 250);

    assert_eq!(TimeSpan::from_millis(14_750) + ClockTick::MARGIN, ClockTick::delay_to_next_minute(now));
}

#[test]
fn wakes_into_the_next_minute() {
    let now = dt(19, 21, 7, 900);

    assert_eq!(22, (now + ClockTick::delay_to_next_minute(now).to_chrono()).minute());
}

#[test]
fn full_minute_wait_at_the_start_of_a_minute() {
    let now = dt(19, 21, 0, 0);

    assert_eq!(TimeSpan::from_mins(1) + ClockTick::MARGIN, ClockTick::delay_to_next_minute(now));
}

#[test]
fn never_shorter_than_the_minimum_delay() {
    let now = dt(19, 21, 59, 990);

    assert_eq!(ClockTick::MINIMUM_DELAY, ClockTick::delay_to_next_minute(now));
}

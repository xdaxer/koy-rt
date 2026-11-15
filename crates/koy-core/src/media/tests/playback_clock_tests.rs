use chrono::{FixedOffset, TimeZone};

use crate::media::date_time_offset_min_value;
use crate::media::{PlaybackClock, PlaybackTimeline};
use crate::time::{add_span, DateTimeOffset, TimeSpan};

fn now() -> DateTimeOffset {
    FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 9, 12, 0, 0).unwrap()
}

fn s(seconds: f64) -> TimeSpan {
    TimeSpan::from_seconds_f64(seconds)
}

fn timeline(position: f64, updated: DateTimeOffset, start: f64, end: f64) -> PlaybackTimeline {
    PlaybackTimeline { position: s(position), start_time: s(start), end_time: s(end), last_updated: updated }
}

#[test]
fn adds_elapsed_time_while_playing() {
    let t = timeline(10.0, add_span(now(), TimeSpan::from_secs(-5)), 0.0, 200.0);
    assert_eq!(s(15.0), PlaybackClock::position_at(&t, true, now()));
}

#[test]
fn paused_position_does_not_advance() {
    let t = timeline(10.0, add_span(now(), TimeSpan::from_secs(-5)), 0.0, 200.0);
    assert_eq!(s(10.0), PlaybackClock::position_at(&t, false, now()));
}

#[test]
fn is_relative_to_start_time() {
    let t = timeline(15.0, now(), 10.0, 200.0);
    assert_eq!(s(5.0), PlaybackClock::position_at(&t, false, now()));
}

#[test]
fn clamps_to_duration() {
    let t = timeline(199.0, add_span(now(), TimeSpan::from_mins(-30)), 0.0, 200.0);
    assert_eq!(s(200.0), PlaybackClock::position_at(&t, true, now()));
}

#[test]
fn ignores_very_old_timestamps() {
    let t = timeline(10.0, add_span(now(), TimeSpan::from_hours(-2)), 0.0, 200.0);
    assert_eq!(s(10.0), PlaybackClock::position_at(&t, true, now()));
}

#[test]
fn ignores_unset_timestamp() {
    let t = timeline(10.0, date_time_offset_min_value(), 0.0, 200.0);
    assert_eq!(s(10.0), PlaybackClock::position_at(&t, true, now()));
}

#[test]
fn negative_duration_is_zero() {
    let t = timeline(0.0, now(), 50.0, 10.0);
    assert_eq!(TimeSpan::ZERO, PlaybackClock::duration(&t));
}

use chrono::{NaiveDate, NaiveDateTime};

use crate::time::TimeSpan;
use crate::tray::{TaskbarCreatedPolicy, date_time_min_value};

const TRAY: isize = 0x1234;

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 7).unwrap().and_hms_opt(17, 30, 0).unwrap()
}

fn ago(span: TimeSpan) -> NaiveDateTime {
    now() - span.to_chrono()
}

#[test]
fn broadcast_just_after_our_own_with_same_explorer_tray_is_an_echo() {
    assert!(TaskbarCreatedPolicy::is_own_broadcast(ago(TimeSpan::from_millis(50)), now(), TRAY, TRAY));
}

#[test]
fn broadcast_at_the_edge_of_the_window_is_still_an_echo() {
    assert!(TaskbarCreatedPolicy::is_own_broadcast(ago(TaskbarCreatedPolicy::OWN_BROADCAST_WINDOW), now(), TRAY, TRAY));
}

#[test]
fn broadcast_long_after_ours_is_a_real_explorer_restart() {
    assert!(!TaskbarCreatedPolicy::is_own_broadcast(ago(TimeSpan::from_secs(10)), now(), TRAY, TRAY));
}

#[test]
fn coincident_real_restart_is_not_swallowed_when_the_explorer_tray_changed() {
    assert!(!TaskbarCreatedPolicy::is_own_broadcast(ago(TimeSpan::from_millis(200)), now(), TRAY, 0x9999));
}

#[test]
fn missing_explorer_tray_right_now_counts_as_restart() {
    assert!(!TaskbarCreatedPolicy::is_own_broadcast(ago(TimeSpan::from_millis(200)), now(), TRAY, 0));
}

#[test]
fn unknown_tray_at_broadcast_time_is_never_an_echo() {
    assert!(!TaskbarCreatedPolicy::is_own_broadcast(ago(TimeSpan::from_millis(200)), now(), 0, 0));
}

#[test]
fn without_any_broadcast_of_ours_nothing_is_an_echo() {
    assert!(!TaskbarCreatedPolicy::is_own_broadcast(date_time_min_value(), now(), TRAY, TRAY));
}

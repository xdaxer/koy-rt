use crate::notifications::*;
use crate::time::{DateTimeOffset, TimeSpan, add_span};
use chrono::{TimeZone, Utc};

pub(super) fn t0() -> DateTimeOffset {
    Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap().fixed_offset()
}

/// C# `Item(id, app = "Mail", title = "Hi", body = "Body", secondsAfter = 0)`.
pub(super) fn item_full(id: u32, app: &str, title: &str, body: &str, seconds_after: i64) -> NotificationItem {
    NotificationItem::new(id, app, title, body, add_span(t0(), TimeSpan::from_secs(seconds_after)))
}

pub(super) fn item(id: u32) -> NotificationItem {
    item_full(id, "Mail", "Hi", "Body", 0)
}

fn item_titled(id: u32, title: &str) -> NotificationItem {
    item_full(id, "Mail", title, "Body", 0)
}

fn item_after(id: u32, seconds_after: i64) -> NotificationItem {
    item_full(id, "Mail", "Hi", "Body", seconds_after)
}

fn snap(items: Vec<NotificationItem>) -> NotificationSnapshot {
    let stamps = items.iter().map(|i| NotificationStamp::new(i.id, i.time)).collect();
    NotificationSnapshot::new(stamps, items)
}

#[test]
fn first_read_is_baseline_and_yields_nothing() {
    let mut diff = NotificationDiff::new();
    assert!(!diff.has_baseline());

    let arrived = diff.apply(&snap(vec![item(1), item(2)]));

    assert!(arrived.is_empty());
    assert!(diff.has_baseline());
    assert!(diff.known().contains_key(&1));
    assert!(diff.known().contains_key(&2));
}

#[test]
fn new_id_after_baseline_is_reported() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![item(1)]));

    let arrived = diff.apply(&snap(vec![item(1), item_titled(2, "New")]));

    assert_eq!(arrived.len(), 1);
    assert_eq!(arrived[0].id, 2);
    assert_eq!(arrived[0].title, "New");
}

#[test]
fn same_id_is_reported_only_once() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![]));
    assert_eq!(diff.apply(&snap(vec![item(5)])).len(), 1);
    assert!(diff.apply(&snap(vec![item(5)])).is_empty());
}

#[test]
fn arrivals_come_in_time_order() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![]));

    let arrived = diff.apply(&snap(vec![item_after(9, 5), item_after(7, 1), item_after(8, 3)]));

    assert_eq!(arrived.iter().map(|i| i.id).collect::<Vec<_>>(), vec![7, 8, 9]);
}

#[test]
fn dismissed_ids_are_forgotten_so_the_set_stays_bounded() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![item(1), item(2)]));
    diff.apply(&snap(vec![item(2)]));

    assert!(!diff.known().contains_key(&1));
    assert!(diff.known().contains_key(&2));
}

#[test]
fn item_missing_from_ids_is_ignored() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![]));

    // Ayrıntısı gelmiş ama artık listede olmayan (okuma sırasında kapanmış) bildirim sayılmaz.
    let arrived = diff.apply(&NotificationSnapshot::new(vec![], vec![item(3)]));

    assert!(arrived.is_empty());
}

#[test]
fn id_without_detail_is_known_but_not_reported() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![]));

    let arrived = diff.apply(&NotificationSnapshot::new(vec![NotificationStamp::new(4, t0())], vec![]));

    assert!(arrived.is_empty());
    assert!(diff.known().contains_key(&4));
}

#[test]
fn same_id_with_advanced_creation_time_is_new() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![item_after(1, 0)]));

    // Lightshot gibi: aynı etiketle bildirimini değiştirir, kimlik aynı kalır, zaman ilerler.
    let arrived = diff.apply(&snap(vec![item_full(1, "Mail", "Second", "Body", 30)]));

    assert_eq!(arrived.len(), 1);
    assert_eq!(arrived[0].title, "Second");
}

#[test]
fn same_id_with_same_time_is_not_new() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![item_after(1, 5)]));

    assert!(diff.apply(&snap(vec![item_after(1, 5)])).is_empty());
}

#[test]
fn replaced_notification_is_reported_once_per_replacement() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![]));

    assert_eq!(diff.apply(&snap(vec![item_after(1, 1)])).len(), 1);
    assert!(diff.apply(&snap(vec![item_after(1, 1)])).is_empty());
    assert_eq!(diff.apply(&snap(vec![item_after(1, 9)])).len(), 1);
}

#[test]
fn older_time_under_the_same_id_is_not_new() {
    let mut diff = NotificationDiff::new();
    diff.apply(&snap(vec![item_after(1, 10)]));

    assert!(diff.apply(&snap(vec![item_after(1, 3)])).is_empty());
}

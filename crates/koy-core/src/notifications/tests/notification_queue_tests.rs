use std::sync::Arc;

use crate::notifications::*;
use crate::time::testing::FakeClock;
use crate::time::{DateTimeOffset, TimeProvider, TimeSpan, add_span};

fn clock_provider(clock: &Arc<FakeClock>) -> Option<Arc<dyn TimeProvider>> {
    Some(clock.clone())
}

fn queue() -> NotificationQueue {
    NotificationQueue::new(Some(Arc::new(FakeClock::new())))
}

/// C# `Item(id, app = "Mail", title = "Hi", body = "Body", time = new FakeClock().GetUtcNow())`.
fn item_full(id: u32, app: &str, title: &str, body: &str, time: Option<DateTimeOffset>) -> NotificationItem {
    NotificationItem::new(id, app, title, body, time.unwrap_or_else(|| FakeClock::new().utc_now()))
}

fn item(id: u32) -> NotificationItem {
    item_full(id, "Mail", "Hi", "Body", None)
}

fn item_app(id: u32, app: &str) -> NotificationItem {
    item_full(id, app, "Hi", "Body", None)
}

fn item_at(id: u32, time: DateTimeOffset) -> NotificationItem {
    item_full(id, "Mail", "Hi", "Body", Some(time))
}

#[test]
fn shows_in_arrival_order_one_at_a_time() {
    let mut queue = queue();
    queue.enqueue(item(1), false);
    queue.enqueue(item(2), false);

    assert_eq!(queue.try_take_next(false).unwrap().id, 1);
    assert!(queue.try_take_next(false).is_none());

    queue.complete();
    assert_eq!(queue.try_take_next(false).unwrap().id, 2);
}

#[test]
fn waits_while_the_notch_is_expanded() {
    let mut queue = queue();
    queue.enqueue(item(1), false);

    assert!(queue.try_take_next(true).is_none());
    assert_eq!(queue.pending_count(), 1);
    assert_eq!(queue.try_take_next(false).unwrap().id, 1);
}

#[test]
fn card_duration_is_six_seconds() {
    assert_eq!(TimeSpan::from_secs(6), NotificationQueue::CARD_DURATION);
}

#[test]
fn filters_own_notifications() {
    let mut queue = queue();
    assert!(!queue.enqueue(item_app(1, "Koy"), false));
    assert!(!queue.enqueue(item_app(2, " koy "), false));
    assert!(queue.enqueue(item_app(3, "Koyun Takvimi"), false));
}

#[test]
fn filters_while_do_not_disturb_is_on() {
    let mut queue = queue();
    assert!(!queue.enqueue(item(1), true));
    assert_eq!(queue.pending_count(), 0);
}

#[test]
fn filters_empty_notifications() {
    let mut queue = queue();
    assert!(!queue.enqueue(item_full(1, "Mail", " ", "", None), false));
    assert!(queue.enqueue(item_full(2, "Mail", "", "only body", None), false));
}

#[test]
fn ignores_duplicate_ids() {
    let mut queue = queue();
    assert!(queue.enqueue(item(1), false));
    assert!(!queue.enqueue(item(1), false));

    queue.try_take_next(false);
    assert!(!queue.enqueue(item(1), false));
}

#[test]
fn same_id_with_newer_time_is_queued_again() {
    let clock = Arc::new(FakeClock::new());
    let mut queue = NotificationQueue::new(clock_provider(&clock));
    let t = clock.utc_now();
    assert!(queue.enqueue(item_at(1, t), false));
    queue.try_take_next(false);
    queue.complete();

    let later = add_span(t, TimeSpan::from_secs(20));
    assert!(queue.enqueue(item_at(1, later), false));
    assert!(!queue.enqueue(item_at(1, later), false));
}

#[test]
fn keeps_at_most_five_pending_dropping_the_oldest() {
    let mut queue = queue();
    for id in 1..=7 {
        queue.enqueue(item(id), false);
    }

    assert_eq!(queue.pending_count(), NotificationQueue::MAX_PENDING);
    assert_eq!(queue.try_take_next(false).unwrap().id, 3);
}

#[test]
fn drops_notifications_that_grew_stale_while_waiting() {
    let clock = Arc::new(FakeClock::new());
    let mut queue = NotificationQueue::new(clock_provider(&clock));
    queue.enqueue(item_at(1, clock.utc_now()), false);

    clock.advance(NotificationQueue::MAX_AGE + TimeSpan::from_secs(1));
    queue.enqueue(item_at(2, clock.utc_now()), false);

    assert_eq!(queue.try_take_next(false).unwrap().id, 2);
}

#[test]
fn clear_forgets_everything() {
    let mut queue = queue();
    queue.enqueue(item(1), false);
    queue.enqueue(item(2), false);
    queue.try_take_next(false);

    queue.clear();

    assert!(queue.current().is_none());
    assert_eq!(queue.pending_count(), 0);
    assert!(queue.try_take_next(false).is_none());
}

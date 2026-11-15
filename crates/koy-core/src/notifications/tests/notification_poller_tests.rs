use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::notification_diff_tests::item;
use crate::cancel::CancellationToken;
use crate::notifications::*;
use crate::time::testing::FakeClock;
use crate::time::{DateTimeOffset, TimeProvider, TimeSpan};

fn snap(items: Vec<NotificationItem>, with_details: bool) -> NotificationSnapshot {
    let stamps = items.iter().map(|i| NotificationStamp::new(i.id, i.time)).collect();
    NotificationSnapshot::new(stamps, if with_details { items } else { vec![] })
}

fn empty_snapshot() -> NotificationSnapshot {
    NotificationSnapshot::new(vec![], vec![])
}

fn wait_until(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() <= deadline, "Koşul zamanında gerçekleşmedi.");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn clock() -> Arc<FakeClock> {
    Arc::new(FakeClock::new())
}

fn as_provider(clock: &Arc<FakeClock>) -> Option<Arc<dyn TimeProvider>> {
    Some(clock.clone())
}

#[test]
fn poll_once_first_read_asks_for_baseline_then_passes_known_ids() {
    // `seen[i]`: i. okumada verilen `known` (None ya da kimlik listesi).
    let seen: Arc<Mutex<Vec<Option<Vec<u32>>>>> = Arc::new(Mutex::new(Vec::new()));
    let seen_in_read = Arc::clone(&seen);
    let poller = NotificationPoller::new(
        Box::new(move |known: Option<&HashMap<u32, DateTimeOffset>>, _| {
            seen_in_read.lock().unwrap().push(known.map(|k| k.keys().copied().collect()));
            Ok(snap(vec![item(1)], true))
        }),
        None,
    );
    let mut diff = NotificationDiff::new();
    let arrived: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let arrived_in_handler = Arc::clone(&arrived);
    poller.arrived().subscribe(move |i| arrived_in_handler.lock().unwrap().push(i.id));

    poller.poll_once(&mut diff, &CancellationToken::none()).unwrap();
    poller.poll_once(&mut diff, &CancellationToken::none()).unwrap();

    let seen = seen.lock().unwrap();
    assert!(seen[0].is_none());
    assert!(seen[1].as_ref().unwrap().contains(&1));
    assert!(arrived.lock().unwrap().is_empty());
}

#[test]
fn poll_once_raises_arrived_for_new_items_only() {
    let round = Arc::new(AtomicUsize::new(0));
    let round_in_read = Arc::clone(&round);
    let poller = NotificationPoller::new(
        Box::new(move |_, _| {
            let r = round_in_read.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(if r == 1 { snap(vec![item(1)], false) } else { snap(vec![item(1), item(2)], true) })
        }),
        None,
    );
    let mut diff = NotificationDiff::new();
    let arrived: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let arrived_in_handler = Arc::clone(&arrived);
    poller.arrived().subscribe(move |i| arrived_in_handler.lock().unwrap().push(i.id));

    poller.poll_once(&mut diff, &CancellationToken::none()).unwrap();
    poller.poll_once(&mut diff, &CancellationToken::none()).unwrap();

    assert_eq!(*arrived.lock().unwrap(), vec![2]);
}

#[test]
fn loop_polls_at_the_fixed_interval_and_never_overlaps() {
    let clock = clock();
    let polls = Arc::new(AtomicUsize::new(0));
    let running = Arc::new(AtomicUsize::new(0));
    let overlapped = Arc::new(AtomicBool::new(false));
    let (p, r, o) = (Arc::clone(&polls), Arc::clone(&running), Arc::clone(&overlapped));
    let poller = NotificationPoller::new(
        Box::new(move |_, _| {
            if r.fetch_add(1, Ordering::SeqCst) + 1 > 1 {
                o.store(true, Ordering::SeqCst);
            }
            p.fetch_add(1, Ordering::SeqCst);
            std::thread::yield_now();
            r.fetch_sub(1, Ordering::SeqCst);
            Ok(empty_snapshot())
        }),
        as_provider(&clock),
    );

    poller.start();
    wait_until(|| polls.load(Ordering::SeqCst) == 1 && clock.active_timers() == 1);

    // Aralık dolmadan yeni yoklama yok.
    clock.advance(NotificationPoller::INTERVAL - TimeSpan::from_millis(1));
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(polls.load(Ordering::SeqCst), 1);

    clock.advance(TimeSpan::from_millis(1));
    wait_until(|| polls.load(Ordering::SeqCst) == 2);

    poller.stop();
    assert!(!overlapped.load(Ordering::SeqCst));
}

#[test]
fn stop_removes_the_timer_and_ends_polling() {
    let clock = clock();
    let polls = Arc::new(AtomicUsize::new(0));
    let p = Arc::clone(&polls);
    let poller = NotificationPoller::new(
        Box::new(move |_, _| {
            p.fetch_add(1, Ordering::SeqCst);
            Ok(empty_snapshot())
        }),
        as_provider(&clock),
    );

    poller.start();
    wait_until(|| polls.load(Ordering::SeqCst) == 1 && clock.active_timers() == 1);

    poller.stop();
    wait_until(|| clock.active_timers() == 0);

    clock.advance(TimeSpan::from_mins(1));
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(!poller.is_running());
}

#[test]
fn restart_takes_a_new_baseline() {
    let clock = clock();
    let round = Arc::new(AtomicUsize::new(0));
    let arrived: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let r = Arc::clone(&round);
    let poller = NotificationPoller::new(
        Box::new(move |_, _| {
            r.fetch_add(1, Ordering::SeqCst);
            Ok(snap(vec![item(7)], true))
        }),
        as_provider(&clock),
    );
    let arrived_in_handler = Arc::clone(&arrived);
    poller.arrived().subscribe(move |i| arrived_in_handler.lock().unwrap().push(i.id));

    poller.start();
    wait_until(|| round.load(Ordering::SeqCst) == 1 && clock.active_timers() == 1);
    poller.stop();
    wait_until(|| clock.active_timers() == 0);

    poller.start();
    wait_until(|| round.load(Ordering::SeqCst) == 2 && clock.active_timers() == 1);
    poller.stop();

    // Durdurulup yeniden başlayınca mevcut bildirimler yeni sayılmaz.
    assert!(arrived.lock().unwrap().is_empty());
}

#[test]
fn read_failure_backs_off_to_the_failure_interval() {
    let clock = clock();
    let polls = Arc::new(AtomicUsize::new(0));
    let p = Arc::clone(&polls);
    let poller = NotificationPoller::new(
        Box::new(move |_, _| {
            p.fetch_add(1, Ordering::SeqCst);
            Err(NotificationReadError::Failed("liste okunamadı".to_owned()))
        }),
        as_provider(&clock),
    );

    poller.start();
    wait_until(|| polls.load(Ordering::SeqCst) == 1 && clock.active_timers() == 1);

    clock.advance(NotificationPoller::INTERVAL);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(polls.load(Ordering::SeqCst), 1);

    clock.advance(NotificationPoller::FAILURE_INTERVAL - NotificationPoller::INTERVAL);
    wait_until(|| polls.load(Ordering::SeqCst) == 2);
    poller.stop();
}

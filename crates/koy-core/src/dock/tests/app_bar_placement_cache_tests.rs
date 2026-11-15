use crate::dock::{AppBarMath, AppBarPlacementCache};
use crate::shell::WindowRect;

const RECT: WindowRect = WindowRect::new(0, 1034, 1920, 1080);

#[test]
fn first_registration_always_sends_setpos_at_the_monitor_bottom() {
    let cache = AppBarPlacementCache::new();

    assert!(cache.should_send(1, RECT, true, false));

    // Gönderilecek alan alt kenara yaslı ve istenen yükseklikte (kayıt anında Gezgin itmesi olsa bile).
    let final_rect = AppBarMath::final_rect(WindowRect::new(0, 0, 1920, 1080), 46, WindowRect::new(0, 986, 1920, 1032), true);
    assert_eq!(RECT, final_rect);
}

#[test]
fn same_rect_is_skipped_only_while_we_own_the_edge() {
    let mut cache = AppBarPlacementCache::new();
    cache.record(1, RECT);

    assert!(!cache.should_send(1, RECT, true, false));
    assert!(cache.should_send(1, RECT, false, false));
    assert!(cache.should_send(1, RECT, true, true));
}

#[test]
fn changed_rect_or_cleared_cache_sends_again() {
    let mut cache = AppBarPlacementCache::new();
    cache.record(1, RECT);

    assert!(cache.should_send(1, WindowRect { top: 1032, ..RECT }, true, false));

    cache.clear(); // sahiplik değişti
    assert!(cache.should_send(1, RECT, true, false));
}

#[test]
fn other_windows_are_independent() {
    let mut cache = AppBarPlacementCache::new();
    cache.record(1, RECT);

    assert!(cache.should_send(2, RECT, true, false));
    cache.forget(1);
    assert!(cache.should_send(1, RECT, true, false));
}

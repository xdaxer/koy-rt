//! `VisibilityControllerTests.cs`. (`WindowRect_GeometryMethodsWorkCorrectly` Shell modülünün testlerinde.)

use std::sync::{Arc, Mutex};

use crate::shell::WindowRect;
use crate::visibility::*;

fn controller(mode: VisibilityMode) -> VisibilityController {
    VisibilityController::new(mode, None)
}

#[test]
fn fixed_mode_initially_visible_and_not_click_through() {
    let controller = controller(VisibilityMode::Fixed);
    assert!(controller.is_visible());
    assert!(!controller.is_click_through());
}

#[test]
fn fixed_mode_hides_when_fullscreen_and_restores_when_fullscreen_exits() {
    let controller = controller(VisibilityMode::Fixed);
    assert!(controller.is_visible());

    let events: Arc<Mutex<Vec<VisibilityResult>>> = Arc::default();
    let sink = Arc::clone(&events);
    controller.changed().subscribe(move |res| sink.lock().unwrap().push(*res));

    controller.set_fullscreen(true);
    assert!(!controller.is_visible());
    assert!(controller.is_click_through());
    assert_eq!(1, events.lock().unwrap().len());
    assert!(!events.lock().unwrap()[0].is_visible);
    assert!(events.lock().unwrap()[0].is_click_through);

    controller.set_fullscreen(false);
    assert!(controller.is_visible());
    assert!(!controller.is_click_through());
    assert_eq!(2, events.lock().unwrap().len());
    assert!(events.lock().unwrap()[1].is_visible);
}

#[test]
fn fixed_mode_does_not_hide_on_window_overlap() {
    let controller = controller(VisibilityMode::Fixed);
    controller.set_window_overlapping(true);
    assert!(controller.is_visible());
    assert!(!controller.is_click_through());
}

#[test]
fn smart_mode_initially_visible_hides_when_window_overlaps() {
    let controller = controller(VisibilityMode::Smart);
    assert!(controller.is_visible());

    controller.set_window_overlapping(true);
    assert!(!controller.is_visible());
    assert!(controller.is_click_through());

    controller.set_window_overlapping(false);
    assert!(controller.is_visible());
    assert!(!controller.is_click_through());
}

#[test]
fn smart_mode_when_overlapped_shows_on_mouse_at_edge_or_mouse_over() {
    let controller = controller(VisibilityMode::Smart);
    controller.set_window_overlapping(true);
    assert!(!controller.is_visible());

    // Edge hover triggers visibility
    controller.set_mouse_at_edge(true);
    assert!(controller.is_visible());

    // Mouse enters notch, leaves edge
    controller.set_mouse_over(true);
    controller.set_mouse_at_edge(false);
    assert!(controller.is_visible());

    // Mouse leaves notch
    controller.set_mouse_over(false);
    assert!(!controller.is_visible());
}

#[test]
fn smart_mode_when_overlapped_fullscreen_overrides_mouse() {
    let controller = controller(VisibilityMode::Smart);
    controller.set_window_overlapping(true);
    controller.set_mouse_at_edge(true);
    assert!(controller.is_visible());

    controller.set_fullscreen(true);
    assert!(!controller.is_visible());
    assert!(controller.is_click_through());

    // Even with mouse over, fullscreen remains hidden
    controller.set_mouse_over(true);
    assert!(!controller.is_visible());
}

#[test]
fn peek_mode_initially_hidden_shows_on_mouse_at_edge_or_over() {
    let controller = controller(VisibilityMode::Peek);
    assert!(!controller.is_visible());
    assert!(controller.is_click_through());

    controller.set_mouse_at_edge(true);
    assert!(controller.is_visible());

    controller.set_mouse_at_edge(false);
    assert!(!controller.is_visible());

    controller.set_mouse_over(true);
    assert!(controller.is_visible());

    controller.set_mouse_over(false);
    assert!(!controller.is_visible());
}

#[test]
fn hold_keeps_notch_visible_after_peek_timeout_until_released() {
    let controller = controller(VisibilityMode::Peek);
    controller.on_media_activity();
    controller.set_hold(true);
    controller.on_media_peek_timeout();
    assert!(controller.is_visible());

    controller.set_hold(false);
    assert!(!controller.is_visible());
}

#[test]
fn hold_does_not_override_fullscreen_hide() {
    let controller = controller(VisibilityMode::Smart);
    controller.set_hold(true);
    controller.set_fullscreen(true);
    assert!(!controller.is_visible());
}

#[test]
fn peek_mode_media_activity_peeks_and_times_out() {
    let controller = controller(VisibilityMode::Peek);
    assert!(!controller.is_visible());

    controller.on_media_activity();
    assert!(controller.is_visible());
    assert!(controller.is_media_peek_active());

    // Timeout expires without mouse interaction
    controller.on_media_peek_timeout();
    assert!(!controller.is_visible());
    assert!(!controller.is_media_peek_active());
}

#[test]
fn peek_mode_media_activity_stays_visible_if_mouse_entered() {
    let controller = controller(VisibilityMode::Peek);
    controller.on_media_activity();
    assert!(controller.is_visible());

    // User moves mouse over notch during peek
    controller.set_mouse_over(true);
    controller.on_media_peek_timeout();

    // Still visible because mouse is over!
    assert!(controller.is_visible());

    controller.set_mouse_over(false);
    assert!(!controller.is_visible());
}

#[test]
fn smart_mode_media_activity_peeks_even_when_overlapped() {
    let controller = controller(VisibilityMode::Smart);
    controller.set_window_overlapping(true);
    assert!(!controller.is_visible());

    // Media activity triggers temporary peek
    controller.on_media_activity();
    assert!(controller.is_visible());

    controller.on_media_peek_timeout();
    assert!(!controller.is_visible());
}

#[test]
fn mode_switching_dynamically_recalculates_visibility() {
    let controller = controller(VisibilityMode::Fixed);
    controller.set_window_overlapping(true);
    assert!(controller.is_visible());

    // Switching to Smart hides immediately because of overlap
    controller.set_mode(VisibilityMode::Smart);
    assert!(!controller.is_visible());

    // Switching to Peek stays hidden
    controller.set_mode(VisibilityMode::Peek);
    assert!(!controller.is_visible());

    // Switching back to Fixed shows immediately
    controller.set_mode(VisibilityMode::Fixed);
    assert!(controller.is_visible());
}

#[test]
fn update_foreground_window_calculates_overlap_with_notch_bounds() {
    let controller = controller(VisibilityMode::Smart);
    controller.set_notch_bounds(WindowRect::new(800, 0, 1120, 40));
    controller.set_notch_handle(1000);

    // Maximized window covering top of monitor -> overlaps
    controller.update_foreground_window(2000, WindowRect::new(0, 0, 1920, 1080));
    assert!(controller.is_window_overlapping());
    assert!(!controller.is_visible());

    // Window below notch (e.g. Top = 100) -> no overlap
    controller.update_foreground_window(2001, WindowRect::new(200, 100, 1200, 800));
    assert!(!controller.is_window_overlapping());
    assert!(controller.is_visible());

    // Own notch window handle -> ignored, no overlap
    controller.update_foreground_window(1000, WindowRect::new(800, 0, 1120, 40));
    assert!(!controller.is_window_overlapping());
    assert!(controller.is_visible());

    // Zero handle or empty bounds -> no overlap
    controller.update_foreground_window(0, WindowRect::EMPTY);
    assert!(!controller.is_window_overlapping());
    assert!(controller.is_visible());
}

#[test]
fn smart_mode_when_overlapped_expanded_keeps_visible_during_mouse_leave() {
    let controller = controller(VisibilityMode::Smart);
    controller.set_window_overlapping(true);
    assert!(!controller.is_visible());

    // Hover & expand
    controller.set_mouse_over(true);
    controller.set_expanded(true);
    assert!(controller.is_visible());

    // Mouse leaves temporarily within 400ms grace period (IsExpanded remains true)
    controller.set_mouse_over(false);
    assert!(controller.is_visible());

    // Grace period expires / collapsed
    controller.set_expanded(false);
    assert!(!controller.is_visible());
}

#[test]
fn peek_mode_expanded_keeps_visible_during_mouse_leave() {
    let controller = controller(VisibilityMode::Peek);
    assert!(!controller.is_visible());

    controller.set_mouse_over(true);
    controller.set_expanded(true);
    assert!(controller.is_visible());

    controller.set_mouse_over(false);
    assert!(controller.is_visible());

    controller.set_expanded(false);
    assert!(!controller.is_visible());
}

#[test]
fn fullscreen_overrides_expanded_state() {
    let controller = controller(VisibilityMode::Fixed);
    controller.set_expanded(true);
    assert!(controller.is_visible());

    controller.set_fullscreen(true);
    assert!(!controller.is_visible());
}

#[test]
fn update_foreground_window_desktop_empty_bounds_never_overlaps() {
    let controller = controller(VisibilityMode::Smart);
    controller.set_notch_bounds(WindowRect::new(750, 0, 1170, 140));

    // First an application window overlaps
    controller.update_foreground_window(1234, WindowRect::new(0, 0, 1920, 1080));
    assert!(controller.is_window_overlapping());
    assert!(!controller.is_visible());

    // User clicks desktop / taskbar (bounds reported as Empty)
    controller.update_foreground_window(5678, WindowRect::EMPTY);
    assert!(!controller.is_window_overlapping());
    assert!(controller.is_visible());
}

// Ek (C#'ta yok): sahte saatle 3 sn sonra göz atma süresi dolar ve Dispose zamanlayıcıyı durdurur.
#[test]
fn media_peek_timer_fires_through_time_provider_and_dispose_stops_it() {
    use crate::time::testing::FakeClock;
    use crate::time::TimeSpan;

    let clock = Arc::new(FakeClock::new());
    let controller = VisibilityController::new(VisibilityMode::Peek, Some(clock.clone()));
    controller.on_media_activity();
    assert!(controller.is_visible());
    assert_eq!(1, clock.active_timers());

    clock.advance(TimeSpan::from_secs(3));
    assert!(!controller.is_media_peek_active());
    assert!(!controller.is_visible());

    controller.on_media_activity();
    assert_eq!(1, clock.active_timers());
    controller.dispose();
    assert_eq!(0, clock.active_timers());
}

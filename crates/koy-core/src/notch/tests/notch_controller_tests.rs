use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicI32, Ordering};

use crate::notch::{NotchController, NotchPanel, NotchState};
use crate::time::testing::FakeClock;
use crate::time::{TimeProvider, TimeSpan};

fn controller() -> NotchController {
    NotchController::new(None)
}

#[test]
fn initial_state_is_collapsed_and_panel_is_home() {
    let controller = controller();
    assert_eq!(NotchState::Collapsed, controller.state());
    assert_eq!(NotchPanel::Home, controller.current_panel());
}

#[test]
fn hover_transitions_to_peek_and_leave_returns_to_collapsed() {
    let controller = controller();
    controller.on_mouse_enter();
    assert_eq!(NotchState::Peek, controller.state());
    assert!(controller.is_mouse_over());

    controller.on_mouse_leave();
    assert_eq!(NotchState::Collapsed, controller.state());
    assert!(!controller.is_mouse_over());
}

#[test]
fn click_from_collapsed_expands() {
    let controller = controller();
    controller.on_click();
    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn click_from_expanded_collapses() {
    let controller = controller();
    controller.on_click(); // to expanded
    assert_eq!(NotchState::Expanded, controller.state());

    controller.on_click(); // to collapsed
    assert_eq!(NotchState::Collapsed, controller.state());
}

#[test]
fn track_change_auto_peeks_and_times_out_when_mouse_outside() {
    let controller = controller();
    controller.on_track_changed();
    assert_eq!(NotchState::Peek, controller.state());

    // Auto peek timeout fires while mouse is outside
    controller.on_auto_peek_timeout();
    assert_eq!(NotchState::Collapsed, controller.state());
}

#[test]
fn track_change_auto_peeks_does_not_collapse_if_mouse_inside() {
    let controller = controller();
    controller.on_track_changed();
    controller.on_mouse_enter();

    controller.on_auto_peek_timeout();
    assert_eq!(NotchState::Peek, controller.state());
}

#[test]
fn expanded_mouse_leave_timeout_collapses_when_mouse_outside() {
    let controller = controller();
    controller.on_click(); // Expanded
    controller.on_mouse_leave();

    controller.on_mouse_leave_timeout();
    assert_eq!(NotchState::Collapsed, controller.state());
}

#[test]
fn expanded_mouse_leave_timeout_does_not_collapse_if_mouse_reentered() {
    let controller = controller();
    controller.on_click(); // Expanded
    controller.on_mouse_leave();
    controller.on_mouse_enter(); // Re-entered

    controller.on_mouse_leave_timeout();
    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn panel_cycling_works_across_enabled_panels() {
    let controller = controller();
    controller.set_enabled_panels(&[NotchPanel::Home, NotchPanel::ControlCenter, NotchPanel::Focus]);

    assert_eq!(NotchPanel::Home, controller.current_panel());

    controller.next_panel();
    assert_eq!(NotchPanel::ControlCenter, controller.current_panel());

    controller.next_panel();
    assert_eq!(NotchPanel::Focus, controller.current_panel());

    controller.next_panel();
    assert_eq!(NotchPanel::Home, controller.current_panel());

    controller.previous_panel();
    assert_eq!(NotchPanel::Focus, controller.current_panel());
}

#[test]
fn state_changed_fires_correctly() {
    let controller = controller();
    let observed: Arc<Mutex<Option<NotchState>>> = Arc::new(Mutex::new(None));
    let o = Arc::clone(&observed);
    controller.state_changed().subscribe(move |s| *o.lock().unwrap() = Some(*s));

    controller.on_click();
    assert_eq!(Some(NotchState::Expanded), *observed.lock().unwrap());
}

#[test]
fn set_enabled_panels_empty_defaults_to_home() {
    let controller = controller();
    controller.set_enabled_panels(&[]);
    assert_eq!(vec![NotchPanel::Home], controller.enabled_panels());
}

#[test]
fn set_enabled_panels_when_current_removed_switches_to_first() {
    let controller = controller();
    controller.set_enabled_panels(&[NotchPanel::Home, NotchPanel::Shelf]);
    controller.select_panel(NotchPanel::Shelf);
    assert_eq!(NotchPanel::Shelf, controller.current_panel());

    // Remove Shelf
    controller.set_enabled_panels(&[NotchPanel::ControlCenter, NotchPanel::Focus]);
    assert_eq!(NotchPanel::ControlCenter, controller.current_panel());
}

#[test]
fn next_previous_panel_with_single_panel_does_not_change() {
    let controller = controller();
    controller.set_enabled_panels(&[NotchPanel::Home]);
    controller.next_panel();
    assert_eq!(NotchPanel::Home, controller.current_panel());
    controller.previous_panel();
    assert_eq!(NotchPanel::Home, controller.current_panel());
}

#[test]
fn track_change_when_already_expanded_stays_expanded() {
    let controller = controller();
    controller.on_click(); // Expanded
    assert_eq!(NotchState::Expanded, controller.state());

    controller.on_track_changed();
    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn click_from_peek_expands() {
    let controller = controller();
    controller.on_mouse_enter();
    assert_eq!(NotchState::Peek, controller.state());

    controller.on_click();
    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn expand_directly_transitions_to_expanded() {
    let controller = controller();
    assert_eq!(NotchState::Collapsed, controller.state());

    controller.expand(None);
    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn expand_with_panel_selects_specified_panel() {
    let controller = controller();
    controller.set_enabled_panels(&[NotchPanel::Home, NotchPanel::Shelf]);

    controller.expand(Some(NotchPanel::Shelf));
    assert_eq!(NotchState::Expanded, controller.state());
    assert_eq!(NotchPanel::Shelf, controller.current_panel());
}

#[test]
fn expand_from_peek_transitions_to_expanded() {
    let controller = controller();
    controller.on_mouse_enter();
    assert_eq!(NotchState::Peek, controller.state());

    controller.expand(Some(NotchPanel::Shelf));
    assert_eq!(NotchState::Expanded, controller.state());
    assert_eq!(NotchPanel::Shelf, controller.current_panel());
}

#[test]
fn expand_when_already_expanded_stays_expanded_and_does_not_toggle() {
    let controller = controller();
    controller.expand(Some(NotchPanel::Home));
    assert_eq!(NotchState::Expanded, controller.state());

    // Subsequent drag or expand call must keep it Expanded (must NOT toggle like OnClick)
    controller.expand(Some(NotchPanel::Shelf));
    assert_eq!(NotchState::Expanded, controller.state());
    assert_eq!(NotchPanel::Shelf, controller.current_panel());
}

#[test]
fn expand_with_unenabled_panel_adds_to_enabled_and_selects() {
    let controller = controller();
    controller.set_enabled_panels(&[NotchPanel::Home]); // Shelf is not enabled

    controller.expand(Some(NotchPanel::Shelf));
    assert_eq!(NotchState::Expanded, controller.state());
    assert_eq!(NotchPanel::Shelf, controller.current_panel());
    assert!(controller.enabled_panels().contains(&NotchPanel::Shelf));
}

#[test]
fn dispose_multiple_calls_does_not_throw() {
    let controller = controller();
    controller.dispose();
    controller.dispose(); // second call should be completely safe
}

#[test]
fn go_to_panel_selects_panel_and_slides_in_panel_order() {
    let controller = controller();
    controller.set_enabled_panels(&[NotchPanel::Home, NotchPanel::ControlCenter, NotchPanel::Focus, NotchPanel::Shelf]);
    let directions: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::new()));
    let d = Arc::clone(&directions);
    controller.panel_transition_triggered().subscribe(move |v| d.lock().unwrap().push(*v));

    controller.go_to_panel(NotchPanel::Focus);
    assert_eq!(NotchPanel::Focus, controller.current_panel());

    controller.go_to_panel(NotchPanel::ControlCenter);
    assert_eq!(NotchPanel::ControlCenter, controller.current_panel());

    assert_eq!(vec![1, -1], *directions.lock().unwrap());
}

#[test]
fn go_to_panel_ignores_current_and_disabled_panels() {
    let controller = controller();
    controller.set_enabled_panels(&[NotchPanel::Home, NotchPanel::ControlCenter]);
    let transitions = Arc::new(AtomicI32::new(0));
    let t = Arc::clone(&transitions);
    controller.panel_transition_triggered().subscribe(move |_| {
        t.fetch_add(1, Ordering::SeqCst);
    });

    controller.go_to_panel(NotchPanel::Home);
    controller.go_to_panel(NotchPanel::Shelf);

    assert_eq!(NotchPanel::Home, controller.current_panel());
    assert_eq!(0, transitions.load(Ordering::SeqCst));
}

#[test]
fn set_enabled_panels_raises_enabled_panels_changed_with_new_order() {
    let controller = controller();
    let raised = Arc::new(AtomicI32::new(0));
    let r = Arc::clone(&raised);
    controller.enabled_panels_changed().subscribe(move |_| {
        r.fetch_add(1, Ordering::SeqCst);
    });

    controller.set_enabled_panels(&[NotchPanel::Focus, NotchPanel::Home]);

    assert_eq!(1, raised.load(Ordering::SeqCst));
    assert_eq!(vec![NotchPanel::Focus, NotchPanel::Home], controller.enabled_panels());
}

#[test]
fn expand_to_disabled_panel_adds_it_and_raises_enabled_panels_changed() {
    let controller = controller();
    controller.set_enabled_panels(&[NotchPanel::Home, NotchPanel::Focus]);
    let raised = Arc::new(AtomicI32::new(0));
    let r = Arc::clone(&raised);
    controller.enabled_panels_changed().subscribe(move |_| {
        r.fetch_add(1, Ordering::SeqCst);
    });

    controller.expand(Some(NotchPanel::Shelf));

    assert_eq!(1, raised.load(Ordering::SeqCst));
    assert_eq!(vec![NotchPanel::Home, NotchPanel::Focus, NotchPanel::Shelf], controller.enabled_panels());
    assert_eq!(NotchPanel::Shelf, controller.current_panel());
}

#[test]
fn hold_keeps_expanded_when_mouse_leaves_and_timeout_fires() {
    let controller = controller();
    controller.expand(None);
    controller.on_mouse_enter();
    controller.hold_open();

    controller.on_mouse_leave();
    controller.on_mouse_leave_timeout();

    assert_eq!(NotchState::Expanded, controller.state());
    assert!(controller.is_held());
    assert!(!controller.is_pinned());
}

#[test]
fn hold_ignores_click_so_picker_is_not_orphaned() {
    let controller = controller();
    controller.expand(None);
    controller.hold_open();

    controller.on_click();

    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn release_with_mouse_outside_closes_after_normal_delay() {
    let clock = Arc::new(FakeClock::new());
    let controller = NotchController::new(Some(Arc::clone(&clock) as Arc<dyn TimeProvider>));
    controller.expand(None);
    controller.on_mouse_enter();
    controller.hold_open();
    controller.on_mouse_leave();
    clock.advance(TimeSpan::from_secs(2));
    assert_eq!(NotchState::Expanded, controller.state());

    controller.release_hold();
    assert_eq!(NotchState::Expanded, controller.state());
    clock.advance(TimeSpan::from_millis(500));

    assert!(!controller.is_held());
    assert_eq!(NotchState::Collapsed, controller.state());
}

#[test]
fn release_with_mouse_inside_stays_open() {
    let clock = Arc::new(FakeClock::new());
    let controller = NotchController::new(Some(Arc::clone(&clock) as Arc<dyn TimeProvider>));
    controller.expand(None);
    controller.on_mouse_enter();
    controller.hold_open();
    controller.release_hold();
    clock.advance(TimeSpan::from_secs(2));

    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn hold_is_counted_nested_holds_keep_it_held() {
    let controller = controller();
    controller.expand(None);
    controller.hold_open();
    controller.hold_open();
    controller.release_hold();
    assert!(controller.is_held());

    controller.release_hold();
    controller.release_hold(); // fazladan bırakma sayacı eksiye düşürmez
    assert!(!controller.is_held());

    controller.on_click();
    assert_eq!(NotchState::Collapsed, controller.state());
}

#[test]
fn expand_to_enabled_panel_does_not_raise_enabled_panels_changed() {
    let controller = controller();
    controller.set_enabled_panels(&[NotchPanel::Home, NotchPanel::ControlCenter]);
    let raised = Arc::new(AtomicI32::new(0));
    let r = Arc::clone(&raised);
    controller.enabled_panels_changed().subscribe(move |_| {
        r.fetch_add(1, Ordering::SeqCst);
    });

    controller.expand(Some(NotchPanel::ControlCenter));

    assert_eq!(0, raised.load(Ordering::SeqCst));
    assert_eq!(NotchPanel::ControlCenter, controller.current_panel());
}

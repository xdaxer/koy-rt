//! Çentik düzeni seçenekleri (issue #78): gecikmeli açılma, kapanma gecikmesi, sekme sırası, sürükleyince açılma.

use std::sync::Arc;

use super::support::{s, ManualTimeProvider};
use crate::notch::{NotchController, NotchOpenBehavior, NotchOpenPolicy, NotchPanel, NotchPanels, NotchState};
use crate::time::TimeProvider;

fn time() -> Arc<ManualTimeProvider> {
    Arc::new(ManualTimeProvider::new())
}

fn controller_with(time: &Arc<ManualTimeProvider>) -> NotchController {
    NotchController::new(Some(Arc::clone(time) as Arc<dyn TimeProvider>))
}

#[test]
fn hover_open_with_delay_peeks_first_and_opens_when_the_timer_fires() {
    let time = time();
    let controller = controller_with(&time);
    controller.set_open_on_hover(true);
    controller.set_open_delay_ms(300);

    controller.on_mouse_enter();
    assert_eq!(NotchState::Peek, controller.state());

    time.fire_last();
    assert_eq!(NotchState::Expanded, controller.state());
}

#[test]
fn leaving_before_the_open_delay_cancels_opening() {
    let time = time();
    let controller = controller_with(&time);
    controller.set_open_on_hover(true);
    controller.set_open_delay_ms(300);

    controller.on_mouse_enter();
    controller.on_mouse_leave();
    controller.on_hover_open_timeout(); // geç kalan zamanlayıcı geri çağrısı da açmamalı

    assert_ne!(NotchState::Expanded, controller.state());
    // C#: `Assert.Contains(openTimer, time.Timers)` zamanlayıcı listede kalır; burada sayaçla denetlenir.
    assert_eq!(1, time.created());
}

#[test]
fn zero_open_delay_keeps_todays_immediate_open() {
    let time = time();
    let controller = controller_with(&time);
    controller.set_open_on_hover(true);

    controller.on_mouse_enter();

    assert_eq!(NotchState::Expanded, controller.state());
    assert_eq!(0, time.created());
}

#[test]
fn delays_are_clamped() {
    let rows = [(-5, 0), (250, 250), (99999, NotchOpenPolicy::MAX_DELAY_MS)];
    for (requested, expected) in rows {
        let controller = NotchController::new(None);
        controller.set_open_delay_ms(requested);
        controller.set_close_delay_ms(requested);
        assert_eq!(expected, controller.open_delay_ms(), "{requested}");
        assert_eq!(expected, controller.close_delay_ms(), "{requested}");
    }
}

#[test]
fn close_delay_default_is_todays_400_ms() {
    assert_eq!(NotchOpenPolicy::DEFAULT_CLOSE_DELAY_MS, NotchController::new(None).close_delay_ms());
}

#[test]
fn enabled_panels_follow_the_user_order() {
    let panels = NotchPanels::parse_enabled_with_order(
        Some(&s(&["home", "focus", "shelf", "system"])),
        true,
        Some(&s(&["system", "shelf"])),
    );

    assert_eq!(vec![NotchPanel::System, NotchPanel::Shelf, NotchPanel::Home, NotchPanel::Focus], panels);
}

#[test]
fn empty_order_keeps_the_default_order() {
    let keys = s(&["home", "focus", "calendar"]);
    assert_eq!(NotchPanels::parse_enabled(Some(&keys), true), NotchPanels::parse_enabled_with_order(Some(&keys), true, Some(&[])));
}

#[test]
fn move_shifts_one_step_and_stops_at_the_ends() {
    let up = NotchPanels::r#move(Some(&[]), NotchPanel::Focus, -1);
    assert_eq!("focus", up[0]);
    assert_eq!("home", up[1]);

    let top = NotchPanels::r#move(Some(&up), NotchPanel::Focus, -1);
    assert_eq!(up, top);

    let down = NotchPanels::r#move(Some(&[]), NotchPanel::ControlCenter, 1);
    assert_eq!("controlcenter", down.last().unwrap());
}

#[test]
fn temporary_panel_is_placed_by_the_user_order() {
    let controller = NotchController::new(None);
    controller.set_panel_order(Some(&s(&["shelf", "home", "focus"])));
    controller.set_enabled_panels(&[NotchPanel::Home, NotchPanel::Focus]);

    controller.expand(Some(NotchPanel::Shelf));

    assert_eq!(vec![NotchPanel::Shelf, NotchPanel::Home, NotchPanel::Focus], controller.enabled_panels());
}

#[test]
fn drag_opens_immediately_except_in_drag_over_mode_with_a_delay() {
    let rows = [
        (NotchOpenBehavior::Hover, 0.0, 500, true),
        (NotchOpenBehavior::Click, 0.0, 500, true),
        (NotchOpenBehavior::DragOver, 100.0, 500, false),
        (NotchOpenBehavior::DragOver, 500.0, 500, true),
        (NotchOpenBehavior::DragOver, 0.0, 0, true),
    ];
    for (behavior, elapsed, delay, expected) in rows {
        assert_eq!(expected, NotchOpenPolicy::drag_should_open(behavior, elapsed, delay), "{behavior:?} {elapsed} {delay}");
    }
}

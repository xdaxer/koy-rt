use crate::notch::{IdleDisplay, IdleDisplayMode, NotchController, NotchLayout, NotchState};

#[test]
fn layout_choice_depends_on_state_and_track() {
    // Parça yokken ipucu boş kalır: boşta yerleşimi açık kalır ki Wi-Fi / Bluetooth simgelerine tıklanabilsin.
    let rows = [
        (NotchState::Collapsed, false, true, false),
        (NotchState::Collapsed, true, true, false),
        (NotchState::Peek, false, true, false),
        (NotchState::Peek, true, false, true),
        (NotchState::Expanded, false, false, false),
        (NotchState::Expanded, true, false, false),
    ];
    for (state, has_track, collapsed, peek) in rows {
        assert_eq!(collapsed, NotchLayout::show_collapsed_layout(state, has_track), "{state:?} {has_track}");
        assert_eq!(peek, NotchLayout::show_peek_layout(state, has_track), "{state:?} {has_track}");
    }
}

#[test]
fn date_and_connections_show_only_when_hovering_without_a_track() {
    let rows = [
        (NotchState::Collapsed, false, false),
        (NotchState::Collapsed, true, false),
        (NotchState::Peek, false, true),
        (NotchState::Peek, true, false),
        (NotchState::Expanded, false, false),
    ];
    for (state, has_track, expected) in rows {
        assert_eq!(expected, NotchLayout::show_peek_details(state, has_track), "{state:?} {has_track}");
    }
}

#[test]
fn peek_details_follow_the_idle_display_setting() {
    let rows = [(IdleDisplayMode::ClockAndBattery, true), (IdleDisplayMode::ClockOnly, false), (IdleDisplayMode::Nothing, false)];
    for (mode, expected) in rows {
        assert_eq!(expected, IdleDisplay::show_peek_details(mode), "{mode:?}");
    }
}

#[test]
fn expanded_notch_fits_the_fixed_canvas() {
    // Kural #13: pencere sabit tuvalde durur; açık çentik tuvali aşamaz, titreme payı kalır.
    assert!((100.0..=NotchLayout::CANVAS_WIDTH).contains(&NotchLayout::EXPANDED_WIDTH));
    assert!((100.0..=NotchLayout::CANVAS_HEIGHT - 8.0).contains(&NotchLayout::EXPANDED_HEIGHT));
}

#[test]
fn hover_in_click_mode_without_track_keeps_idle_layout() {
    let controller = NotchController::new(None);
    controller.set_open_on_hover(false);

    controller.on_mouse_enter();

    assert_eq!(NotchState::Peek, controller.state());
    assert!(NotchLayout::show_collapsed_layout(controller.state(), false));
    assert!(!NotchLayout::show_peek_layout(controller.state(), false));
}

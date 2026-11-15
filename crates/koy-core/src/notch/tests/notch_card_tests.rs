use crate::notch::{NotchCard, NotchCardKind, NotchLayout};

#[test]
fn has_primary_action_only_when_label_given() {
    assert!(NotchCard::new(NotchCardKind::Update, "t", "s").with_primary_action_label("Update").has_primary_action());
    assert!(!NotchCard::new(NotchCardKind::Notification, "t", "s").has_primary_action());
}

#[test]
fn card_size_fits_inside_canvas_and_differs_from_hud() {
    assert!(NotchLayout::CARD_WIDTH < NotchLayout::CANVAS_WIDTH);
    assert!(NotchLayout::CARD_HEIGHT < NotchLayout::CANVAS_HEIGHT);
    assert!(NotchLayout::CARD_WIDTH > 260.0 && NotchLayout::CARD_HEIGHT > 40.0);
}

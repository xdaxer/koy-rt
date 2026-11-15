use crate::notch::{NotchStyle, NotchStyleMode, NotchSurface};

#[test]
fn pinned_keeps_ears_and_touches_the_screen_edge() {
    assert_eq!(12.0, NotchStyle::ear_radius(NotchStyleMode::Pinned));
    assert_eq!(0.0, NotchStyle::top_offset_dip(NotchStyleMode::Pinned));
}

#[test]
fn floating_has_no_ears_and_sits_nine_dip_below_the_top() {
    assert_eq!(0.0, NotchStyle::ear_radius(NotchStyleMode::Floating));
    assert_eq!(9.0, NotchStyle::top_offset_dip(NotchStyleMode::Floating));
}

#[test]
fn floating_radius_is_a_capsule_when_closed_and_fixed_when_open() {
    let rows = [
        (NotchSurface::Closed, 34.0, 17.0),
        (NotchSurface::Closed, 38.0, 19.0),
        (NotchSurface::Closed, 40.0, 20.0),
        (NotchSurface::Expanded, 352.0, 30.0),
        (NotchSurface::Card, 72.0, 26.0),
    ];
    for (surface, height, expected) in rows {
        assert_eq!(expected, NotchStyle::corner_radius(NotchStyleMode::Floating, surface, height, 99.0), "{surface:?} {height}");
    }
}

#[test]
fn pinned_radius_is_the_existing_value() {
    for surface in [NotchSurface::Closed, NotchSurface::Expanded, NotchSurface::Card] {
        assert_eq!(21.0, NotchStyle::corner_radius(NotchStyleMode::Pinned, surface, 50.0, 21.0), "{surface:?}");
    }
}

#[test]
fn edge_trigger_covers_the_floating_gap() {
    let rows = [
        (NotchStyleMode::Pinned, 1.0, 2),
        (NotchStyleMode::Floating, 1.0, 11),
        (NotchStyleMode::Floating, 1.5, 16),
        (NotchStyleMode::Floating, 2.0, 20),
    ];
    for (style, scale, expected_pixels) in rows {
        assert_eq!(expected_pixels, NotchStyle::edge_trigger_bottom_pixels(style, scale), "{style:?} {scale}");
    }
}

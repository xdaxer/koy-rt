use crate::dock::DockFitMath;

const SPACING: f64 = DockFitMath::SLOT_MARGIN_DIP;

// C# varsayılanları: iconSpacingDip = SlotMarginDip.
fn fit(preferred: f64, slots: i32, fixed_width: f64, available: f64) -> f64 {
    DockFitMath::fit_icon_size(preferred, slots, fixed_width, available, SPACING)
}

#[test]
fn fits_without_shrinking_when_there_is_room() {
    assert_eq!(28.0, fit(28.0, 10, 100.0, 1920.0));
}

#[test]
fn shrinks_to_the_largest_whole_icon_that_fits() {
    // 20 yuva, sabit 100, 1000 genişlik: yuva başına 45 -> simge 45 - 12 = 33 sığar; tercih 28 olduğundan 28.
    assert_eq!(28.0, fit(28.0, 20, 100.0, 1000.0));
    // 30 yuva: (1000-100)/30 = 30 -> 30 - 12 = 18 -> alt sınır 24'e çekilir.
    assert_eq!(24.0, fit(28.0, 30, 100.0, 1000.0));
    // 25 yuva: 36 - 12 = 24.
    assert_eq!(24.0, fit(28.0, 25, 100.0, 1000.0));
    // 24 yuva: 37.5 - 12 = 25.5 -> 25
    assert_eq!(25.0, fit(28.0, 24, 100.0, 1000.0));
}

#[test]
fn never_goes_below_the_minimum_even_when_nothing_fits() {
    assert_eq!(DockFitMath::MIN_ICON_SIZE_DIP, fit(32.0, 200, 100.0, 800.0));
}

#[test]
fn never_grows_beyond_the_preferred_size() {
    assert_eq!(32.0, fit(32.0, 2, 0.0, 5000.0));
}

#[test]
fn preferred_below_minimum_is_left_alone() {
    assert_eq!(20.0, fit(20.0, 100, 0.0, 500.0));
}

#[test]
fn degenerate_input_keeps_the_preferred_size() {
    // (slots, fixedWidth, available)
    let cases = [(0, 100.0, 1000.0), (5, 100.0, 0.0), (5, 100.0, f64::NAN), (5, 100.0, f64::INFINITY)];
    for (slots, fixed_width, available) in cases {
        assert_eq!(28.0, fit(28.0, slots, fixed_width, available), "({slots}, {fixed_width}, {available})");
    }
}

#[test]
fn result_actually_fits_when_above_minimum() {
    const SLOTS: i32 = 22;
    const FIXED_WIDTH: f64 = 120.0;
    const AVAILABLE: f64 = 1100.0;
    let icon = fit(32.0, SLOTS, FIXED_WIDTH, AVAILABLE);
    assert!(icon > DockFitMath::MIN_ICON_SIZE_DIP);
    assert!(FIXED_WIDTH + SLOTS as f64 * DockFitMath::slot_width(icon, SPACING) <= AVAILABLE);
}

#[test]
fn item_size_and_slot_width_follow_the_icon() {
    assert_eq!(38.0, DockFitMath::item_size_for(28.0));
    assert_eq!(40.0, DockFitMath::slot_width(28.0, SPACING));
}

#[test]
fn hover_peak_scale_never_overflows_the_bar() {
    // (bar, item, expected); C# varsayılanı maxScale = 1.30
    let cases = [
        (60.0, 38.0, 1.30),                // kapsül: tepe sınırı
        (48.0, 38.0, 1.2105263157894737),  // tam genişlik: (48-2)/38
        (48.0, 60.0, 1.0),                 // kutu çubuğa zaten sığmıyorsa büyütme yok
        (0.0, 38.0, 1.0),
    ];
    for (bar, item, expected) in cases {
        let actual = DockFitMath::hover_peak_scale(bar, item, 1.30);
        assert!((expected - actual).abs() < 1e-6, "({bar}, {item}): {actual}");
    }
}

#[test]
fn strip_max_width_never_negative() {
    assert_eq!(600.0, DockFitMath::strip_max_width(1000.0, 400.0));
    assert_eq!(0.0, DockFitMath::strip_max_width(100.0, 400.0));
}

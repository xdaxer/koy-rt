use crate::dock::DockPress;

fn approx(expected: f64, actual: f64) {
    assert!((expected - actual).abs() < 1e-6, "{expected} != {actual}");
}

#[test]
fn released_state_changes_nothing() {
    assert_eq!(1.0, DockPress::scale(0.0, true));
    assert_eq!(1.0, DockPress::opacity(0.0));
}

#[test]
fn fully_pressed_shrinks_and_darkens() {
    approx(DockPress::PRESSED_SCALE, DockPress::scale(1.0, true));
    approx(DockPress::PRESSED_OPACITY, DockPress::opacity(1.0));
    assert!((0.90..=0.94).contains(&DockPress::PRESSED_SCALE));
}

#[test]
fn reduced_motion_never_changes_the_scale_only_the_color() {
    assert_eq!(1.0, DockPress::scale(1.0, false));
    assert!(DockPress::opacity(1.0) < 1.0);
}

#[test]
fn spring_overshoot_springs_back_above_one_but_never_over_darkens() {
    // Bırakınca yay sıfırın altına sarkabilir: ölçek 1'in biraz üstüne çıkar (geri sıçrama), opaklık 1'de kalır.
    assert!(DockPress::scale(-0.1, true) > 1.0);
    assert_eq!(1.0, DockPress::opacity(-0.1));
    approx(DockPress::PRESSED_OPACITY, DockPress::opacity(1.3));
}

#[test]
fn press_scale_is_independent_of_hover_magnify() {
    // Görünen ölçek = hover x basma: ikisi çarpılır, biri öbürünün hedefini değiştirmez.
    let hover = 1.2;
    let combined = hover * DockPress::scale(1.0, true);
    approx(1.2 * DockPress::PRESSED_SCALE, combined);
    assert_eq!(1.2, hover);
}

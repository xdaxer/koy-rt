use crate::animation::Spring;

#[test]
fn settles_on_target() {
    let mut spring = Spring::new(0.0).with_target(100.0);

    let mut i = 0;
    while i < 120 && spring.step(1.0 / 60.0) {
        i += 1;
    }

    assert!(spring.is_at_rest());
    assert_eq!(100.0, spring.value());
}

#[test]
fn result_does_not_depend_on_frame_size() {
    let mut small = Spring::new(0.0).with_target(100.0);
    let mut large = Spring::new(0.0).with_target(100.0);

    for _ in 0..6 {
        small.step(1.0 / 60.0);
    }
    large.step(6.0 / 60.0);

    assert_eq!(crate::text::round_digits(small.value(), 6), crate::text::round_digits(large.value(), 6));
}

#[test]
fn step_returns_false_when_already_at_rest() {
    assert!(!Spring::new(42.0).step(1.0 / 60.0));
}

#[test]
fn snap_to_stops_motion() {
    let mut spring = Spring::new(0.0).with_target(100.0);
    spring.step(0.05);

    spring.snap_to(10.0);

    assert_eq!(10.0, spring.value());
    assert_eq!(0.0, spring.velocity());
    assert!(spring.is_at_rest());
}

#[test]
fn default_damping_overshoots_when_closing() {
    let mut spring = Spring::new(620.0).with_target(236.0);

    assert!(minimum_while_settling(&mut spring) < 235.0);
}

#[test]
fn critical_damping_closes_without_overshoot() {
    let mut spring = Spring::new(620.0).with_target(236.0);
    spring.set_damping(spring.critical_damping());

    assert!(minimum_while_settling(&mut spring) > 235.99);
    assert_eq!(236.0, spring.value());
}

#[test]
fn kick_moves_away_and_returns_to_target() {
    let mut spring = Spring::new(0.0).with_stiffness(260.0).with_damping(14.0);

    spring.kick(30.0);
    let mut peak = 0.0f64;
    let mut i = 0;
    while i < 240 && spring.step(1.0 / 60.0) {
        peak = peak.max(spring.value());
        i += 1;
    }

    assert!(peak >= 0.8 && peak <= 1.2, "{peak}");
    assert!(spring.is_at_rest());
    assert_eq!(0.0, spring.value());
}

fn minimum_while_settling(spring: &mut Spring) -> f64 {
    let mut min = spring.value();
    let mut i = 0;
    while i < 240 && spring.step(1.0 / 60.0) {
        min = min.min(spring.value());
        i += 1;
    }
    min
}

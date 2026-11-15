use crate::dock::SnapDebounce;
use crate::time::TimeSpan;

fn ms(ms: i64) -> TimeSpan {
    TimeSpan::from_millis(ms)
}

#[test]
fn first_state_is_adopted_immediately() {
    let mut d = SnapDebounce::new(ms(150));
    assert!(d.update(true, ms(0)));
    assert!(!d.is_pending());
}

#[test]
fn brief_flip_that_returns_is_ignored() {
    let mut d = SnapDebounce::new(ms(150));
    d.update(false, ms(0));

    assert!(!d.update(true, ms(10))); // Alt+Tab ara durağı
    assert!(d.is_pending());
    assert!(!d.update(false, ms(60))); // geri döndü
    assert!(!d.is_pending());
}

#[test]
fn sustained_change_is_applied_after_the_delay() {
    let mut d = SnapDebounce::new(ms(150));
    d.update(false, ms(0));

    assert!(!d.update(true, ms(10)));
    assert!(!d.update(true, ms(100)));
    assert!(d.update(true, ms(170)));
    assert!(!d.is_pending());
}

#[test]
fn remaining_counts_down_and_is_zero_without_pending() {
    let mut d = SnapDebounce::new(ms(150));
    d.update(false, ms(0));
    assert_eq!(TimeSpan::ZERO, d.remaining(ms(5)));

    d.update(true, ms(10));
    assert_eq!(ms(100), d.remaining(ms(60)));
    assert_eq!(TimeSpan::ZERO, d.remaining(ms(500)));
}

#[test]
fn reset_jumps_without_waiting() {
    let mut d = SnapDebounce::new(ms(150));
    d.update(false, ms(0));
    d.update(true, ms(10));

    d.reset(true);

    assert!(d.effective());
    assert!(!d.is_pending());
}

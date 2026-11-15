//! Karşılığı: `koy/tests/Core.Tests/UpdateCardPolicyTests.cs`.

use std::sync::Arc;

use crate::time::testing::FakeClock;
use crate::time::{TimeProvider, TimeSpan};
use crate::updates::UpdateCardPolicy;

fn policy(clock: &Arc<FakeClock>) -> UpdateCardPolicy {
    let provider: Arc<dyn TimeProvider> = clock.clone();
    UpdateCardPolicy::new(Some(provider))
}

#[test]
fn shows_first_time() {
    let policy = policy(&Arc::new(FakeClock::new()));
    assert!(policy.should_show(true, Some("0.3.0"), false));
}

#[test]
fn does_not_show_without_update() {
    let cases: [(bool, Option<&str>); 3] = [(false, Some("0.3.0")), (true, None), (true, Some(""))];
    for (has_update, version) in cases {
        let policy = policy(&Arc::new(FakeClock::new()));
        assert!(!policy.should_show(has_update, version, false), "{has_update} {version:?}");
    }
}

#[test]
fn does_not_show_again_for_same_version() {
    let mut policy = policy(&Arc::new(FakeClock::new()));
    policy.mark_shown("0.3.0");
    assert!(!policy.should_show(true, Some("0.3.0"), false));
}

#[test]
fn dismissed_version_stays_hidden() {
    let mut policy = policy(&Arc::new(FakeClock::new()));
    policy.mark_shown("0.3.0");
    policy.dismiss("0.3.0");
    assert!(!policy.should_show(true, Some("0.3.0"), false));
}

#[test]
fn different_version_shows_again() {
    let mut policy = policy(&Arc::new(FakeClock::new()));
    policy.mark_shown("0.3.0");
    policy.dismiss("0.3.0");
    assert!(policy.should_show(true, Some("0.3.1"), false));
}

#[test]
fn waits_while_notch_expanded_then_shows() {
    let policy = policy(&Arc::new(FakeClock::new()));
    assert!(!policy.should_show(true, Some("0.3.0"), true));
    assert!(policy.should_show(true, Some("0.3.0"), false));
}

#[test]
fn expires_after_fifteen_seconds() {
    let clock = Arc::new(FakeClock::new());
    let mut policy = policy(&clock);
    assert!(!policy.is_expired());

    policy.mark_shown("0.3.0");
    clock.advance(TimeSpan::from_secs(14));
    assert!(!policy.is_expired());

    clock.advance(TimeSpan::from_secs(1));
    assert!(policy.is_expired());
}

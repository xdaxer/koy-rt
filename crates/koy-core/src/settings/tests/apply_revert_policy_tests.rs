//! `Settings/ApplyRevertPolicyTests.cs`.

use chrono::{FixedOffset, TimeZone};

use crate::settings::*;
use crate::time::{DateTimeOffset, TimeSpan};

fn applied_at() -> DateTimeOffset {
    FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 11, 10, 0, 0).unwrap()
}

fn secs(s: f64) -> TimeSpan {
    TimeSpan::from_seconds_f64(s)
}

fn add(at: DateTimeOffset, span: TimeSpan) -> DateTimeOffset {
    crate::time::add_span(at, span)
}

const TIMEOUT: TimeSpan = ApplyRevertPolicy::DEFAULT_TIMEOUT;

#[test]
fn varsayilan_zaman_asimi_15_saniye() {
    assert_eq!(TimeSpan::from_secs(15), ApplyRevertPolicy::DEFAULT_TIMEOUT);
}

#[test]
fn zaman_asimi_dolmadan_yanit_yoksa_beklenir() {
    let decision = ApplyRevertPolicy::decide(applied_at(), add(applied_at(), secs(14.9)), ApplyRevertChoice::None, TIMEOUT);

    assert_eq!(ApplyRevertDecision::Wait, decision);
}

#[test]
fn zaman_asimi_dolunca_yanit_yoksa_onceki_deger_geri_alinir() {
    let at_timeout = ApplyRevertPolicy::decide(applied_at(), add(applied_at(), TIMEOUT), ApplyRevertChoice::None, TIMEOUT);
    let after_timeout = ApplyRevertPolicy::decide(applied_at(), add(applied_at(), TimeSpan::from_mins(1)), ApplyRevertChoice::None, TIMEOUT);

    assert_eq!(ApplyRevertDecision::Revert, at_timeout);
    assert_eq!(ApplyRevertDecision::Revert, after_timeout);
}

#[test]
fn onay_zaman_asiminda_bile_degeri_tutar() {
    let decision = ApplyRevertPolicy::decide(applied_at(), add(applied_at(), TimeSpan::from_hours(1)), ApplyRevertChoice::Confirm, TIMEOUT);

    assert_eq!(ApplyRevertDecision::Keep, decision);
}

#[test]
fn acik_geri_alma_zaman_dolmadan_geri_alir() {
    let decision = ApplyRevertPolicy::decide(applied_at(), add(applied_at(), secs(1.0)), ApplyRevertChoice::Revert, TIMEOUT);

    assert_eq!(ApplyRevertDecision::Revert, decision);
}

#[test]
fn kalan_sure_azalir_ve_sifirda_durur() {
    assert_eq!(TimeSpan::from_secs(15), ApplyRevertPolicy::remaining(applied_at(), applied_at(), TIMEOUT));
    assert_eq!(TimeSpan::from_secs(10), ApplyRevertPolicy::remaining(applied_at(), add(applied_at(), secs(5.0)), TIMEOUT));
    assert_eq!(TimeSpan::ZERO, ApplyRevertPolicy::remaining(applied_at(), add(applied_at(), secs(30.0)), TIMEOUT));
}

#[test]
fn sifir_veya_negatif_zaman_asimi_reddedilir() {
    assert!(std::panic::catch_unwind(|| {
        ApplyRevertPolicy::decide(applied_at(), applied_at(), ApplyRevertChoice::None, TimeSpan::ZERO)
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| ApplyRevertPolicy::remaining(applied_at(), applied_at(), TimeSpan::from_secs(-1))).is_err());
}

#[test]
fn remaining_never_exceeds_the_timeout_when_the_clock_moves_backward() {
    let applied = FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap();
    let earlier = add(applied, secs(-5.0));

    assert_eq!(TimeSpan::from_secs(15), ApplyRevertPolicy::remaining(applied, earlier, TimeSpan::from_secs(15)));
}

#[test]
fn remaining_does_not_overflow_with_a_huge_timeout_when_the_clock_moves_backward() {
    let earlier = add(applied_at(), TimeSpan::from_hours(-1));

    assert_eq!(TimeSpan::MAX, ApplyRevertPolicy::remaining(applied_at(), earlier, TimeSpan::MAX));
}

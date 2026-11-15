//! Karşılığı: `Rules/ContextProfileSwitcherTests.cs`.

use chrono::{FixedOffset, NaiveTime, TimeZone};

use crate::rules::*;
use crate::time::{DateTimeOffset, TimeSpan, add_span};

fn t0() -> DateTimeOffset {
    FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 12, 12, 0, 0).unwrap()
}

const STABLE: TimeSpan = TimeSpan::from_secs(1);

fn after(span: TimeSpan) -> DateTimeOffset {
    add_span(t0(), span)
}

fn ctx(process: &str) -> RuleContext {
    RuleContext::new(Some(process), NaiveTime::from_hms_opt(12, 0, 0).unwrap(), 1, false)
}

fn rule(process: &str, profile: &str) -> AutoRule {
    rule_enabled(process, profile, true)
}

fn rule_enabled(process: &str, profile: &str, enabled: bool) -> AutoRule {
    AutoRule::new(format!("r-{process}"), profile, enabled, vec![ProcessCondition::new(process).into()])
}

fn make(rules: &[AutoRule]) -> ContextProfileSwitcher {
    let mut s = ContextProfileSwitcher::new(STABLE);
    s.set_rules(rules);
    s
}

fn apply(profile: &str) -> SwitchDecision {
    SwitchDecision::new(SwitchActionKind::Apply, Some(profile), TimeSpan::ZERO)
}

#[test]
fn no_rules_never_acts() {
    let mut s = make(&[]);
    assert_eq!(SwitchActionKind::None, s.observe(&ctx("code"), t0()).kind);
    assert_eq!(SwitchActionKind::None, s.tick(after(TimeSpan::from_mins(1))).kind);
}

#[test]
fn match_waits_for_stability_then_applies() {
    let mut s = make(&[rule("game", "p1")]);
    let first = s.observe(&ctx("game"), t0());
    assert_eq!(SwitchActionKind::Wait, first.kind);
    assert_eq!(STABLE, first.delay);

    let mid = s.tick(after(TimeSpan::from_millis(400)));
    assert_eq!(SwitchActionKind::Wait, mid.kind);
    assert_eq!(TimeSpan::from_millis(600), mid.delay);

    let done = s.tick(after(STABLE));
    assert_eq!(apply("p1"), done);
}

#[test]
fn quick_alt_tab_away_and_back_never_applies() {
    let mut s = make(&[rule("game", "p1")]);
    assert_eq!(SwitchActionKind::Wait, s.observe(&ctx("game"), t0()).kind);
    // Eşleşmeyen uygulamaya geçildi: bekleyen eylem düşer.
    assert_eq!(SwitchActionKind::None, s.observe(&ctx("notepad"), after(TimeSpan::from_millis(300))).kind);
    assert_eq!(SwitchActionKind::None, s.tick(after(STABLE)).kind);
}

#[test]
fn flapping_restarts_the_stability_window() {
    let mut s = make(&[rule("a", "pa"), rule("b", "pb")]);
    s.observe(&ctx("a"), t0());
    s.observe(&ctx("b"), after(TimeSpan::from_millis(900)));
    // a'nın süresi dolsa da artık b bekliyor; b için tam süre geçmeden uygulanmaz.
    let early = s.tick(after(TimeSpan::from_millis(1500)));
    assert_eq!(SwitchActionKind::Wait, early.kind);
    assert_eq!(apply("pb"), s.tick(after(TimeSpan::from_millis(1900))));
}

#[test]
fn same_target_again_does_not_restart_the_window() {
    let mut s = make(&[rule("game", "p1")]);
    s.observe(&ctx("game"), t0());
    let again = s.observe(&ctx("game"), after(TimeSpan::from_millis(700)));
    assert_eq!(TimeSpan::from_millis(300), again.delay);
}

#[test]
fn observe_after_window_elapsed_applies_immediately() {
    let mut s = make(&[rule("game", "p1")]);
    s.observe(&ctx("game"), t0());
    assert_eq!(apply("p1"), s.observe(&ctx("game"), after(TimeSpan::from_secs(2))));
}

#[test]
fn already_active_profile_is_not_reapplied() {
    let mut s = make(&[rule("game", "p1")]);
    s.note_applied("p1");
    assert_eq!(SwitchActionKind::None, s.observe(&ctx("game"), t0()).kind);
    assert_eq!(SwitchActionKind::None, s.tick(after(STABLE)).kind);
}

#[test]
fn no_match_after_rule_applied_reverts_once_stable() {
    let mut s = make(&[rule("game", "p1")]);
    s.note_applied("p1");
    assert_eq!(SwitchActionKind::Wait, s.observe(&ctx("notepad"), t0()).kind);
    assert_eq!(SwitchDecision::new(SwitchActionKind::Revert, None, TimeSpan::ZERO), s.tick(after(STABLE)));
    s.note_reverted();
    assert!(!s.is_rule_active());
    // Geri dönüldükten sonra bir daha dönülmez.
    assert_eq!(SwitchActionKind::None, s.observe(&ctx("notepad"), after(TimeSpan::from_secs(5))).kind);
}

#[test]
fn no_match_without_rule_profile_active_never_reverts() {
    let mut s = make(&[rule("game", "p1")]);
    assert_eq!(SwitchActionKind::None, s.observe(&ctx("notepad"), t0()).kind);
}

#[test]
fn rule_to_rule_applies_second_directly_without_revert() {
    let mut s = make(&[rule("a", "pa"), rule("b", "pb")]);
    s.note_applied("pa");
    s.observe(&ctx("b"), t0());
    assert_eq!(apply("pb"), s.tick(after(STABLE)));
}

#[test]
fn revert_cancelled_when_matching_app_returns_before_stable() {
    let mut s = make(&[rule("game", "p1")]);
    s.note_applied("p1");
    assert_eq!(SwitchActionKind::Wait, s.observe(&ctx("notepad"), t0()).kind);
    assert_eq!(SwitchActionKind::None, s.observe(&ctx("game"), after(TimeSpan::from_millis(200))).kind);
    assert_eq!(SwitchActionKind::None, s.tick(after(STABLE)).kind);
    assert!(s.is_rule_active());
}

#[test]
fn disabled_rules_are_ignored() {
    let mut s = make(&[rule_enabled("game", "p1", false)]);
    assert_eq!(SwitchActionKind::None, s.observe(&ctx("game"), t0()).kind);
}

#[test]
fn reset_after_manual_apply_does_not_revert() {
    let mut s = make(&[rule("game", "p1")]);
    s.note_applied("p1");
    s.reset();
    assert_eq!(SwitchActionKind::None, s.observe(&ctx("notepad"), t0()).kind);
    assert_eq!(SwitchActionKind::Wait, s.observe(&ctx("game"), t0()).kind);
}

#[test]
fn set_rules_drops_pending_but_keeps_active_rule_profile() {
    let mut s = make(&[rule("game", "p1")]);
    s.observe(&ctx("game"), t0());
    s.set_rules(&[rule("game", "p1")]);
    assert_eq!(SwitchActionKind::None, s.tick(after(STABLE)).kind);

    s.note_applied("p1");
    s.set_rules(&[]);
    assert!(s.is_rule_active());
    // Kurallar silindi ve kural profili yürürlükte: kararlı olunca geri dön.
    s.observe(&ctx("game"), t0());
    assert_eq!(SwitchActionKind::Revert, s.tick(after(STABLE)).kind);
}

#[test]
fn note_calls_never_produce_actions() {
    // Uygulama sonucu bildirimi yeniden değerlendirme tetiklemez: döngü yok.
    let mut s = make(&[rule("game", "p1")]);
    s.note_applied("p1");
    s.note_reverted();
    s.reset();
    assert_eq!(SwitchActionKind::None, s.tick(after(TimeSpan::from_hours(1))).kind);
}

#[test]
fn apply_then_revert_cycle_produces_exactly_one_action_each() {
    let mut s = make(&[rule("game", "p1")]);
    let mut actions: Vec<SwitchActionKind> = Vec::new();
    let mut feed = |s: &mut ContextProfileSwitcher, process: &str, seconds: f64| {
        let now = after(TimeSpan::from_seconds_f64(seconds));
        let mut d = s.observe(&ctx(process), now);
        if d.kind == SwitchActionKind::Wait {
            d = s.tick(add_span(now, d.delay));
        }
        if d.kind == SwitchActionKind::Apply {
            s.note_applied(d.profile_id.as_deref().unwrap());
        } else if d.kind == SwitchActionKind::Revert {
            s.note_reverted();
        }
        if matches!(d.kind, SwitchActionKind::Apply | SwitchActionKind::Revert) {
            actions.push(d.kind);
        }
    };

    feed(&mut s, "game", 0.0);
    feed(&mut s, "game", 10.0);
    feed(&mut s, "game", 20.0);
    feed(&mut s, "notepad", 30.0);
    feed(&mut s, "notepad", 40.0);
    assert_eq!(vec![SwitchActionKind::Apply, SwitchActionKind::Revert], actions);
}

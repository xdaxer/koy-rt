//! Karşılığı: `Rules/RuleEvaluatorTests.cs`.

use chrono::NaiveTime;

use crate::rules::*;

fn noon() -> NaiveTime {
    NaiveTime::from_hms_opt(12, 0, 0).unwrap()
}

fn hm(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

/// C#: `Context(process = "code", time = null, displays = 1, fullscreen = false)`.
fn context(process: Option<&str>, time: Option<NaiveTime>, displays: i32, fullscreen: bool) -> RuleContext {
    RuleContext::new(process, time.unwrap_or_else(noon), displays, fullscreen)
}

fn ctx() -> RuleContext {
    context(Some("code"), None, 1, false)
}

fn rule(profile: &str, conditions: Vec<RuleCondition>) -> AutoRule {
    AutoRule::new(format!("kural-{profile}"), profile, true, conditions)
}

fn process(name: &str) -> RuleCondition {
    ProcessCondition::new(name).into()
}

fn window(from: NaiveTime, to: NaiveTime) -> RuleCondition {
    TimeWindowCondition::new(from, to).into()
}

#[test]
fn no_rules_returns_none() {
    assert_eq!(None, RuleEvaluator::evaluate(&[], &ctx()));
}

#[test]
fn process_condition_matches_case_insensitively_ignoring_exe_suffix() {
    for context_process in ["code", "Code.exe", "  CODE  "] {
        let rules = [rule("work", vec![process("code")])];
        assert_eq!(
            Some("work".to_string()),
            RuleEvaluator::evaluate(&rules, &context(Some(context_process), None, 1, false)),
            "{context_process}"
        );
    }
}

#[test]
fn process_condition_without_foreground_process_does_not_match() {
    let rules = [rule("work", vec![process("code")])];
    assert_eq!(None, RuleEvaluator::evaluate(&rules, &context(None, None, 1, false)));
}

#[test]
fn first_matching_rule_wins_by_list_order() {
    let rules = [rule("first", vec![process("code")]), rule("second", vec![process("code")])];
    assert_eq!(Some("first".to_string()), RuleEvaluator::evaluate(&rules, &ctx()));
}

#[test]
fn disabled_rule_is_skipped() {
    let disabled = AutoRule::new("kapali", "first", false, vec![process("code")]);
    let rules = [disabled, rule("second", vec![process("code")])];
    assert_eq!(Some("second".to_string()), RuleEvaluator::evaluate(&rules, &ctx()));
}

#[test]
fn all_conditions_must_match() {
    let rules = [rule("work", vec![process("code"), FullscreenCondition::new(true).into()])];
    assert_eq!(None, RuleEvaluator::evaluate(&rules, &context(Some("code"), None, 1, false)));
    assert_eq!(Some("work".to_string()), RuleEvaluator::evaluate(&rules, &context(Some("code"), None, 1, true)));
}

#[test]
fn same_day_time_window_start_inclusive_end_exclusive() {
    let cases = [(12, 0, true), (9, 0, true), (16, 59, true), (17, 0, false), (8, 59, false)];
    for (hour, minute, expected) in cases {
        let rules = [rule("mesai", vec![window(hm(9, 0), hm(17, 0))])];
        let result = RuleEvaluator::evaluate(&rules, &context(Some("code"), Some(hm(hour, minute)), 1, false));
        assert_eq!(expected.then(|| "mesai".to_string()), result, "{hour}:{minute}");
    }
}

#[test]
fn midnight_spanning_time_window_wraps_around() {
    let cases = [(23, 30, true), (22, 0, true), (3, 0, true), (6, 0, false), (12, 0, false)];
    for (hour, minute, expected) in cases {
        let rules = [rule("uyku", vec![window(hm(22, 0), hm(6, 0))])];
        let result = RuleEvaluator::evaluate(&rules, &context(Some("code"), Some(hm(hour, minute)), 1, false));
        assert_eq!(expected.then(|| "uyku".to_string()), result, "{hour}:{minute}");
    }
}

#[test]
fn display_count_matches_exactly() {
    for (displays, expected) in [(1, false), (2, true), (3, false)] {
        let rules = [rule("iki-ekran", vec![DisplayCountCondition::new(2).into()])];
        let result = RuleEvaluator::evaluate(&rules, &context(Some("code"), None, displays, false));
        assert_eq!(expected.then(|| "iki-ekran".to_string()), result, "{displays}");
    }
}

#[test]
fn fullscreen_false_matches_only_when_not_fullscreen() {
    let rules = [rule("normal", vec![FullscreenCondition::new(false).into()])];
    assert_eq!(Some("normal".to_string()), RuleEvaluator::evaluate(&rules, &context(Some("code"), None, 1, false)));
    assert_eq!(None, RuleEvaluator::evaluate(&rules, &context(Some("code"), None, 1, true)));
}

#[test]
fn rule_without_conditions_never_matches() {
    let rules = [AutoRule::new("bos", "work", true, vec![])];
    assert_eq!(None, RuleEvaluator::evaluate(&rules, &ctx()));
}

#[test]
fn window_ending_at_midnight_covers_until_end_of_day() {
    let cases = [(23, 59, true), (22, 0, true), (0, 0, false)];
    for (hour, minute, expected) in cases {
        let rules = [rule("aksam", vec![window(hm(22, 0), hm(0, 0))])];
        let result = RuleEvaluator::evaluate(&rules, &context(Some("code"), Some(hm(hour, minute)), 1, false));
        assert_eq!(expected.then(|| "aksam".to_string()), result, "{hour}:{minute}");
    }
}

#[test]
fn midnight_spanning_window_excludes_end_instant_includes_seconds_before() {
    let rules = [rule("uyku", vec![window(hm(22, 0), hm(6, 0))])];
    let before_end = NaiveTime::from_hms_opt(5, 59, 59).unwrap();
    assert_eq!(Some("uyku".to_string()), RuleEvaluator::evaluate(&rules, &context(Some("code"), Some(before_end), 1, false)));
    assert_eq!(None, RuleEvaluator::evaluate(&rules, &context(Some("code"), Some(hm(6, 0)), 1, false)));
}

#[test]
fn equal_from_and_to_never_matches_even_when_built_without_validator() {
    let rules = [rule("belirsiz", vec![window(hm(9, 0), hm(9, 0))])];
    assert_eq!(None, RuleEvaluator::evaluate(&rules, &context(Some("code"), Some(hm(9, 0)), 1, false)));
    assert_eq!(None, RuleEvaluator::evaluate(&rules, &context(Some("code"), Some(hm(15, 0)), 1, false)));
}

#[test]
fn unknown_condition_type_never_matches() {
    let rules = [rule("x", vec![RuleCondition::Unknown])];
    assert_eq!(None, RuleEvaluator::evaluate(&rules, &ctx()));
}

#[test]
fn process_with_path_never_matches() {
    let rules = [rule("work", vec![process("code")])];
    assert_eq!(None, RuleEvaluator::evaluate(&rules, &context(Some(r"C:\apps\code.exe"), None, 1, false)));
}

#[test]
fn conflicting_rules_resolve_by_order_and_swapping_order_swaps_winner() {
    let a = rule("a", vec![process("code")]);
    let b = rule("b", vec![process("code"), DisplayCountCondition::new(1).into()]);

    assert_eq!(Some("a".to_string()), RuleEvaluator::evaluate(&[a.clone(), b.clone()], &ctx()));
    assert_eq!(Some("b".to_string()), RuleEvaluator::evaluate(&[b, a], &ctx()));
}

#[test]
fn evaluation_is_deterministic_for_same_input() {
    let rules = [rule("a", vec![process("code")]), rule("b", vec![DisplayCountCondition::new(1).into()])];
    let context = ctx();

    let mut results: Vec<Option<String>> = Vec::new();
    for _ in 0..5 {
        let r = RuleEvaluator::evaluate(&rules, &context);
        if !results.contains(&r) {
            results.push(r);
        }
    }

    assert_eq!(vec![Some("a".to_string())], results);
}

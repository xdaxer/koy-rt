//! Karşılığı: `Rules/RuleListEditorTests.cs`.

use crate::rules::*;

#[test]
fn add_process_rule_normalizes_and_adds_disabled() {
    let result = RuleListEditor::add_process_rule(&[], Some(" Game.EXE "), Some(" p1 "));
    assert!(result.is_success());
    assert_eq!(1, result.rules.len());
    let rule = &result.rules[0];
    assert!(!rule.enabled);
    assert_eq!(Some("p1"), rule.profile_id.as_deref());
    assert_eq!(Some("game".to_string()), RuleListEditor::process_of(rule));

    let validated = RuleSetValidator::validate(&result.rules);
    assert_eq!(1, validated.rules.len());
    assert_eq!(0, validated.dropped_rules);
}

#[test]
fn add_process_rule_rejects_bad_process() {
    for process in [None, Some(""), Some("C:\\x\\game.exe")] {
        assert_eq!(
            RuleEditError::InvalidProcess,
            RuleListEditor::add_process_rule(&[], process, Some("p")).error,
            "{process:?}"
        );
    }
}

#[test]
fn add_process_rule_requires_profile() {
    for profile in [None, Some("  ")] {
        assert_eq!(RuleEditError::NoProfile, RuleListEditor::add_process_rule(&[], Some("game"), profile).error, "{profile:?}");
    }
}

#[test]
fn add_process_rule_rejects_duplicate_process() {
    let first = RuleListEditor::add_process_rule(&[], Some("game"), Some("p1"));
    let second = RuleListEditor::add_process_rule(&first.rules, Some("GAME.exe"), Some("p2"));
    assert_eq!(RuleEditError::Duplicate, second.error);
    assert_eq!(1, second.rules.len());
}

#[test]
fn add_process_rule_respects_limit() {
    let mut rules: Vec<RuleDraft> = Vec::new();
    for i in 0..RuleLimits::MAX_RULES {
        rules = RuleListEditor::add_process_rule(&rules, Some(&format!("app{i}")), Some("p")).rules;
    }
    assert_eq!(RuleEditError::LimitReached, RuleListEditor::add_process_rule(&rules, Some("extra"), Some("p")).error);
}

#[test]
fn set_enabled_and_delete_work_by_index_and_reject_bad_index() {
    let mut rules = RuleListEditor::add_process_rule(&[], Some("a"), Some("p")).rules;
    rules = RuleListEditor::add_process_rule(&rules, Some("b"), Some("p")).rules;

    let enabled = RuleListEditor::set_enabled(&rules, 1, true);
    assert!(enabled.rules[1].enabled);
    assert!(!enabled.rules[0].enabled);

    let deleted = RuleListEditor::delete(&enabled.rules, 0);
    assert_eq!(1, deleted.rules.len());
    assert_eq!(Some("b".to_string()), RuleListEditor::process_of(&deleted.rules[0]));

    assert_eq!(RuleEditError::NotFound, RuleListEditor::set_enabled(&rules, 5, true).error);
    assert_eq!(RuleEditError::NotFound, RuleListEditor::delete(&rules, -1).error);
}

#[test]
fn process_of_is_none_for_advanced_rules() {
    let advanced = RuleDraft::new(
        Some("x"),
        Some("p"),
        true,
        vec![RuleConditionDraft::process(Some("game")), RuleConditionDraft::fullscreen(Some(true))],
    );
    assert_eq!(None, RuleListEditor::process_of(&advanced));
}

//! Karşılığı: `Rules/RuleSetValidatorTests.cs`.

use chrono::NaiveTime;

use crate::rules::*;

fn draft(name: Option<&str>, profile: Option<&str>, conditions: Vec<RuleConditionDraft>) -> RuleDraft {
    RuleDraft::new(name, profile, true, conditions)
}

fn kod(conditions: Vec<RuleConditionDraft>) -> RuleDraft {
    draft(Some("Kod"), Some("work"), conditions)
}

fn process(name: &str) -> RuleConditionDraft {
    RuleConditionDraft::process(Some(name))
}

#[test]
fn valid_rule_is_kept_with_trimmed_name_and_profile() {
    let result = RuleSetValidator::validate(&[draft(Some("  Kod  "), Some(" work "), vec![process("Code.exe")])]);

    assert_eq!(1, result.rules.len());
    assert_eq!("Kod", result.rules[0].name);
    assert_eq!("work", result.rules[0].profile_id);
    assert_eq!(0, result.dropped_rules);
}

#[test]
fn empty_name_is_rejected() {
    for name in [None, Some(""), Some("   ")] {
        let result = RuleSetValidator::validate(&[draft(name, Some("work"), vec![process("code")])]);

        assert!(result.rules.is_empty(), "{name:?}");
        assert_eq!(1, result.dropped_rules, "{name:?}");
    }
}

#[test]
fn name_longer_than_limit_is_rejected() {
    let name = "a".repeat(RuleLimits::MAX_NAME_LENGTH + 1);

    let result = RuleSetValidator::validate(&[draft(Some(&name), Some("work"), vec![process("code")])]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_rules);
}

#[test]
fn empty_profile_id_is_rejected() {
    for profile in [None, Some(""), Some("  ")] {
        let result = RuleSetValidator::validate(&[draft(Some("Kod"), profile, vec![process("code")])]);

        assert!(result.rules.is_empty(), "{profile:?}");
        assert_eq!(1, result.dropped_rules, "{profile:?}");
    }
}

#[test]
fn unknown_condition_kind_is_dropped_and_whole_rule_is_dropped_fail_closed() {
    let result = RuleSetValidator::validate(&[kod(vec![RuleConditionDraft::new(Some("wifiSsid")), process("code")])]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_conditions);
    assert_eq!(1, result.dropped_rules);
}

#[test]
fn invalid_time_window_alongside_valid_process_drops_whole_rule_not_only_process_condition() {
    // Yalnız "süreç=oyun" kalırsa kural gün boyu tetiklenir; bu yüzden tüm kural atılmalı.
    let result = RuleSetValidator::validate(&[draft(
        Some("Oyun"),
        Some("game"),
        vec![process("game"), RuleConditionDraft::time_window(Some("25:00"), Some("06:00"))],
    )]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_conditions);
    assert_eq!(1, result.dropped_rules);
}

#[test]
fn dropped_rule_does_not_affect_other_valid_rules() {
    let result = RuleSetValidator::validate(&[
        draft(Some("Birinci"), Some("work"), vec![process("code")]),
        draft(Some("Bozuk"), Some("game"), vec![process("game"), RuleConditionDraft::new(Some("battery"))]),
        draft(Some("Üçüncü"), Some("sleep"), vec![RuleConditionDraft::time_window(Some("22:00"), Some("06:30"))]),
    ]);

    let names: Vec<&str> = result.rules.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(vec!["Birinci", "Üçüncü"], names);
    assert_eq!(1, result.dropped_rules);
}

#[test]
fn rule_whose_only_conditions_are_unknown_is_dropped_not_made_unconditional() {
    let result = RuleSetValidator::validate(&[kod(vec![RuleConditionDraft::new(Some("battery"))])]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_conditions);
    assert_eq!(1, result.dropped_rules);
}

#[test]
fn rule_without_conditions_is_dropped() {
    let result = RuleSetValidator::validate(&[kod(vec![])]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_rules);
}

#[test]
fn condition_kind_is_case_insensitive() {
    let result = RuleSetValidator::validate(&[kod(vec![RuleConditionDraft {
        kind: Some("PROCESS".into()),
        process_name: Some("code".into()),
        ..Default::default()
    }])]);

    assert_eq!(1, result.rules.len());
    assert_eq!(0, result.dropped_conditions);
}

#[test]
fn invalid_or_empty_time_window_is_dropped() {
    let cases = [
        (None, Some("17:00")),
        (Some("09:00"), None),
        (Some("9am"), Some("17:00")),
        (Some("09:00"), Some("09:00")),
    ];
    for (from, to) in cases {
        let result = RuleSetValidator::validate(&[kod(vec![RuleConditionDraft::time_window(from, to)])]);

        assert!(result.rules.is_empty(), "{from:?} {to:?}");
        assert_eq!(1, result.dropped_conditions, "{from:?} {to:?}");
    }
}

#[test]
fn valid_time_window_is_parsed() {
    let result = RuleSetValidator::validate(&[draft(
        Some("Gece"),
        Some("sleep"),
        vec![RuleConditionDraft::time_window(Some("22:00"), Some("06:30"))],
    )]);

    assert_eq!(1, result.rules.len());
    assert_eq!(1, result.rules[0].conditions.len());
    let RuleCondition::TimeWindow(condition) = &result.rules[0].conditions[0] else {
        panic!("TimeWindowCondition bekleniyordu");
    };
    assert_eq!(NaiveTime::from_hms_opt(22, 0, 0).unwrap(), condition.from);
    assert_eq!(NaiveTime::from_hms_opt(6, 30, 0).unwrap(), condition.to);
}

#[test]
fn display_count_out_of_range_is_dropped() {
    for count in [0, -1, 17] {
        let result = RuleSetValidator::validate(&[kod(vec![RuleConditionDraft::display_count(Some(count))])]);

        assert!(result.rules.is_empty(), "{count}");
        assert_eq!(1, result.dropped_conditions, "{count}");
    }
}

#[test]
fn fullscreen_without_value_is_dropped() {
    let result = RuleSetValidator::validate(&[kod(vec![RuleConditionDraft::fullscreen(None)])]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_conditions);
}

#[test]
fn normalize_process_name_handles_case_suffix_and_paths() {
    let cases: [(Option<&str>, Option<&str>); 6] = [
        (Some("  Code.EXE "), Some("code")),
        (Some("chrome"), Some("chrome")),
        (Some(".exe"), None),
        (Some(r"C:\apps\code.exe"), None),
        (Some(""), None),
        (None, None),
    ];
    for (raw, expected) in cases {
        assert_eq!(expected.map(String::from), RuleSetValidator::normalize_process_name(raw), "{raw:?}");
    }
}

#[test]
fn invalid_process_name_is_dropped() {
    let result = RuleSetValidator::validate(&[kod(vec![process(r"C:\apps\code.exe")])]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_conditions);
}

#[test]
fn rules_over_limit_are_dropped_from_the_end_and_order_is_kept() {
    let drafts: Vec<RuleDraft> = (0..RuleLimits::MAX_RULES + 3)
        .map(|i| draft(Some(&format!("Kural {i}")), Some(&format!("profil-{i}")), vec![process(&format!("app{i}"))]))
        .collect();

    let result = RuleSetValidator::validate(&drafts);

    assert_eq!(RuleLimits::MAX_RULES, result.rules.len());
    assert_eq!(3, result.dropped_rules);
    assert_eq!("profil-0", result.rules[0].profile_id);
    assert_eq!(format!("profil-{}", RuleLimits::MAX_RULES - 1), result.rules.last().unwrap().profile_id);
}

#[test]
fn rules_over_limit_do_not_parse_their_conditions() {
    let mut drafts: Vec<RuleDraft> = (0..RuleLimits::MAX_RULES)
        .map(|i| draft(Some(&format!("Kural {i}")), Some("p"), vec![process(&format!("app{i}"))]))
        .collect();
    drafts.push(draft(Some("Fazla"), Some("p"), vec![RuleConditionDraft::new(Some("battery"))]));

    let result = RuleSetValidator::validate(&drafts);

    assert_eq!(RuleLimits::MAX_RULES, result.rules.len());
    assert_eq!(1, result.dropped_rules);
    assert_eq!(0, result.dropped_conditions);
}

#[test]
fn too_many_conditions_drops_rule() {
    let conditions: Vec<RuleConditionDraft> = (0..RuleLimits::MAX_CONDITIONS_PER_RULE + 1).map(|i| process(&format!("app{i}"))).collect();

    let result = RuleSetValidator::validate(&[kod(conditions)]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_rules);
}

#[test]
fn conditions_at_limit_are_kept() {
    let conditions: Vec<RuleConditionDraft> = (0..RuleLimits::MAX_CONDITIONS_PER_RULE).map(|i| process(&format!("app{i}"))).collect();

    let result = RuleSetValidator::validate(&[kod(conditions)]);
    assert_eq!(1, result.rules.len());
    assert_eq!(RuleLimits::MAX_CONDITIONS_PER_RULE, result.rules[0].conditions.len());
}

#[test]
fn overlong_profile_id_is_rejected() {
    let profile = "p".repeat(RuleLimits::MAX_PROFILE_ID_LENGTH + 1);

    let result = RuleSetValidator::validate(&[draft(Some("Kod"), Some(&profile), vec![process("code")])]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_rules);
}

#[test]
fn overlong_process_name_is_dropped() {
    let name = "a".repeat(RuleLimits::MAX_PROCESS_NAME_LENGTH + 1);

    let result = RuleSetValidator::validate(&[kod(vec![process(&name)])]);

    assert!(result.rules.is_empty());
    assert_eq!(1, result.dropped_conditions);
}

#[test]
fn null_draft_null_conditions_and_null_condition_are_dropped_without_panicking() {
    // El ile düzenlenmiş JSON'dan null liste/öğe gelebilir; Rust'ta null taslak = `RuleDraft::default()`,
    // null liste = boş liste, null koşul = `RuleConditionDraft::default()` (bkz. rule_draft.rs).
    let result = RuleSetValidator::validate(&[
        RuleDraft::default(),
        RuleDraft::new(Some("A"), Some("work"), true, vec![]),
        RuleDraft::new(Some("B"), Some("work"), true, vec![RuleConditionDraft::default()]),
        draft(Some("C"), Some("work"), vec![process("code")]),
    ]);

    assert_eq!(1, result.rules.len());
    assert_eq!("C", result.rules[0].name);
    assert_eq!(3, result.dropped_rules);
}

#[test]
fn time_window_tolerates_surrounding_whitespace() {
    let result = RuleSetValidator::validate(&[draft(
        Some("Gece"),
        Some("sleep"),
        vec![RuleConditionDraft::time_window(Some(" 22:00 "), Some("06:00\t"))],
    )]);

    assert_eq!(1, result.rules.len());
}

#[test]
fn nonexistent_clock_times_are_dropped() {
    for (from, to) in [("24:00", "06:00"), ("22:00", "24:00"), ("22:00", "06:60")] {
        let result = RuleSetValidator::validate(&[draft(
            Some("Gece"),
            Some("sleep"),
            vec![RuleConditionDraft::time_window(Some(from), Some(to))],
        )]);

        assert!(result.rules.is_empty(), "{from} {to}");
    }
}

#[test]
fn disabled_rule_is_kept_with_enabled_false() {
    let d = RuleDraft::new(Some("Kod"), Some("work"), false, vec![process("code")]);

    let result = RuleSetValidator::validate(&[d]);
    assert_eq!(1, result.rules.len());
    assert!(!result.rules[0].enabled);
}

#[test]
fn json_null_elements_become_default_drafts() {
    // Rust'a özgü: JSON `null` liste ve öğeleri C#'taki null davranışına eşlenir.
    let json = r#"[null, {"Name":"A","ProfileId":"p","Enabled":true,"Conditions":null},
        {"Name":"B","ProfileId":"p","Enabled":true,"Conditions":[null]},
        {"Name":"C","ProfileId":"p","Enabled":true,"Conditions":[{"Kind":"process","ProcessName":"code"}]}]"#;
    let drafts: Vec<RuleDraft> = serde_json::from_str(json).unwrap();

    let result = RuleSetValidator::validate(&drafts);

    assert_eq!(1, result.rules.len());
    assert_eq!("C", result.rules[0].name);
    assert_eq!(3, result.dropped_rules);
}

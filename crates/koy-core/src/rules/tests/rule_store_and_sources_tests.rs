//! Karşılığı: `Rules/RuleStoreAndSourcesTests.cs`.

use std::path::PathBuf;

use chrono::NaiveTime;

use crate::rules::*;

/// C#: `_dir` + `Dispose` (geçici klasör, testten sonra silinir).
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        TempDir(std::env::temp_dir().join(format!("koy-rules-{nanos:x}-{n}-{}", std::process::id())))
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_file_means_no_rules() {
    let dir = TempDir::new();
    let store = RuleStore::new(dir.file("rules.json"));
    assert!(store.load_drafts().is_empty());
    assert!(store.load_rules().is_empty());
}

#[test]
fn save_then_load_round_trips_conditions_in_order() {
    let dir = TempDir::new();
    let store = RuleStore::new(dir.file("rules.json"));
    let drafts = vec![
        RuleDraft::new(Some("one"), Some("p1"), true, vec![RuleConditionDraft::process(Some("game"))]),
        RuleDraft::new(
            Some("two"),
            Some("p2"),
            false,
            vec![
                RuleConditionDraft::time_window(Some("22:00"), Some("06:00")),
                RuleConditionDraft::display_count(Some(2)),
                RuleConditionDraft::fullscreen(Some(true)),
            ],
        ),
    ];
    assert!(store.save(&drafts));

    let rules = store.load_rules();
    let profiles: Vec<&str> = rules.iter().map(|r| r.profile_id.as_str()).collect();
    assert_eq!(vec!["p1", "p2"], profiles);
    assert!(rules[0].enabled);
    assert!(!rules[1].enabled);
    assert_eq!(3, rules[1].conditions.len());
}

#[test]
fn corrupt_file_yields_no_rules_without_panicking() {
    let dir = TempDir::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    let path = dir.file("rules.json");
    std::fs::write(&path, "{ not json").unwrap();
    let store = RuleStore::new(path);
    assert!(store.load_rules().is_empty());
}

#[test]
fn sources_none_when_no_enabled_rules() {
    assert_eq!(RuleSource::None, RuleSources::needed(&[]));
    let disabled = AutoRule::new("a", "p", false, vec![ProcessCondition::new("game").into()]);
    assert_eq!(RuleSource::None, RuleSources::needed(&[disabled]));
}

#[test]
fn sources_union_of_enabled_rule_conditions() {
    let a = AutoRule::new("a", "p", true, vec![ProcessCondition::new("game").into(), FullscreenCondition::new(true).into()]);
    let b = AutoRule::new("b", "p", true, vec![DisplayCountCondition::new(2).into()]);
    let c = AutoRule::new(
        "c",
        "p",
        false,
        vec![TimeWindowCondition::new(NaiveTime::from_hms_opt(1, 0, 0).unwrap(), NaiveTime::from_hms_opt(2, 0, 0).unwrap()).into()],
    );
    assert_eq!(RuleSource::Process | RuleSource::Fullscreen | RuleSource::DisplayCount, RuleSources::needed(&[a, b, c]));
}

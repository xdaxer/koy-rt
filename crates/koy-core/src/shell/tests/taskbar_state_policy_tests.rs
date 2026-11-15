use chrono::NaiveDate;

use crate::shell::{TaskbarSavedState, TaskbarStatePolicy};

const ALWAYS_ON_TOP: i32 = TaskbarStatePolicy::ALWAYS_ON_TOP;
const AUTO_HIDE: i32 = TaskbarStatePolicy::AUTO_HIDE;

#[test]
fn choose_previous_state_kayit_yoksa_anlik_durumu_kullanir() {
    assert_eq!(ALWAYS_ON_TOP, TaskbarStatePolicy::choose_previous_state(None, ALWAYS_ON_TOP));
}

#[test]
fn choose_previous_state_zaten_gizliyse_kayitli_onceki_durumu_korur() {
    // İkinci Hide(): anlık durum bizim koyduğumuz AUTOHIDE; kullanıcının gerçek durumu AlwaysOnTop kalmalı.
    let saved = TaskbarSavedState::new(true, ALWAYS_ON_TOP);

    let chosen = TaskbarStatePolicy::choose_previous_state(Some(saved), AUTO_HIDE);

    assert_eq!(ALWAYS_ON_TOP, chosen);
}

#[test]
fn choose_previous_state_kayit_gizli_degilse_anlik_durumu_kullanir() {
    let saved = TaskbarSavedState::new(false, ALWAYS_ON_TOP);

    assert_eq!(AUTO_HIDE, TaskbarStatePolicy::choose_previous_state(Some(saved), AUTO_HIDE));
}

#[test]
fn choose_previous_state_iki_kez_hide_durumu_ezmez() {
    let mut current = ALWAYS_ON_TOP;
    let mut file: Option<TaskbarSavedState> = None;

    // 1. Hide
    let mut prev = TaskbarStatePolicy::choose_previous_state(file, current);
    file = Some(TaskbarSavedState::new(true, prev));
    current = AUTO_HIDE; // Hide AUTOHIDE koydu

    // 2. Hide
    prev = TaskbarStatePolicy::choose_previous_state(file, current);
    file = Some(TaskbarSavedState::new(true, prev));

    assert_eq!(ALWAYS_ON_TOP, TaskbarStatePolicy::resolve_restore_state(file));
}

#[test]
fn resolve_restore_state_gecersiz_kayit_always_on_top_a_duser() {
    for previous in [0, -1] {
        assert_eq!(ALWAYS_ON_TOP, TaskbarStatePolicy::resolve_restore_state(Some(TaskbarSavedState::new(true, previous))), "previous {previous}");
        assert_eq!(ALWAYS_ON_TOP, TaskbarStatePolicy::resolve_restore_state(None));
    }
}

#[test]
fn resolve_restore_state_kullanicinin_auto_hide_tercihini_korur() {
    assert_eq!(AUTO_HIDE, TaskbarStatePolicy::resolve_restore_state(Some(TaskbarSavedState::new(true, AUTO_HIDE))));
}

#[test]
fn serialize_parse_gidis_donus() {
    let now = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap().and_hms_opt(0, 0, 0).unwrap();
    let json = TaskbarStatePolicy::serialize(TaskbarSavedState::new(true, 3), now);

    // "O" biçimi: C# ile aynı metin.
    assert_eq!(r#"{"taskbarHidden": true, "previousState": 3, "timestamp": "2026-11-05T00:00:00.0000000Z"}"#, json);

    let parsed = TaskbarStatePolicy::parse(Some(&json));

    assert_eq!(Some(TaskbarSavedState::new(true, 3)), parsed);
}

#[test]
fn parse_bozuk_icerik_none_doner() {
    for content in [None, Some(""), Some("bozuk"), Some("{\"previousState\": abc}")] {
        assert_eq!(None, TaskbarStatePolicy::parse(content), "{content:?}");
    }
}

#[test]
fn should_write_state_kayit_yokken_ve_force_false_ise_false() {
    assert!(!TaskbarStatePolicy::should_write_state(None, false));
}

#[test]
fn should_write_state_kayit_hidden_degilse_ve_force_false_ise_false() {
    let saved = TaskbarSavedState::new(false, AUTO_HIDE);
    assert!(!TaskbarStatePolicy::should_write_state(Some(saved), false));
}

#[test]
fn should_write_state_kayit_hidden_iken_force_false_bile_olsa_true() {
    let saved = TaskbarSavedState::new(true, ALWAYS_ON_TOP);
    assert!(TaskbarStatePolicy::should_write_state(Some(saved), false));
}

#[test]
fn should_write_state_force_true_ise_her_zaman_true() {
    for has_record in [true, false] {
        let saved = if has_record { Some(TaskbarSavedState::new(false, 2)) } else { None };
        assert!(TaskbarStatePolicy::should_write_state(saved, true), "hasRecord {has_record}");
    }
}

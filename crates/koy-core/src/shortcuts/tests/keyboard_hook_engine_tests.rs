//! Karşılığı: `KeyboardHookEngineTests.cs` (`KeyboardHookEngineTests` sınıfı).

use std::collections::HashMap;

use crate::shortcuts::*;

const L_WIN: i32 = 0x5B;
const L_SHIFT: i32 = 0xA0;
const L_CTRL: i32 = 0xA2;
const L_ALT: i32 = 0xA4;
const S: i32 = 0x53;
const X: i32 = 0x58;
const ESC: i32 = 0x1B;
const ID: i32 = 6;

fn parse(text: &str) -> Hotkey {
    Hotkey::try_parse(text).unwrap_or_else(|| panic!("{text}"))
}

fn engine(bindings: &[&str]) -> KeyboardHookEngine {
    let engine = KeyboardHookEngine::new();
    let mut map: HashMap<i32, Hotkey> = HashMap::new();
    for (i, b) in bindings.iter().enumerate() {
        map.insert(ID + i as i32, parse(b));
    }
    engine.set_bindings(&map);
    engine
}

fn down(e: &KeyboardHookEngine, vk: i32) -> HookOutcome {
    e.process(vk, true, 0, None)
}

fn down_at(e: &KeyboardHookEngine, vk: i32, now: i64) -> HookOutcome {
    e.process(vk, true, now, None)
}

fn down_physically(e: &KeyboardHookEngine, vk: i32, physically: &dyn Fn(i32) -> bool) -> HookOutcome {
    e.process(vk, true, 0, Some(physically))
}

fn up(e: &KeyboardHookEngine, vk: i32) -> HookOutcome {
    e.process(vk, false, 0, None)
}

#[test]
fn win_alone_fires_on_release_and_masks_the_start_menu() {
    let e = engine(&["Win"]);

    assert_eq!(HookOutcome::PASS, down(&e, L_WIN));

    let u = up(&e, L_WIN);
    assert_eq!(Some(ID), u.fired_id);
    assert!(u.mask_modifier);
    assert!(!u.swallow);
}

#[test]
fn win_with_another_key_does_not_fire_win_alone() {
    let e = engine(&["Win"]);

    down(&e, L_WIN);
    assert_eq!(HookOutcome::PASS, down(&e, X));
    assert_eq!(HookOutcome::PASS, up(&e, X));

    assert_eq!(HookOutcome::PASS, up(&e, L_WIN));
}

#[test]
fn win_alone_is_left_to_windows_when_not_bound() {
    let e = engine(&["Win+Shift"]);

    down(&e, L_WIN);

    assert_eq!(HookOutcome::PASS, up(&e, L_WIN));
}

#[test]
fn two_modifiers_fire_when_both_are_released_in_either_order() {
    let e = engine(&["Win+Shift"]);

    down(&e, L_WIN);
    down(&e, L_SHIFT);

    let win_up = up(&e, L_WIN);
    assert_eq!(None, win_up.fired_id);
    assert!(win_up.mask_modifier);

    assert_eq!(Some(ID), up(&e, L_SHIFT).fired_id);
}

#[test]
fn shift_alone_does_not_need_a_mask() {
    let e = engine(&["Shift"]);

    down(&e, L_SHIFT);
    let u = up(&e, L_SHIFT);

    assert_eq!(Some(ID), u.fired_id);
    assert!(!u.mask_modifier);
}

#[test]
fn windows_shortcut_is_overridden_and_start_stays_closed() {
    let e = engine(&["Win+S"]);

    down(&e, L_WIN);
    let key = down(&e, S);
    assert!(key.swallow);
    assert_eq!(Some(ID), key.fired_id);

    assert!(up(&e, S).swallow);

    let win_up = up(&e, L_WIN);
    assert!(win_up.mask_modifier);
    assert_eq!(None, win_up.fired_id);
}

#[test]
fn holding_the_key_fires_once_and_keeps_it_from_windows() {
    let e = engine(&["Win+S"]);
    down(&e, L_WIN);
    down(&e, S);

    let repeat = down(&e, S);

    assert!(repeat.swallow);
    assert_eq!(None, repeat.fired_id);
}

#[test]
fn other_shortcuts_pass_through() {
    let e = engine(&["Win+S"]);
    down(&e, L_CTRL);

    assert_eq!(HookOutcome::PASS, down(&e, S));
    assert_eq!(HookOutcome::PASS, up(&e, S));
    assert_eq!(HookOutcome::PASS, up(&e, L_CTRL));
}

#[test]
fn alt_is_masked_after_an_override_so_the_menu_bar_stays_closed() {
    let e = engine(&["Alt+Tab"]);
    down(&e, L_ALT);
    down(&e, 0x09);
    up(&e, 0x09);

    assert!(up(&e, L_ALT).mask_modifier);
}

#[test]
fn missed_release_does_not_leave_a_modifier_stuck() {
    let e = engine(&["Win+S", "S"]);
    down(&e, L_WIN); // Win+L ile kilitlenince bırakılma olayı gelmez

    let key = down_physically(&e, S, &|_| false);

    assert_eq!(Some(ID + 1), key.fired_id);
}

#[test]
fn capture_swallows_keys_and_reports_the_combination() {
    let e = engine(&[]);
    e.begin_capture(0);

    assert!(down(&e, L_CTRL).swallow);
    let key = down(&e, X);

    assert!(key.swallow);
    assert_eq!(Some(parse("Ctrl+X")), key.captured);
}

#[test]
fn capture_reports_win_alone_when_it_is_released() {
    let e = engine(&[]);
    e.begin_capture(0);

    assert_eq!(None, down(&e, L_WIN).captured);
    let u = up(&e, L_WIN);

    assert!(u.swallow);
    assert_eq!(Some(parse("Win")), u.captured);
    assert!(!u.mask_modifier);
}

#[test]
fn capture_reports_a_bare_key_such_as_escape() {
    let e = engine(&[]);
    e.begin_capture(0);

    assert_eq!(Some(Hotkey::new(HotkeyModifiers::None, ESC)), down(&e, ESC).captured);
}

#[test]
fn capture_does_not_fire_existing_shortcuts() {
    let e = engine(&["Win"]);
    e.begin_capture(0);

    down(&e, L_WIN);

    assert_eq!(None, up(&e, L_WIN).fired_id);
}

#[test]
fn keys_of_a_captured_chord_stay_swallowed_after_capture_ends() {
    let e = engine(&[]);
    e.begin_capture(0);
    down(&e, L_CTRL);
    down(&e, X);

    e.end_capture();

    assert!(up(&e, X).swallow);
    assert!(up(&e, L_CTRL).swallow);
    assert_eq!(HookOutcome::PASS, down(&e, X));
}

#[test]
fn abandoned_capture_stops_swallowing_the_keyboard() {
    let e = engine(&[]);
    e.begin_capture(0);

    assert_eq!(HookOutcome::PASS, down_at(&e, X, 15_001));
}

#[test]
fn right_side_modifiers_count_too() {
    let e = engine(&["Win"]);

    down(&e, 0x5C);

    assert_eq!(Some(ID), up(&e, 0x5C).fired_id);
}

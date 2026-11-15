//! `Tray/TrayIconVisibilityPolicyTests.cs`. `ValueDictionary<T>` (Settings modülü) henüz taşınmadığı için
//! sözlük yerine `HashMap<String, TrayIconVisibility>` kullanılır; davranış aynı.

use std::collections::HashMap;

use crate::tray::{TrayIconVisibility, TrayIconVisibilityPolicy};

#[test]
fn new_icons_default_to_overflow() {
    assert_eq!(TrayIconVisibility::Overflow, TrayIconVisibilityPolicy::DEFAULT_NEW_ICON);
}

#[test]
fn unknown_app_gets_the_new_icon_default() {
    let per_app = HashMap::new();

    assert_eq!(TrayIconVisibility::Hidden, TrayIconVisibilityPolicy::resolve(Some("unknown.exe"), &per_app, TrayIconVisibility::Hidden));
    assert_eq!(
        TrayIconVisibility::Overflow,
        TrayIconVisibilityPolicy::resolve(Some("unknown.exe"), &per_app, TrayIconVisibilityPolicy::DEFAULT_NEW_ICON)
    );
}

#[test]
fn per_app_record_wins_over_the_default() {
    let per_app = HashMap::from([("discord.exe".to_owned(), TrayIconVisibility::Shown)]);

    assert_eq!(TrayIconVisibility::Shown, TrayIconVisibilityPolicy::resolve(Some("discord.exe"), &per_app, TrayIconVisibility::Hidden));
}

#[test]
fn app_key_ignores_case_and_surrounding_spaces() {
    let per_app = HashMap::from([(TrayIconVisibilityPolicy::app_key(Some("  Discord.EXE ")), TrayIconVisibility::Hidden)]);

    assert_eq!("discord.exe", TrayIconVisibilityPolicy::app_key(Some("  Discord.EXE ")));
    assert_eq!(TrayIconVisibility::Hidden, TrayIconVisibilityPolicy::resolve(Some("DISCORD.exe"), &per_app, TrayIconVisibility::Shown));
}

#[test]
fn missing_exe_name_falls_back_to_the_default() {
    for exe in [None, Some(""), Some("   ")] {
        let per_app = HashMap::from([(String::new(), TrayIconVisibility::Hidden)]);

        assert_eq!(TrayIconVisibility::Shown, TrayIconVisibilityPolicy::resolve(exe, &per_app, TrayIconVisibility::Shown), "{exe:?}");
    }
}

#[test]
fn settings_list_merges_seen_and_saved_apps_as_sorted_keys() {
    let apps = TrayIconVisibilityPolicy::settings_apps(
        [Some("Steam.exe"), None, Some(" "), Some("discord.exe")],
        ["DISCORD.EXE", "onedrive.exe"],
    );

    assert_eq!(vec!["discord.exe", "onedrive.exe", "steam.exe"], apps);
}

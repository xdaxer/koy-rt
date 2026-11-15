//! Karşılığı: `tests/Core.Tests/JumpList/ShortcutAppIdMatcherTests.cs`.

use crate::jump_list::{ExeAppIdCache, ShortcutAppIdMatcher};

const DISCORD_EXE: &str = r"C:\Users\u\AppData\Local\Discord\app-1.0.9267\Discord.exe";
const UPDATER: &str = r"C:\Users\u\AppData\Local\Discord\Update.exe";

#[test]
fn exact_ignores_case_and_quotes() {
    assert!(ShortcutAppIdMatcher::matches_exact(r"C:\Apps\Foo\foo.exe", Some("\"c:\\apps\\FOO\\Foo.exe\"")));
    assert!(!ShortcutAppIdMatcher::matches_exact(r"C:\Apps\Foo\foo.exe", Some(r"C:\Apps\Foo\bar.exe")));
    assert!(!ShortcutAppIdMatcher::matches_exact(r"C:\Apps\Foo\foo.exe", None));
    assert!(!ShortcutAppIdMatcher::matches_exact(r"C:\Apps\Foo\foo.exe", Some("")));
}

#[test]
fn squirrel_matches_across_versions() {
    assert!(ShortcutAppIdMatcher::matches_squirrel(DISCORD_EXE, Some(UPDATER), Some("--processStart Discord.exe")));
    assert!(ShortcutAppIdMatcher::matches_squirrel(
        r"C:\Users\u\AppData\Local\Discord\app-2.0.1\Discord.exe",
        Some(UPDATER),
        Some("--processStart \"discord.exe\"")
    ));
    assert!(ShortcutAppIdMatcher::matches_squirrel(DISCORD_EXE, Some(UPDATER), Some("--processStartAndWait Discord.exe")));
}

#[test]
fn squirrel_rejects_other_root_exe_or_missing_flag() {
    assert!(!ShortcutAppIdMatcher::matches_squirrel(DISCORD_EXE, Some(r"C:\Other\Update.exe"), Some("--processStart Discord.exe")));
    assert!(!ShortcutAppIdMatcher::matches_squirrel(DISCORD_EXE, Some(UPDATER), Some("--processStart Other.exe")));
    assert!(!ShortcutAppIdMatcher::matches_squirrel(DISCORD_EXE, Some(UPDATER), Some("Discord.exe")));
    assert!(!ShortcutAppIdMatcher::matches_squirrel(DISCORD_EXE, Some(UPDATER), None));
    assert!(!ShortcutAppIdMatcher::matches_squirrel(
        DISCORD_EXE,
        Some(r"C:\Users\u\AppData\Local\Discord\app-1.0.9267\Update.exe"),
        Some("--processStart Discord.exe")
    ));
}

#[test]
fn squirrel_accepts_root_stub_launcher() {
    // Discord'un gerçek kısayolu: hedef kökteki Discord.exe, bağımsız değişken --processStart Discord.exe.
    assert!(ShortcutAppIdMatcher::matches_squirrel(
        DISCORD_EXE,
        Some(r"C:\Users\u\AppData\Local\Discord\Discord.exe"),
        Some("--processStart Discord.exe")
    ));
}

#[test]
fn squirrel_requires_app_folder_layout() {
    assert!(!ShortcutAppIdMatcher::matches_squirrel(
        r"C:\Users\u\AppData\Local\Discord\bin\Discord.exe",
        Some(UPDATER),
        Some("--processStart Discord.exe")
    ));
    assert!(!ShortcutAppIdMatcher::matches_squirrel(
        r"C:\Users\u\AppData\Local\Discord\Discord.exe",
        Some(UPDATER),
        Some("--processStart Discord.exe")
    ));
}

#[test]
fn exe_app_id_cache_keeps_negative_results_and_is_bounded() {
    let cache = ExeAppIdCache::new(2);
    cache.set(r"C:\a.exe", None);
    cache.set(r"C:\b.exe", Some("B".to_string()));
    let a = cache.try_get(r"C:\A.EXE");
    assert_eq!(a, Some(None));
    cache.set(r"C:\c.exe", Some("C".to_string()));
    assert_eq!(cache.count(), 2);
    assert_eq!(cache.try_get(r"C:\a.exe"), None);
    assert_eq!(cache.try_get(r"C:\b.exe"), Some(Some("B".to_string())));
    cache.clear();
    assert_eq!(cache.count(), 0);
}

//! Karşılığı: `tests/Core.Tests/JumpList/JumpListEntryTests.cs` (JumpListEntryFactoryTests, JumpListLaunchPlanTests,
//! JumpListCacheTests).

use crate::jump_list::{
    FileStamp, JumpListCache, JumpListEntryFactory, JumpListLaunch, JumpListLaunchPlan, JumpListLimits, JumpListSection,
    JumpListSectionKind, JumpListText,
};

// --- JumpListEntryFactoryTests ---

#[test]
fn link_with_title_and_target_becomes_a_task_entry() {
    let entry = JumpListEntryFactory::from_link(
        Some("Yeni Pencere"),
        Some(r"C:\app\app.exe"),
        Some("--new-window"),
        Some(r"C:\app"),
        false,
    )
    .unwrap();

    assert_eq!(entry.title, "Yeni Pencere");
    let launch = entry.launch.unwrap();
    assert_eq!(launch.target, r"C:\app\app.exe");
    assert_eq!(launch.arguments.as_deref(), Some("--new-window"));
    assert_eq!(launch.working_directory.as_deref(), Some(r"C:\app"));
    assert!(!launch.is_document);
}

#[test]
fn empty_title_and_target_is_a_separator() {
    for (title, target) in [(None, None), (Some(""), Some("")), (Some("  "), Some(" "))] {
        let entry = JumpListEntryFactory::from_link(title, target, None, None, false).unwrap();

        assert!(entry.is_separator(), "{title:?} {target:?}");
    }
}

#[test]
fn marked_separator_wins_even_with_text() {
    assert!(JumpListEntryFactory::from_link(Some("x"), Some(r"C:\a.exe"), None, None, true).unwrap().is_separator());
}

#[test]
fn empty_title_with_a_target_is_skipped_not_invented() {
    // Spotify gibi: PKEY_Title boş ama hedef var. Başlık uydurulmaz.
    assert!(JumpListEntryFactory::from_link(Some(""), Some(r"C:\Users\x\spotify.exe"), Some("--uri=spotify:"), None, false).is_none());
    assert!(JumpListEntryFactory::from_link(Some("   "), Some(r"C:\Users\x\spotify.exe"), None, None, false).is_none());
}

#[test]
fn title_without_target_is_skipped() {
    assert!(JumpListEntryFactory::from_link(Some("Başlık"), Some(""), None, None, false).is_none());
}

#[test]
fn oversized_target_or_arguments_are_skipped() {
    let long_target = "x".repeat(JumpListLimits::MAX_TARGET_CHARS as usize + 1);
    let long_args = "x".repeat(JumpListLimits::MAX_ARGUMENTS_CHARS as usize + 1);
    assert!(JumpListEntryFactory::from_link(Some("a"), Some(&long_target), None, None, false).is_none());
    assert!(JumpListEntryFactory::from_link(Some("a"), Some(r"C:\a.exe"), Some(&long_args), None, false).is_none());
}

#[test]
fn blank_arguments_and_directory_become_null() {
    let entry = JumpListEntryFactory::from_link(Some("a"), Some(r"C:\a.exe"), Some("  "), Some(""), false).unwrap();
    let launch = entry.launch.unwrap();

    assert_eq!(launch.arguments, None);
    assert_eq!(launch.working_directory, None);
}

#[test]
fn document_entry_is_marked_as_document() {
    let entry = JumpListEntryFactory::from_document(Some("proje"), Some(r"C:\Dev\proje")).unwrap();

    assert!(entry.launch.unwrap().is_document);
    assert!(JumpListEntryFactory::from_document(Some(""), Some(r"C:\Dev\proje")).is_none());
    assert!(JumpListEntryFactory::from_document(Some("x"), None).is_none());
}

#[test]
fn title_is_sanitised_and_truncated() {
    let entry = JumpListEntryFactory::from_link(Some("  Yeni\r\n  pencere\u{0}  "), Some(r"C:\a.exe"), None, None, false).unwrap();
    assert_eq!(entry.title, "Yeni pencere");

    let long = JumpListEntryFactory::from_link(Some(&"a".repeat(500)), Some(r"C:\a.exe"), None, None, false).unwrap();
    assert_eq!(long.title.encode_utf16().count(), JumpListLimits::MAX_TITLE_CHARS as usize);
    assert!(long.title.ends_with('…'));
}

#[test]
fn truncation_does_not_split_a_surrogate_pair() {
    let title = format!("{}😀😀😀", "a".repeat(JumpListLimits::MAX_TITLE_CHARS as usize - 2));

    let entry = JumpListEntryFactory::from_link(Some(&title), Some(r"C:\a.exe"), None, None, false).unwrap();

    // Rust dizesi geçerli UTF-8'dir; kesme bir vekil çiftini bölseydi sonda U+FFFD kalırdı.
    assert!(!entry.title.contains('\u{FFFD}'));
    assert!(entry.title.encode_utf16().count() <= JumpListLimits::MAX_TITLE_CHARS as usize);
}

#[test]
fn indirect_strings_are_detected() {
    for (text, expected) in [
        (Some("@%SystemRoot%\\system32\\shell32.dll,-22067"), true),
        (Some("@{Pkg?ms-resource://x}"), true),
        (Some("@"), false),
        (Some("Normal"), false),
        (None, false),
    ] {
        assert_eq!(expected, JumpListText::is_indirect(text), "{text:?}");
    }
}

// --- JumpListLaunchPlanTests ---

const CODE: &str = r"C:\Apps\Code\Code.exe";

fn doc(target: &str) -> JumpListLaunch {
    JumpListLaunch { target: target.to_string(), is_document: true, ..Default::default() }
}

#[test]
fn task_runs_as_is() {
    let launch = JumpListLaunch {
        target: r"C:\a\a.exe".to_string(),
        arguments: Some("--x".to_string()),
        working_directory: Some(r"C:\a".to_string()),
        is_document: false,
    };
    let plan = JumpListLaunchPlan::for_launch(&launch, Some(CODE));

    assert_eq!(plan.file, r"C:\a\a.exe");
    assert_eq!(plan.arguments.as_deref(), Some("--x"));
    assert_eq!(plan.working_directory.as_deref(), Some(r"C:\a"));
}

#[test]
fn document_opens_with_the_app_exe() {
    let plan = JumpListLaunchPlan::for_launch(&doc(r"C:\Dev\proje"), Some(CODE));

    assert_eq!(plan.file, CODE);
    assert_eq!(plan.arguments.as_deref(), Some("\"C:\\Dev\\proje\""));
}

#[test]
fn document_path_ending_in_backslash_keeps_its_closing_quote() {
    let plan = JumpListLaunchPlan::for_launch(&doc(r"D:\"), Some(CODE));

    assert_eq!(plan.arguments.as_deref(), Some("\"D:\\\\\""));
}

#[test]
fn shortcuts_programs_and_shell_names_use_the_default_association() {
    for target in [
        r"C:\Oyunlar\Oyun.url",
        r"C:\Kisayol\a.lnk",
        r"C:\Araclar\arac.exe",
        "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}",
        "shell:AppsFolder\\Pkg!App",
    ] {
        let plan = JumpListLaunchPlan::for_launch(&doc(target), Some(CODE));

        assert_eq!(plan.file, target);
        assert_eq!(plan.arguments, None, "{target}");
    }
}

#[test]
fn unknown_or_hosted_app_exe_uses_the_default_association() {
    for exe in [
        None,
        Some(""),
        Some(r"C:\Apps\run.bat"),
        Some(r"C:\Program Files\WindowsApps\Pkg\app.exe"),
        Some(r"C:\Windows\System32\ApplicationFrameHost.exe"),
    ] {
        let plan = JumpListLaunchPlan::for_launch(&doc(r"C:\Dev\proje"), exe);

        assert_eq!(plan.file, r"C:\Dev\proje", "{exe:?}");
    }
}

#[test]
fn url_documents_open_with_the_app_exe() {
    let plan = JumpListLaunchPlan::for_launch(&doc("https://example.com/a"), Some(r"C:\Apps\chrome.exe"));

    assert_eq!(plan.file, r"C:\Apps\chrome.exe");
    assert_eq!(plan.arguments.as_deref(), Some("\"https://example.com/a\""));
}

#[test]
fn quote_in_target_uses_the_default_association() {
    let plan = JumpListLaunchPlan::for_launch(&doc("a\" --evil"), Some(CODE));

    assert_eq!(plan.file, "a\" --evil");
}

// --- JumpListCacheTests ---

const A: FileStamp = FileStamp { length: 10, last_write_utc_ticks: 100 };
const B: FileStamp = FileStamp { length: 20, last_write_utc_ticks: 200 };

fn sections(name: &str) -> Vec<JumpListSection> {
    vec![JumpListSection::new(JumpListSectionKind::Custom, Some(name.to_string()), vec![])]
}

#[test]
fn hit_requires_equal_stamps() {
    let cache = JumpListCache::default();
    cache.set("app", A, B, sections("1"));

    let hit = cache.try_get("APP", A, B).unwrap();
    assert_eq!(hit[0].name.as_deref(), Some("1"));
    assert!(cache.try_get("app", A, A).is_none());
}

#[test]
fn stale_entry_is_removed_on_miss() {
    let cache = JumpListCache::default();
    cache.set("app", A, B, sections("1"));

    assert!(cache.try_get("app", B, B).is_none());
    assert_eq!(cache.count(), 0);
}

#[test]
fn capacity_is_bounded_and_least_recently_used_goes_first() {
    let cache = JumpListCache::new(3);
    cache.set("a", A, A, sections("a"));
    cache.set("b", A, A, sections("b"));
    cache.set("c", A, A, sections("c"));
    assert!(cache.try_get("a", A, A).is_some()); // a yeniden kullanıldı
    cache.set("d", A, A, sections("d")); // b atılmalı

    assert_eq!(cache.count(), 3);
    assert!(cache.try_get("b", A, A).is_none());
    assert!(cache.try_get("a", A, A).is_some());
    assert!(cache.try_get("c", A, A).is_some());
    assert!(cache.try_get("d", A, A).is_some());
}

#[test]
fn set_replaces_existing_app_and_clear_empties() {
    let cache = JumpListCache::default();
    cache.set("app", A, A, sections("old"));
    cache.set("app", B, B, sections("new"));

    assert_eq!(cache.count(), 1);
    let hit = cache.try_get("app", B, B).unwrap();
    assert_eq!(hit[0].name.as_deref(), Some("new"));

    cache.clear();
    assert_eq!(cache.count(), 0);
}

#[test]
fn missing_stamp_is_distinct_from_real_files() {
    assert_ne!(FileStamp::missing(), FileStamp::new(0, 0));
}

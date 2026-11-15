//! Karşılığı: `tests/Core.Tests/JumpList/JumpListLayoutTests.cs`.

use super::custom_destinations_builder::{link, CustomDestinationsBuilder};
use crate::jump_list::{
    CustomDestinationsFile, CustomDestinationsReader, JumpListEntry, JumpListLaunch, JumpListLayout, JumpListLimits,
    JumpListSection, JumpListSectionKind,
};

fn e(title: &str) -> JumpListEntry {
    JumpListEntry::new(title, Some(JumpListLaunch::new(format!(r"C:\app\{title}.exe"))))
}

fn section(kind: JumpListSectionKind, name: Option<&str>, entries: Vec<JumpListEntry>) -> JumpListSection {
    JumpListSection::new(kind, name.map(str::to_string), entries)
}

fn many(kind: JumpListSectionKind, name: Option<&str>, count: usize) -> JumpListSection {
    section(kind, name, (1..=count).map(|i| e(&format!("{kind:?}{i}"))).collect())
}

fn count(sections: &[JumpListSection], kind: JumpListSectionKind) -> Vec<&JumpListSection> {
    sections.iter().filter(|s| s.kind == kind).collect()
}

fn single(sections: &[JumpListSection], kind: JumpListSectionKind) -> &JumpListSection {
    let found = count(sections, kind);
    assert_eq!(found.len(), 1);
    found[0]
}

#[test]
fn sections_are_ordered_custom_frequent_recent_tasks() {
    let result = JumpListLayout::arrange(&[
        section(JumpListSectionKind::Tasks, None, vec![e("t")]),
        section(JumpListSectionKind::Recent, None, vec![e("r")]),
        section(JumpListSectionKind::Frequent, None, vec![e("f")]),
        section(JumpListSectionKind::Custom, Some("Kategori A"), vec![e("a")]),
        section(JumpListSectionKind::Custom, Some("Kategori B"), vec![e("b")]),
    ]);

    assert_eq!(
        vec![
            JumpListSectionKind::Custom,
            JumpListSectionKind::Custom,
            JumpListSectionKind::Frequent,
            JumpListSectionKind::Recent,
            JumpListSectionKind::Tasks
        ],
        result.iter().map(|s| s.kind).collect::<Vec<_>>()
    );
    assert_eq!(
        vec![Some("Kategori A".to_string()), Some("Kategori B".to_string())],
        result.iter().filter(|s| s.kind == JumpListSectionKind::Custom).map(|s| s.name.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn empty_sections_and_nameless_categories_are_dropped() {
    let result = JumpListLayout::arrange(&[
        section(JumpListSectionKind::Recent, None, vec![]),
        section(JumpListSectionKind::Custom, Some("  "), vec![e("a")]),
        section(JumpListSectionKind::Custom, None, vec![e("a")]),
        section(JumpListSectionKind::Tasks, None, vec![JumpListEntry::separator()]),
        section(JumpListSectionKind::Tasks, None, vec![e("t")]),
    ]);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].kind, JumpListSectionKind::Tasks);
}

#[test]
fn separators_are_normalised() {
    let sep = JumpListEntry::separator;
    let result = JumpListLayout::arrange(&[section(
        JumpListSectionKind::Tasks,
        None,
        vec![sep(), e("a"), sep(), sep(), e("b"), sep()],
    )]);

    assert_eq!(result.len(), 1);
    let entries = &result[0].entries;
    assert_eq!(entries.len(), 3);
    assert!(!entries[0].is_separator());
    assert!(entries[1].is_separator());
    assert!(!entries[2].is_separator());
}

#[test]
fn per_section_caps_apply() {
    let result = JumpListLayout::arrange(&[
        many(JumpListSectionKind::Custom, Some("k"), 50),
        many(JumpListSectionKind::Tasks, None, 50),
    ]);

    assert!(single(&result, JumpListSectionKind::Custom).entries.len() <= JumpListLimits::MAX_PER_CUSTOM_SECTION as usize);
    assert!(single(&result, JumpListSectionKind::Tasks).entries.len() <= JumpListLimits::MAX_TASKS as usize);
}

#[test]
fn individual_caps_are_exact_when_total_is_not_exceeded() {
    let result = JumpListLayout::arrange(&[
        many(JumpListSectionKind::Recent, None, 20),
        many(JumpListSectionKind::Tasks, None, 20),
    ]);

    assert_eq!(single(&result, JumpListSectionKind::Recent).entries.len(), JumpListLimits::MAX_RECENT as usize);
    assert_eq!(single(&result, JumpListSectionKind::Tasks).entries.len(), JumpListLimits::MAX_TASKS as usize);
}

#[test]
fn custom_section_count_is_capped_in_file_order() {
    let sections: Vec<JumpListSection> =
        (1..=8).map(|i| section(JumpListSectionKind::Custom, Some(&format!("K{i}")), vec![e(&format!("x{i}"))])).collect();

    let result = JumpListLayout::arrange(&sections);

    assert_eq!(result.len(), JumpListLimits::MAX_CUSTOM_SECTIONS as usize);
    assert_eq!(
        vec![Some("K1".to_string()), Some("K2".to_string()), Some("K3".to_string())],
        result.iter().map(|s| s.name.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn total_cap_trims_frequent_then_recent_and_keeps_tasks() {
    let result = JumpListLayout::arrange(&[
        many(JumpListSectionKind::Custom, Some("A"), 6),
        many(JumpListSectionKind::Custom, Some("B"), 6),
        many(JumpListSectionKind::Custom, Some("C"), 6),
        many(JumpListSectionKind::Frequent, None, 3),
        many(JumpListSectionKind::Recent, None, 5),
        many(JumpListSectionKind::Tasks, None, 6),
    ]);

    let total: i32 = result.iter().map(|s| JumpListLayout::count_entries(&s.entries)).sum();
    assert!(total <= JumpListLimits::MAX_TOTAL_ENTRIES, "toplam {total}");

    // Görevler (en sonda kırpılır) sınırından fazla küçülmemeli; her bölüm en az MinPerSection girdi korur.
    for s in &result {
        assert!(JumpListLayout::count_entries(&s.entries) >= JumpListLimits::MIN_PER_SECTION_WHEN_TRIMMING);
    }
    let tasks = single(&result, JumpListSectionKind::Tasks);
    let recent = single(&result, JumpListSectionKind::Recent);
    assert!(JumpListLayout::count_entries(&tasks.entries) >= JumpListLayout::count_entries(&recent.entries));
}

#[test]
fn total_cap_never_goes_below_minimum_per_section() {
    let mut all: Vec<JumpListSection> = Vec::new();
    for i in 0..20 {
        all.push(many(JumpListSectionKind::Custom, Some(&format!("K{i}")), 6));
    }
    all.push(many(JumpListSectionKind::Frequent, None, 9));
    all.push(many(JumpListSectionKind::Recent, None, 9));
    all.push(many(JumpListSectionKind::Tasks, None, 9));

    let result = JumpListLayout::arrange(&all);

    assert!(result.iter().map(|s| JumpListLayout::count_entries(&s.entries)).sum::<i32>() <= JumpListLimits::MAX_TOTAL_ENTRIES);
    assert_eq!(result.len(), 6);
}

#[test]
fn trimming_does_not_leave_trailing_separators() {
    let sep = JumpListEntry::separator();
    let result = JumpListLayout::arrange(&[section(
        JumpListSectionKind::Tasks,
        None,
        vec![e("1"), e("2"), e("3"), e("4"), e("5"), e("6"), sep, e("7")],
    )]);

    assert_eq!(result.len(), 1);
    assert!(!result[0].entries.last().unwrap().is_separator());
}

#[test]
fn titles_in_category_name_are_sanitised() {
    let result = JumpListLayout::arrange(&[section(
        JumpListSectionKind::Custom,
        Some("Son\r\nkullanılan\t  klasörler"),
        vec![e("a")],
    )]);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name.as_deref(), Some("Son kullanılan klasörler"));
}

// --- Son / sık kullanılanlar hangi durumda gösterilir ---

#[test]
fn recent_is_shown_when_the_app_has_no_usable_list() {
    assert!(JumpListLayout::shows_recent(None));
    assert!(JumpListLayout::shows_recent(Some(&CustomDestinationsFile::empty())));
    assert!(!JumpListLayout::shows_frequent(None));
    assert!(!JumpListLayout::shows_frequent(Some(&CustomDestinationsFile::empty())));
}

#[test]
fn valid_file_with_only_an_unrecognised_known_category_does_not_default_to_recent() {
    let file = CustomDestinationsReader::parse(&CustomDestinationsBuilder::new().known(-1).build());

    assert!(file.is_valid);
    assert!(file.is_empty());
    assert!(!JumpListLayout::shows_recent(Some(&file)));
    assert!(!JumpListLayout::shows_frequent(Some(&file)));
}

#[test]
fn known_categories_follow_what_the_app_declared() {
    let tasks_only = CustomDestinationsReader::parse(&CustomDestinationsBuilder::new().tasks(&[link()]).build());
    assert!(!JumpListLayout::shows_recent(Some(&tasks_only)));
    assert!(!JumpListLayout::shows_frequent(Some(&tasks_only)));

    let recent = CustomDestinationsReader::parse(&CustomDestinationsBuilder::new().tasks(&[link()]).known(2).build());
    assert!(JumpListLayout::shows_recent(Some(&recent)));
    assert!(!JumpListLayout::shows_frequent(Some(&recent)));

    let frequent = CustomDestinationsReader::parse(&CustomDestinationsBuilder::new().known(1).build());
    assert!(!JumpListLayout::shows_recent(Some(&frequent)));
    assert!(JumpListLayout::shows_frequent(Some(&frequent)));
}

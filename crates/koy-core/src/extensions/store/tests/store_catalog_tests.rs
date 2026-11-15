//! Karşılığı: `Extensions/Store/StoreCatalogTests.cs`.

use std::collections::{HashMap, HashSet};

use super::store_test_kit::*;
use crate::extensions::store::*;

fn state(items: Vec<StoreItem>, registry: Option<InstalledRegistry>, query: Option<StoreQuery>, busy: &[&str]) -> StoreListState {
    StoreListState::new(
        Some(StoreIndex::new(None, items)),
        registry.unwrap_or_else(InstalledRegistry::empty),
        busy.iter().map(|b| b.to_string()).collect::<HashSet<_>>(),
        query.unwrap_or_default(),
    )
}

fn ids(state: &StoreListState) -> Vec<String> {
    state.visible().into_iter().map(|r| r.id).collect()
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

fn items_of(specs: &[(&str, ExtensionKind, &str)]) -> Vec<StoreItem> {
    specs.iter().map(|(id, kind, version)| item(id, *kind, version)).collect()
}

fn theme(id: &str, version: &str) -> StoreItem {
    item(id, ExtensionKind::Theme, version)
}

fn inst(id: &str, kind: ExtensionKind, version: &str) -> InstalledEntry {
    installed(id, kind, version, false, "")
}

fn query(f: impl FnOnce(&mut StoreQuery)) -> StoreQuery {
    let mut q = StoreQuery::default();
    f(&mut q);
    q
}

fn search(text: &str) -> StoreQuery {
    query(|q| q.search = Some(text.to_string()))
}

#[test]
fn status_not_installed_installed_update_and_installing() {
    let s = state(
        vec![theme("aa", "1.0.0"), theme("bb", "2.0.0"), theme("cc", "1.0.0"), theme("dd", "1.0.0")],
        Some(registry(vec![
            inst("bb", ExtensionKind::Theme, "2.0.0"),
            inst("cc", ExtensionKind::Theme, "0.9.0"),
            inst("dd", ExtensionKind::Theme, "1.0.0"),
        ])),
        None,
        &["dd"],
    );

    let rows: HashMap<String, StoreEntryView> = s.visible().into_iter().map(|r| (r.id.clone(), r)).collect();

    assert_eq!(StoreItemStatus::NotInstalled, rows["aa"].status);
    assert_eq!(StoreItemStatus::Installed, rows["bb"].status);
    assert_eq!(StoreItemStatus::UpdateAvailable, rows["cc"].status);
    assert_eq!(StoreItemStatus::Installing, rows["dd"].status);
    assert_eq!(Some("0.9.0"), rows["cc"].installed_version.as_deref());
    assert_eq!(Some("1.0.0"), rows["cc"].available_version.as_deref());
    assert_eq!(1, s.update_count());
}

#[test]
fn older_index_version_is_not_an_update() {
    let s = state(vec![theme("aa", "1.0.0")], Some(registry(vec![inst("aa", ExtensionKind::Theme, "2.0.0")])), None, &[]);

    assert_eq!(StoreItemStatus::Installed, s.visible()[0].status);
}

#[test]
fn same_id_with_other_kind_is_not_an_update() {
    let s = state(
        vec![item("aa", ExtensionKind::Profile, "2.0.0")],
        Some(registry(vec![inst("aa", ExtensionKind::Theme, "1.0.0")])),
        None,
        &[],
    );

    assert!(!s.visible()[0].has_update);
}

#[test]
fn installing_an_installed_update_keeps_it_in_installed_and_updates_filters() {
    let items = vec![theme("aa", "2.0.0")];
    let reg = registry(vec![inst("aa", ExtensionKind::Theme, "1.0.0")]);

    assert_eq!(
        vec!["aa".to_string()],
        ids(&state(items.clone(), Some(reg.clone()), Some(query(|q| q.filter = StoreFilter::Updates)), &["aa"]))
    );
    assert_eq!(StoreItemStatus::Installing, state(items, Some(reg), None, &["aa"]).visible()[0].status);
}

#[test]
fn filters_all_installed_updates() {
    let items = vec![theme("aa", "1.0.0"), theme("bb", "2.0.0"), theme("cc", "1.0.0")];
    let reg = registry(vec![inst("bb", ExtensionKind::Theme, "1.0.0"), inst("cc", ExtensionKind::Theme, "1.0.0")]);

    let with = |filter: StoreFilter| state(items.clone(), Some(reg.clone()), Some(query(|q| q.filter = filter)), &[]);

    assert_eq!(vec!["aa", "bb", "cc"], sorted(ids(&with(StoreFilter::All))));
    assert_eq!(vec!["bb", "cc"], sorted(ids(&with(StoreFilter::Installed))));
    assert_eq!(vec!["bb"], ids(&with(StoreFilter::Updates)));
}

#[test]
fn file_installed_items_not_in_index_show_under_installed_only() {
    let items = vec![theme("aa", "1.0.0")];
    let reg = registry(vec![InstalledEntry { source: ExtensionSource::File, ..inst("local-one", ExtensionKind::Profile, "1.0.0") }]);

    let with = |filter: StoreFilter| state(items.clone(), Some(reg.clone()), Some(query(|q| q.filter = filter)), &[]).visible();
    let all = with(StoreFilter::All);
    let installed = with(StoreFilter::Installed);
    let updates = with(StoreFilter::Updates);

    assert!(all.iter().any(|r| r.id == "local-one" && r.item.is_none() && r.installed.is_some()));
    assert_eq!(vec!["local-one"], installed.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
    assert!(updates.is_empty());
}

#[test]
fn everything_is_visible_without_an_index_when_installed() {
    let s = StoreListState::new(None, registry(vec![default_installed_as("aa")]), HashSet::new(), StoreQuery::default());

    assert_eq!(vec!["aa"], ids(&s));
    assert!(ids(&StoreListState { registry: InstalledRegistry::empty(), ..s }).is_empty());
}

fn default_installed_as(id: &str) -> InstalledEntry {
    inst(id, ExtensionKind::Theme, "1.0.0")
}

#[test]
fn category_filters_by_kind_and_null_means_all() {
    let items = items_of(&[("aa", ExtensionKind::Theme, "1.0.0"), ("bb", ExtensionKind::IconPack, "1.0.0"), ("cc", ExtensionKind::Profile, "1.0.0")]);

    assert_eq!(vec!["bb"], ids(&state(items.clone(), None, Some(query(|q| q.category = Some(ExtensionKind::IconPack))), &[])));
    assert_eq!(3, ids(&state(items, None, Some(query(|q| q.category = None)), &[])).len());
}

#[test]
fn category_and_filter_combine() {
    let items = items_of(&[("aa", ExtensionKind::Theme, "1.0.0"), ("bb", ExtensionKind::IconPack, "1.0.0")]);
    let reg = registry(vec![inst("aa", ExtensionKind::Theme, "1.0.0")]);

    assert!(ids(&state(
        items.clone(),
        Some(reg.clone()),
        Some(query(|q| {
            q.filter = StoreFilter::Installed;
            q.category = Some(ExtensionKind::IconPack);
        })),
        &[]
    ))
    .is_empty());
    assert_eq!(
        vec!["aa"],
        ids(&state(
            items,
            Some(reg),
            Some(query(|q| {
                q.filter = StoreFilter::Installed;
                q.category = Some(ExtensionKind::Theme);
            })),
            &[]
        ))
    );
}

#[test]
fn search_is_case_and_turkish_letter_insensitive() {
    let rows = [
        ("nord", "nord"),
        ("NORD", "nord"),
        ("ISIK", "isik"),
        ("ışık", "isik"),
        ("IŞIK", "isik"),
        ("İŞİK", "isik"),
        ("işık", "isik"),
        ("sifre", "sifre"),
        ("ŞİFRE", "sifre"),
    ];
    let names = [("nord", "Nord"), ("isik", "Işık Teması"), ("sifre", "Şifre"), ("other", "Baska")];
    let items: Vec<StoreItem> = names
        .iter()
        .map(|(key, name)| {
            let mut i = theme(&format!("koyhq.{key}"), "1.0.0");
            i.name = name.to_string();
            i
        })
        .collect();

    for (q, target) in rows {
        assert_eq!(vec![format!("koyhq.{target}")], ids(&state(items.clone(), None, Some(search(q)), &[])), "{q}");
    }
}

fn named(id: &str, name: &str, summary: &str, author: &str) -> StoreItem {
    let mut i = theme(id, "1.0.0");
    i.name = name.to_string();
    i.summary = LocalizedText::plain(summary);
    i.author = author.to_string();
    i
}

#[test]
fn search_matches_summary_and_author_and_requires_every_word() {
    let items = vec![named("aa", "Alpha", "Koyu mavi tema", "Ayşe"), named("bb", "Beta", "Acik tema", "Mehmet")];
    let ids_of = |q: StoreQuery| ids(&state(items.clone(), None, Some(q), &[]));

    assert_eq!(vec!["aa"], ids_of(search("MAVI")));
    assert_eq!(vec!["bb"], ids_of(search("mehmet")));
    assert_eq!(vec!["aa"], ids_of(search("ayse koyu")));
    assert!(ids_of(search("ayse acik")).is_empty());
    assert_eq!(2, ids_of(search("  ")).len());
    assert_eq!(2, ids_of(query(|q| q.search = None)).len());
}

#[test]
fn search_does_not_match_id_or_description() {
    let mut i = theme("koyhq.secret", "1.0.0");
    i.name = "Alpha".to_string();

    assert!(ids(&state(vec![i], None, Some(search("secret")), &[])).is_empty());
}

fn dated(id: &str, name: &str, updated: &str) -> StoreItem {
    let mut i = theme(id, "1.0.0");
    i.name = name.to_string();
    i.updated = chrono::NaiveDate::parse_from_str(updated, "%Y-%m-%d").unwrap();
    i
}

#[test]
fn sort_newest_is_updated_descending_with_name_tiebreak() {
    let items = vec![dated("aa", "Zeta", "2026-01-01"), dated("bb", "Beta", "2026-03-01"), dated("cc", "Alpha", "2026-03-01")];

    assert_eq!(vec!["cc", "bb", "aa"], ids(&state(items, None, Some(query(|q| q.sort = StoreSort::Newest)), &[])));
}

#[test]
fn sort_alphabetical_uses_turkish_folding() {
    let mut items = Vec::new();
    for (id, name) in [("aa", "Zeta"), ("bb", "Çam"), ("cc", "Arı"), ("dd", "ısı")] {
        let mut i = theme(id, "1.0.0");
        i.name = name.to_string();
        items.push(i);
    }

    // Katlanmış: ari, cam, isi, zeta
    assert_eq!(vec!["cc", "bb", "dd", "aa"], ids(&state(items, None, Some(query(|q| q.sort = StoreSort::Alphabetical)), &[])));
}

#[test]
fn sort_most_downloads_is_descending() {
    let mut items = Vec::new();
    for (id, name, downloads) in [("aa", "A", 5), ("bb", "B", 50), ("cc", "C", 5)] {
        let mut i = theme(id, "1.0.0");
        i.name = name.to_string();
        i.downloads = downloads;
        items.push(i);
    }

    assert_eq!(vec!["bb", "aa", "cc"], ids(&state(items, None, Some(query(|q| q.sort = StoreSort::MostDownloads)), &[])));
}

#[test]
fn applied_flag_comes_from_the_registry() {
    let s = state(vec![theme("aa", "1.0.0")], Some(registry(vec![installed("aa", ExtensionKind::Theme, "1.0.0", true, "")])), None, &[]);

    assert!(s.visible()[0].applied);
}

#[test]
fn initial_state_is_empty() {
    assert!(StoreListState::initial().visible().is_empty());
    assert_eq!(0, StoreListState::initial().update_count());
}

//! Karşılığı: `Extensions/InstalledExtensionListTests.cs`.

use std::cell::Cell;

use crate::extensions::*;

fn installed(id: &str, enabled: bool) -> InstalledExtension {
    let manifest = ExtensionManifestValidator::try_create(
        Some(id),
        Some(&format!("Ad {id}")),
        Some("1.0.0"),
        Some("Ada"),
        std::iter::empty(),
        Some("data"),
    )
    .expect("geçerli manifest");
    InstalledExtension::new(manifest, enabled)
}

fn ids(list: &[InstalledExtension]) -> Vec<&str> {
    list.iter().map(|e| e.manifest.id()).collect()
}

#[test]
fn build_orders_by_id_regardless_of_input_order() {
    let list = InstalledExtensionList::try_build([installed("zeta", true), installed("alpha", true), installed("mid", true)]).unwrap();

    assert_eq!(vec!["alpha", "mid", "zeta"], ids(&list));
}

#[test]
fn build_keeps_enabled_state() {
    let list = InstalledExtensionList::try_build([installed("beta", false), installed("alpha", true)]).unwrap();

    assert!(!list[1].enabled);
    assert!(list[0].enabled);
}

#[test]
fn build_accepts_empty_list() {
    let list = InstalledExtensionList::try_build(Vec::new()).unwrap();

    assert!(list.is_empty());
}

#[test]
fn build_rejects_duplicate_ids() {
    let result = InstalledExtensionList::try_build([installed("alpha", true), installed("alpha", false)]);

    assert_eq!(Err(InstalledExtensionListError::DuplicateId), result);
}

#[test]
fn build_accepts_exactly_the_maximum_count() {
    let entries: Vec<InstalledExtension> = (0..InstalledExtensionList::MAX_INSTALLED).map(|i| installed(&format!("ext-{i:02}"), true)).collect();

    let list = InstalledExtensionList::try_build(entries).unwrap();

    assert_eq!(InstalledExtensionList::MAX_INSTALLED, list.len());
}

#[test]
fn build_rejects_more_than_the_maximum_count() {
    let entries: Vec<InstalledExtension> = (0..InstalledExtensionList::MAX_INSTALLED + 1).map(|i| installed(&format!("ext-{i:02}"), true)).collect();

    assert_eq!(Err(InstalledExtensionListError::TooManyExtensions), InstalledExtensionList::try_build(entries));
}

#[test]
fn set_enabled_changes_only_the_named_extension() {
    let list = InstalledExtensionList::try_build([installed("alpha", true), installed("beta", true)]).unwrap();

    let result = InstalledExtensionList::try_set_enabled(&list, "beta", false).unwrap();

    assert!(result[0].enabled);
    assert!(!result[1].enabled);
    assert_eq!(vec!["alpha", "beta"], ids(&result));
}

#[test]
fn set_enabled_does_not_modify_the_input_list() {
    let list = InstalledExtensionList::try_build([installed("alpha", true)]).unwrap();

    let _ = InstalledExtensionList::try_set_enabled(&list, "alpha", false);

    assert!(list[0].enabled);
}

// C# `result` hatada girdi listenin kendisiydi (`Assert.Same`); Rust'ta `Err` döner ve girdi zaten değişmez.
#[test]
fn set_enabled_on_unknown_id_fails_and_leaves_the_list_untouched() {
    let list = InstalledExtensionList::try_build([installed("alpha", true)]).unwrap();
    let before = list.clone();

    let result = InstalledExtensionList::try_set_enabled(&list, "missing", false);

    assert_eq!(Err(InstalledExtensionListError::UnknownId), result);
    assert_eq!(before, list);
}

#[test]
fn set_enabled_match_is_case_sensitive() {
    let list = InstalledExtensionList::try_build([installed("alpha", true)]).unwrap();

    assert_eq!(Err(InstalledExtensionListError::UnknownId), InstalledExtensionList::try_set_enabled(&list, "Alpha", false));
}

#[test]
fn build_stops_reading_once_limit_is_exceeded() {
    // Rakamlar harfe çevrilir: kimlik kuralları ("ext-" + sayı) rakamı da kabul eder, ama C# testi aynı şekilde yazılmıştı.
    let pulled = Cell::new(0usize);
    let endless = std::iter::from_fn(|| {
        if pulled.get() < 10_000 {
            let n = pulled.get();
            pulled.set(n + 1);
            let letters: String = n.to_string().chars().map(|c| (b'a' + (c as u8 - b'0')) as char).collect();
            Some(installed(&format!("ext-{letters}"), true))
        } else {
            None
        }
    });

    let result = InstalledExtensionList::try_build(endless);

    assert_eq!(Err(InstalledExtensionListError::TooManyExtensions), result);
    assert_eq!(InstalledExtensionList::MAX_INSTALLED + 1, pulled.get());
}

// C# `Built_list_cannot_be_mutated_through_a_cast`: Rust'ta liste `Vec` döner ve sahibi çağırandır; salt okunur sarmalayıcı
// kavramı yoktur. Değişmezlik `&[T]` ödünç almasıyla sağlanır.

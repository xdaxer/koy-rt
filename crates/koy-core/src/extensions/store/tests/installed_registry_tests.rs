//! Karşılığı: `Extensions/Store/InstalledRegistryTests.cs`.

use std::sync::{Arc, Mutex};

use chrono::TimeZone;

use super::store_test_kit::*;
use crate::extensions::store::*;
use crate::extensions::InstalledExtensionList;

fn inst(id: &str) -> InstalledEntry {
    installed(id, ExtensionKind::Theme, "1.0.0", false, "")
}

fn inst_kind(id: &str, kind: ExtensionKind) -> InstalledEntry {
    installed(id, kind, "1.0.0", false, "")
}

fn inst_version(id: &str, version: &str) -> InstalledEntry {
    installed(id, ExtensionKind::Theme, version, false, "")
}

fn ids(registry: &InstalledRegistry) -> Vec<&str> {
    registry.entries().iter().map(|e| e.id.as_str()).collect()
}

fn store_at(path: std::path::PathBuf) -> InstalledRegistryStore {
    InstalledRegistryStore::new(Some(path), None)
}

/// Hata geri çağrısını toplayan kayıt deposu.
fn store_with_errors(path: std::path::PathBuf) -> (InstalledRegistryStore, Arc<Mutex<Vec<String>>>) {
    let errors = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&errors);
    let store = InstalledRegistryStore::new(
        Some(path),
        Some(Box::new(move |source: &str, _error: &RegistryError| sink.lock().unwrap().push(source.to_string()))),
    );
    (store, errors)
}

fn silent_store(path: std::path::PathBuf) -> InstalledRegistryStore {
    InstalledRegistryStore::new(Some(path), Some(Box::new(|_: &str, _: &RegistryError| {})))
}

fn upsert_aa(r: &InstalledRegistry) -> Option<InstalledRegistry> {
    r.try_upsert(inst("aa")).ok()
}

#[test]
fn build_orders_by_id_and_rejects_duplicates() {
    let registry = InstalledRegistry::try_build([inst("zeta"), inst("alpha")]).unwrap();
    assert_eq!(vec!["alpha", "zeta"], ids(&registry));

    assert_eq!(InstalledRegistryError::DuplicateId, InstalledRegistry::try_build([inst("aa"), inst("aa")]).unwrap_err());
}

#[test]
fn build_enforces_the_64_limit() {
    let ok: Vec<InstalledEntry> = (0..InstalledExtensionList::MAX_INSTALLED).map(|i| inst(&format!("ext-{i}"))).collect();
    let too_many: Vec<InstalledEntry> = (0..InstalledExtensionList::MAX_INSTALLED + 1).map(|i| inst(&format!("ext-{i}"))).collect();

    assert!(InstalledRegistry::try_build(ok).is_ok());
    assert_eq!(InstalledRegistryError::TooManyExtensions, InstalledRegistry::try_build(too_many).unwrap_err());
}

#[test]
fn build_rejects_invalid_entries() {
    assert_eq!(InstalledRegistryError::InvalidEntry, InstalledRegistry::try_build([inst("Bad Id")]).unwrap_err());
    assert!(InstalledRegistry::try_build([InstalledEntry { version: "1.0".to_string(), ..inst("ok") }]).is_err());
    // C#'taki `(ExtensionKind)99` satırı Rust'ta ifade edilemez: enum'a tanımsız değer atanamaz.
    assert!(InstalledRegistry::try_build([InstalledEntry { license: String::new(), ..inst("ok") }]).is_err());
}

#[test]
fn upsert_replaces_same_id_and_adds_new() {
    let registry = registry(vec![inst_version("aa", "1.0.0")]);

    let updated = registry.try_upsert(inst_version("aa", "1.1.0")).unwrap();
    assert_eq!(1, updated.entries().len());
    assert_eq!("1.1.0", updated.entries()[0].version);

    let two = updated.try_upsert(inst("bb")).unwrap();
    assert_eq!(2, two.entries().len());
    assert_eq!("1.0.0", registry.find(Some("aa")).unwrap().version);
}

#[test]
fn upsert_of_new_id_fails_at_limit_but_update_of_existing_does_not() {
    let full = registry((0..64).map(|i| inst(&format!("ext-{i}"))).collect());

    assert_eq!(InstalledRegistryError::TooManyExtensions, full.try_upsert(inst("new-one")).unwrap_err());
    assert!(full.try_upsert(inst_version("ext-3", "2.0.0")).is_ok());
}

#[test]
fn remove_and_find() {
    let registry = registry(vec![inst("aa"), inst("bb")]);

    assert!(registry.remove("aa").find(Some("aa")).is_none());
    assert!(registry.remove("aa").find(Some("bb")).is_some());
    // C# `Assert.Same`: Rust'ta değer eşitliği.
    assert_eq!(registry.find(Some("aa")), registry.find(Some("aa")));
    assert!(registry.find(None).is_none());
}

#[test]
fn with_applied_is_exclusive_within_a_kind_only() {
    let registry = registry(vec![
        installed("theme-a", ExtensionKind::Theme, "1.0.0", true, ""),
        inst_kind("theme-b", ExtensionKind::Theme),
        installed("icons-a", ExtensionKind::IconPack, "1.0.0", true, ""),
    ]);

    let next = registry.with_applied("theme-b");

    assert!(!next.find(Some("theme-a")).unwrap().applied);
    assert!(next.find(Some("theme-b")).unwrap().applied);
    assert!(next.find(Some("icons-a")).unwrap().applied);
    assert!(!next.without_applied("theme-b").find(Some("theme-b")).unwrap().applied);
}

#[test]
fn profiles_are_never_marked_applied() {
    let registry = registry(vec![inst_kind("prof", ExtensionKind::Profile)]);

    assert!(!registry.with_applied("prof").find(Some("prof")).unwrap().applied);
    // C# `Assert.Same(registry, registry.WithApplied("yok"))`: kayıt değişmez.
    assert_eq!(registry, registry.with_applied("yok"));
}

// Dosya gidiş-dönüşü

#[test]
fn store_round_trips_all_fields_through_a_temp_folder() {
    let dir = TempDir::new();
    let store = store_at(dir.combine("sub/extensions.json"));
    let entry = InstalledEntry {
        id: "koyhq.ornek".to_string(),
        version: "1.2.3".to_string(),
        kind: ExtensionKind::Background,
        name: "Örnek ş".to_string(),
        author: "Yazar".to_string(),
        license: "GPL-3.0-or-later".to_string(),
        installed_at: chrono::FixedOffset::east_opt(3 * 3600).unwrap().with_ymd_and_hms(2026, 11, 12, 10, 30, 0).unwrap(),
        source: ExtensionSource::File,
        applied: true,
        payload_file: "background.gif".to_string(),
    };

    assert!(store.save(&registry(vec![entry.clone()])));
    let loaded = store.load();

    assert_eq!(1, loaded.entries().len());
    assert_eq!(entry, loaded.entries()[0]);
    assert_eq!(entry.installed_at.offset(), loaded.entries()[0].installed_at.offset());
    assert!(!std::path::Path::new(&format!("{}.tmp", store.path().display())).exists());
}

#[test]
fn saved_file_uses_camel_case_and_string_enums() {
    let dir = TempDir::new();
    let store = store_at(dir.combine("extensions.json"));
    store.save(&registry(vec![inst_kind("aa", ExtensionKind::IconPack)]));

    let text = std::fs::read_to_string(store.path()).unwrap();

    assert!(text.contains("\"extensions\""), "{text}");
    assert!(text.contains("\"kind\": \"IconPack\""), "{text}");
    assert!(text.contains("\"source\": \"Store\""), "{text}");
    // .NET varsayilan kodlayicisi "+" karakterini ters bolu + u002B olarak yazar.
    assert!(text.contains("\"installedAt\": \"2026-11-01T00:00:00\\u002B00:00\""), "{text}");
}

#[test]
fn missing_file_loads_empty() {
    let dir = TempDir::new();

    assert!(store_at(dir.combine("yok.json")).load().entries().is_empty());
}

#[test]
fn corrupt_file_loads_empty_without_throwing() {
    for content in ["", "{ bozuk", "[]", "null", "{ \"version\": 99, \"extensions\": [] }", "{ \"version\": 1, \"extensions\": 5 }"] {
        let dir = TempDir::new();
        let path = dir.combine("extensions.json");
        std::fs::write(&path, content).unwrap();
        let (store, _errors) = store_with_errors(path);

        let loaded = store.load();

        assert!(loaded.entries().is_empty(), "{content:?}");
    }
}

fn one_entry_json(id: &str, name: &str) -> String {
    format!(
        "{{ \"id\": \"{id}\", \"version\": \"1.0.0\", \"kind\": \"Theme\", \"name\": \"{name}\", \"author\": \"X\", \"license\": \"MIT\", \"installedAt\": \"2026-11-01T00:00:00+00:00\", \"source\": \"Store\", \"applied\": false }}"
    )
}

#[test]
fn bad_entries_and_duplicates_are_dropped_but_good_ones_survive() {
    let dir = TempDir::new();
    let path = dir.combine("extensions.json");
    let good = one_entry_json("aa", "A");
    let bad_id = good.replace("\"aa\"", "\"../evil\"");
    let dup = good.replace("\"A\"", "\"Ikinci\"");
    std::fs::write(&path, format!("{{ \"version\": 1, \"extensions\": [{good},{bad_id},{dup}, null] }}")).unwrap();

    let loaded = store_at(path).load();

    assert_eq!(1, loaded.entries().len());
    assert_eq!("A", loaded.entries()[0].name);
    // Eksik `payloadFile` C# varsayılanı "" olur.
    assert_eq!("", loaded.entries()[0].payload_file);
}

#[test]
fn file_with_more_than_64_entries_is_truncated() {
    let dir = TempDir::new();
    let path = dir.combine("extensions.json");
    let entries: Vec<String> = (0..70).map(|i| one_entry_json(&format!("ext-{i}"), "A")).collect();
    std::fs::write(&path, format!("{{ \"version\": 1, \"extensions\": [{}] }}", entries.join(","))).unwrap();

    assert_eq!(64, store_at(path).load().entries().len());
}

#[test]
fn update_is_read_modify_write_and_null_means_no_change() {
    let dir = TempDir::new();
    let store = store_at(dir.combine("extensions.json"));

    assert!(store.update(upsert_aa));
    assert!(!store.update(|_| None));
    let loaded = store.load();
    assert_eq!(1, loaded.entries().len());
    assert_eq!("aa", loaded.entries()[0].id);
}

#[test]
fn save_to_unwritable_location_returns_false_and_reports() {
    let dir = TempDir::new();
    // Yolun üst parçası bir dosya: klasör oluşturulamaz.
    std::fs::write(dir.combine("blocker"), "x").unwrap();
    let (store, errors) = store_with_errors(dir.combine("blocker/extensions.json"));

    assert!(!store.save(&registry(vec![inst("aa")])));
    assert_eq!(1, errors.lock().unwrap().len());
}

#[test]
fn update_moves_corrupt_file_aside_then_writes() {
    for content in ["", "{ bozuk", "null", "{ \"version\": 0, \"extensions\": [] }"] {
        let dir = TempDir::new();
        let path = dir.combine("extensions.json");
        std::fs::write(&path, content).unwrap();
        let store = silent_store(path.clone());

        assert!(store.update(upsert_aa), "{content:?}");

        assert_eq!(content, std::fs::read_to_string(format!("{}.bad", path.display())).unwrap());
        let loaded = store.load();
        assert_eq!(1, loaded.entries().len());
        assert_eq!("aa", loaded.entries()[0].id);
    }
}

#[test]
fn update_keeps_an_earlier_bad_backup() {
    let dir = TempDir::new();
    let path = dir.combine("extensions.json");
    std::fs::write(format!("{}.bad", path.display()), "ilk").unwrap();
    std::fs::write(&path, "{ bozuk").unwrap();
    let store = silent_store(path.clone());

    assert!(store.update(upsert_aa));

    assert_eq!("ilk", std::fs::read_to_string(format!("{}.bad", path.display())).unwrap());
    let bad_files = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("extensions.json") && n.ends_with(".bad"))
        .count();
    assert_eq!(2, bad_files);
}

#[test]
fn update_never_overwrites_a_newer_version_file() {
    let dir = TempDir::new();
    let path = dir.combine("extensions.json");
    let newer = "{ \"version\": 99, \"extensions\": [] }";
    std::fs::write(&path, newer).unwrap();
    let store = store_at(path.clone());

    assert!(!store.update(upsert_aa));
    assert!(!store.save(&registry(vec![inst("aa")])));

    assert_eq!(newer, std::fs::read_to_string(&path).unwrap());
    assert!(!std::path::Path::new(&format!("{}.bad", path.display())).exists());
}

// Dosya başka bir süreçte özel kilitli gibi: okuma G/Ç hatası verir (Windows paylaşım kipi).
#[cfg(windows)]
#[test]
fn update_does_not_write_when_the_file_is_unreadable() {
    use std::os::windows::fs::OpenOptionsExt;

    let dir = TempDir::new();
    let path = dir.combine("extensions.json");
    let store = silent_store(path.clone());
    assert!(store.save(&registry(vec![inst("aa")])));
    let before = std::fs::read_to_string(&path).unwrap();

    {
        let _lock = std::fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
        assert!(!store.update(|r| Some(r.remove("aa"))));
    }

    assert_eq!(before, std::fs::read_to_string(&path).unwrap());
    let loaded = store.load();
    assert_eq!(1, loaded.entries().len());
    assert_eq!("aa", loaded.entries()[0].id);
}

// Rust'a özgü ek: C# `UseStringEnumConverter` büyük/küçük harf duyarsız okur, tamsayı değeri de kabul eder;
// tanınmayan metin adı dosyayı bozuk sayar, tanımsız tamsayı yalnız o kaydı eler.
#[test]
fn enum_reading_follows_the_string_enum_converter() {
    let dir = TempDir::new();
    let path = dir.combine("extensions.json");
    let lower = one_entry_json("aa", "A").replace("\"Theme\"", "\"iconpack\"");
    let numeric = one_entry_json("bb", "B").replace("\"Theme\"", "3");
    let undefined = one_entry_json("cc", "C").replace("\"Theme\"", "99");
    std::fs::write(&path, format!("{{ \"version\": 1, \"extensions\": [{lower},{numeric},{undefined}] }}")).unwrap();
    let loaded = silent_store(path.clone()).load();
    assert_eq!(ExtensionKind::IconPack, loaded.find(Some("aa")).unwrap().kind);
    assert_eq!(ExtensionKind::Profile, loaded.find(Some("bb")).unwrap().kind);
    assert!(loaded.find(Some("cc")).is_none());

    let unknown_name = one_entry_json("aa", "A").replace("\"Theme\"", "\"Widget\"");
    std::fs::write(&path, format!("{{ \"version\": 1, \"extensions\": [{unknown_name}] }}")).unwrap();
    let store = silent_store(path.clone());
    assert!(store.load().entries().is_empty());
    assert!(store.update(upsert_aa));
    assert!(std::path::Path::new(&format!("{}.bad", path.display())).exists());
}

//! Karşılığı: `Extensions/Store/RepositoryIndexTests.cs`.

use super::store_test_kit::repo_root;
use crate::extensions::store::*;

// Depodaki gerçek store/index.json uygulamanın ayrıştırıcısından atlanmadan geçmeli (betik ve C# kuralları ayrışmasın).
// C# deposu yoksa (Rust çalışma alanı tek başına) atlanır.
#[test]
fn committed_store_index_parses_without_skipped_items_and_has_turkish_texts() {
    let Some(root) = repo_root() else {
        eprintln!("atlandı: C# deposu (store/index.json) bulunamadı");
        return;
    };
    let bytes = std::fs::read(root.join("store").join("index.json")).expect("index.json okunmalı");

    let result = StoreIndexParser::parse_bytes(&bytes);

    assert!(result.is_success());
    assert_eq!(0, result.skipped_items);
    let index = result.index.unwrap();
    assert!(!index.items.is_empty());
    for item in &index.items {
        assert_ne!(item.summary.resolve(Some("en")), item.summary.resolve(Some("tr")), "{}", item.id);
        assert_ne!(item.description.resolve(Some("en")), item.description.resolve(Some("tr")), "{}", item.id);
    }
}

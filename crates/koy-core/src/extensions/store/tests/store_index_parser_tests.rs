//! Karşılığı: `Extensions/Store/StoreIndexParserTests.cs`.

use chrono::{NaiveDate, TimeZone};

use super::store_test_kit::*;
use crate::extensions::store::*;

fn item_with(extra: &str) -> String {
    item_json("koyhq.ornek", "theme", extra)
}

fn item_id(id: &str) -> String {
    item_json(id, "theme", "")
}

#[test]
fn valid_index_is_parsed() {
    let result = StoreIndexParser::parse(Some(&index_json(&[item_with(
        ", \"downloads\": 7, \"homepage\": \"https://github.com/koyhq/koy\", \"description\": \"a\\nb\"",
    )])));

    assert!(result.is_success());
    assert_eq!(0, result.skipped_items);
    let index = result.index.unwrap();
    assert_eq!(1, index.items.len());
    let item = &index.items[0];
    assert_eq!("koyhq.ornek", item.id);
    assert_eq!(ExtensionKind::Theme, item.kind);
    assert_eq!(7, item.downloads);
    assert_eq!(NaiveDate::from_ymd_opt(2026, 11, 12).unwrap(), item.updated);
    assert_eq!("a\nb", item.description.resolve(Some("en")));
    assert_eq!(1234, item.package.size);
    assert_eq!("a".repeat(64), item.package.sha256);
    assert_eq!(Some(chrono::FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 12, 0, 0, 0).unwrap()), index.generated);
}

#[test]
fn closed_set_of_kinds_is_parsed() {
    for (json, kind) in [
        ("theme", ExtensionKind::Theme),
        ("iconpack", ExtensionKind::IconPack),
        ("background", ExtensionKind::Background),
        ("profile", ExtensionKind::Profile),
    ] {
        let result = StoreIndexParser::parse(Some(&index_json(&[item_json("koyhq.ornek", json, "")])));

        assert_eq!(kind, result.index.unwrap().items[0].kind, "{json}");
    }
}

#[test]
fn unknown_kind_skips_only_that_item() {
    for kind in ["Theme", "widget", ""] {
        let result = StoreIndexParser::parse(Some(&index_json(&[item_json("koyhq.ornek", kind, ""), item_id("koyhq.baska")])));

        assert_eq!(1, result.skipped_items, "{kind:?}");
        let items = result.index.unwrap().items;
        assert_eq!(1, items.len());
        assert_eq!("koyhq.baska", items[0].id);
    }
}

#[test]
fn duplicate_id_skips_the_second() {
    let first = item_with(", \"downloads\": 1");
    let second = item_with(", \"downloads\": 2");
    let result = StoreIndexParser::parse(Some(&index_json(&[first, second])));

    assert_eq!(1, result.skipped_items);
    let items = result.index.unwrap().items;
    assert_eq!(1, items.len());
    assert_eq!(1, items[0].downloads);
}

#[test]
fn unknown_fields_are_ignored() {
    let result = StoreIndexParser::parse(Some(&index_json(&[item_with(", \"rating\": 5, \"tags\": [\"a\"]")])));

    assert_eq!(1, result.index.unwrap().items.len());
    assert_eq!(0, result.skipped_items);
}

#[test]
fn http_package_url_skips_item() {
    let item = default_item_json().replace(PACKAGE_URL, "http://example.com/p.koyext");

    let result = StoreIndexParser::parse(Some(&index_json(&[item])));

    assert_eq!(1, result.skipped_items);
    assert!(result.index.unwrap().items.is_empty());
}

#[test]
fn bad_urls_skip_item() {
    for url in [
        "ftp://example.com/p",
        "file:///c:/p.koyext",
        "https://user:pw@example.com/p",
        "https://exa mple.com/p",
        "//example.com/p",
        "example.com/p",
    ] {
        let item = default_item_json().replace(PACKAGE_URL, url);

        assert_eq!(1, StoreIndexParser::parse(Some(&index_json(&[item]))).skipped_items, "{url}");
    }
}

#[test]
fn http_homepage_skips_item() {
    let result = StoreIndexParser::parse(Some(&index_json(&[item_with(", \"homepage\": \"http://example.com\"")])));

    assert_eq!(1, result.skipped_items);
}

#[test]
fn bad_preview_is_dropped_but_item_is_kept() {
    let result =
        StoreIndexParser::parse(Some(&index_json(&[item_with(", \"preview\": { \"url\": \"http://x.com/p.png\", \"sha256\": \"zz\" }")])));

    assert_eq!(0, result.skipped_items);
    assert!(result.index.unwrap().items[0].preview.is_none());
}

#[test]
fn good_preview_is_kept_and_hash_is_lowercased() {
    let hash = "A".repeat(64);
    let result = StoreIndexParser::parse(Some(&index_json(&[item_with(&format!(
        ", \"preview\": {{ \"url\": \"{PREVIEW_URL}\", \"sha256\": \"{hash}\" }}"
    ))])));

    assert_eq!("a".repeat(64), result.index.unwrap().items[0].preview.clone().unwrap().sha256);
}

#[test]
fn bad_sha256_skips_item() {
    for hash in ["short", ""] {
        let item = default_item_json().replace(&"a".repeat(64), hash);

        assert_eq!(1, StoreIndexParser::parse(Some(&index_json(&[item]))).skipped_items, "{hash:?}");
    }
}

#[test]
fn non_hex_sha256_skips_item() {
    let item = default_item_json().replace(&"a".repeat(64), &"g".repeat(64));

    assert_eq!(1, StoreIndexParser::parse(Some(&index_json(&[item]))).skipped_items);
}

#[test]
fn invalid_id_skips_item() {
    for id in ["Bad Id", "X", "con", "a..b"] {
        assert_eq!(1, StoreIndexParser::parse(Some(&index_json(&[item_id(id)]))).skipped_items, "{id}");
    }
}

#[test]
fn summary_over_limit_skips_item_and_limit_itself_is_accepted() {
    let at_limit = default_item_json().replace("\"Ozet\"", &format!("\"{}\"", "s".repeat(200)));
    let over = item_id("koyhq.uzun").replace("\"Ozet\"", &format!("\"{}\"", "s".repeat(201)));

    let result = StoreIndexParser::parse(Some(&index_json(&[at_limit, over])));

    assert_eq!(1, result.skipped_items);
    assert_eq!(1, result.index.unwrap().items.len());
}

#[test]
fn description_over_limit_skips_item_and_limit_itself_is_accepted() {
    let at_limit = item_with(&format!(", \"description\": \"{}\"", "d".repeat(2000)));
    let over = item_json("koyhq.uzun", "theme", &format!(", \"description\": \"{}\"", "d".repeat(2001)));

    let result = StoreIndexParser::parse(Some(&index_json(&[at_limit, over])));

    assert_eq!(1, result.skipped_items);
    let items = result.index.unwrap().items;
    assert_eq!(1, items.len());
    assert_eq!(2000, items[0].description.resolve(Some("en")).encode_utf16().count());
}

#[test]
fn summary_with_newline_is_rejected_but_description_allows_it() {
    let bad = default_item_json().replace("\"Ozet\"", "\"a\\nb\"");

    assert_eq!(1, StoreIndexParser::parse(Some(&index_json(&[bad]))).skipped_items);
}

#[test]
fn missing_required_fields_skip_item() {
    let no_license = default_item_json().replace("\"license\": \"GPL-3.0-or-later\",", "");
    let no_updated = item_id("koyhq.b").replace("\"updated\": \"2026-11-12\",", "");
    let bad_date = item_id("koyhq.c").replace("2026-11-12\",", "12.11.2026\",");

    let result = StoreIndexParser::parse(Some(&index_json(&[no_license, no_updated, bad_date])));

    assert_eq!(3, result.skipped_items);
    assert!(result.index.unwrap().items.is_empty());
}

#[test]
fn negative_downloads_or_size_skip_item() {
    let negative_downloads = item_with(", \"downloads\": -1");
    let zero_size = item_id("koyhq.b").replace("\"size\": 1234", "\"size\": 0");
    let huge_size = item_id("koyhq.c").replace("\"size\": 1234", &format!("\"size\": {}", PackageEntryPolicy::MAX_DOWNLOAD_BYTES + 1));

    assert_eq!(3, StoreIndexParser::parse(Some(&index_json(&[negative_downloads, zero_size, huge_size]))).skipped_items);
}

#[test]
fn non_object_item_is_skipped() {
    let json = format!("{{ \"format\": \"koy-store-index\", \"formatVersion\": 1, \"items\": [1, \"x\", null, {}] }}", default_item_json());

    let result = StoreIndexParser::parse(Some(&json));

    assert_eq!(3, result.skipped_items);
    assert_eq!(1, result.index.unwrap().items.len());
}

#[test]
fn wrong_format_name_is_rejected() {
    let result = StoreIndexParser::parse(Some("{ \"format\": \"other\", \"formatVersion\": 1, \"items\": [] }"));

    assert_eq!(StoreIndexError::NotAStoreIndex, result.error);
    assert!(result.index.is_none());
}

#[test]
fn newer_format_version_is_rejected() {
    let result = StoreIndexParser::parse(Some("{ \"format\": \"koy-store-index\", \"formatVersion\": 2, \"items\": [] }"));

    assert_eq!(StoreIndexError::UnsupportedFormatVersion, result.error);
}

#[test]
fn structurally_wrong_index_is_rejected() {
    for json in [
        "{ \"format\": \"koy-store-index\", \"items\": [] }",
        "{ \"format\": \"koy-store-index\", \"formatVersion\": 0, \"items\": [] }",
        "{ \"format\": \"koy-store-index\", \"formatVersion\": \"1\", \"items\": [] }",
        "{ \"format\": \"koy-store-index\", \"formatVersion\": 1 }",
        "{ \"format\": \"koy-store-index\", \"formatVersion\": 1, \"items\": {} }",
        "[]",
    ] {
        assert_eq!(StoreIndexError::NotAStoreIndex, StoreIndexParser::parse(Some(json)).error, "{json}");
    }
}

#[test]
fn malformed_json_is_rejected() {
    for json in ["", "{ not json", "{ \"format\": "] {
        assert_eq!(StoreIndexError::MalformedJson, StoreIndexParser::parse(Some(json)).error, "{json:?}");
    }
}

#[test]
fn null_text_is_rejected_without_throwing() {
    assert!(!StoreIndexParser::parse(None).is_success());
}

#[test]
fn more_than_500_items_reject_whole_index_and_exactly_500_is_accepted() {
    let make = |n: usize| -> Vec<String> { (0..n).map(|i| item_id(&format!("koyhq.i{i}"))).collect() };

    assert_eq!(StoreIndexError::TooManyItems, StoreIndexParser::parse(Some(&index_json(&make(501)))).error);
    assert_eq!(500, StoreIndexParser::parse(Some(&index_json(&make(500)))).index.unwrap().items.len());
}

#[test]
fn index_over_one_megabyte_is_rejected_before_parsing() {
    let big = format!(
        "{{ \"format\": \"koy-store-index\", \"formatVersion\": 1, \"items\": [], \"pad\": \"{}\" }}",
        "x".repeat(StoreIndexParser::MAX_BYTES)
    );

    assert_eq!(StoreIndexError::TooLarge, StoreIndexParser::parse(Some(&big)).error);
    assert_eq!(StoreIndexError::TooLarge, StoreIndexParser::parse_bytes(big.as_bytes()).error);
}

#[test]
fn multibyte_text_is_measured_in_bytes() {
    // 600k karakter ama 1.2 MB UTF-8: karakter sayısına bakan bir denetim bunu kaçırırdı.
    let big = format!("{{ \"pad\": \"{}\" }}", "ş".repeat(600_000));

    assert_eq!(StoreIndexError::TooLarge, StoreIndexParser::parse(Some(&big)).error);
}

#[test]
fn utf8_bom_is_tolerated() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(index_json(&[default_item_json()]).as_bytes());

    assert!(StoreIndexParser::parse_bytes(&bytes).is_success());
}

#[test]
fn deeply_nested_json_is_rejected_not_thrown() {
    let json = format!(
        "{{ \"format\": \"koy-store-index\", \"formatVersion\": 1, \"items\": [], \"x\": {}{} }}",
        "[".repeat(200),
        "]".repeat(200)
    );

    assert_eq!(StoreIndexError::MalformedJson, StoreIndexParser::parse(Some(&json)).error);
}

#[test]
fn real_store_index_in_repository_parses_without_skips() {
    let Some(root) = repo_root() else {
        eprintln!("atlandı: C# deposu (store/index.json) bulunamadı");
        return;
    };

    let result = StoreIndexParser::parse_bytes(&std::fs::read(root.join("store").join("index.json")).unwrap());

    assert!(result.is_success(), "{:?}", result.error);
    assert_eq!(0, result.skipped_items);
    let ids: std::collections::HashSet<String> = result.index.unwrap().items.into_iter().map(|i| i.id).collect();
    for id in [
        "koyhq.nord-theme",
        "koyhq.solarized-theme",
        "koyhq.high-contrast-theme",
        "koyhq.mono-icons",
        "koyhq.aurora-background",
        "koyhq.minimal-profile",
    ] {
        assert!(ids.contains(id), "{id}");
    }
}

// Rust'a özgü ek: yinelenen anahtar `JsonDocument` gibi korunur (son değer geçerli) ve yinelenen dil bozuk sayılır.
#[test]
fn duplicate_property_names_use_the_last_value() {
    let item = default_item_json().replace("\"name\": \"Ornek\",", "\"name\": \"Ilk\", \"name\": \"Son\",");

    let items = StoreIndexParser::parse(Some(&index_json(&[item]))).index.unwrap().items;

    assert_eq!("Son", items[0].name);
}

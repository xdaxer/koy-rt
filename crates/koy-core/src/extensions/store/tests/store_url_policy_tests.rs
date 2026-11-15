//! Karşılığı: `Extensions/Store/StoreUrlPolicyTests.cs`.

use super::store_test_kit::*;
use crate::extensions::store::uri::Uri;
use crate::extensions::store::*;

#[test]
fn only_the_store_repository_on_raw_githubusercontent_is_trusted() {
    let rows: [(Option<&str>, bool); 12] = [
        (Some("https://raw.githubusercontent.com/koyhq/koy/main/store/packages/a.koyext"), true),
        (Some("https://RAW.GITHUBUSERCONTENT.COM/koyhq/koy/abc123/store/previews/a.png"), true),
        (Some("http://raw.githubusercontent.com/koyhq/koy/main/store/packages/a.koyext"), false),
        (Some("https://example.com/koyhq/koy/main/a.koyext"), false),
        (Some("https://raw.githubusercontent.com.evil.com/koyhq/koy/main/a.koyext"), false),
        (Some("https://raw.githubusercontent.com/other/repo/main/a.koyext"), false),
        (Some("https://raw.githubusercontent.com/koyhq/koy-fork/main/a.koyext"), false),
        (Some("https://raw.githubusercontent.com/koyhq/koy/../evil/a.koyext"), false),
        (Some("https://raw.githubusercontent.com:8443/koyhq/koy/main/a.koyext"), false),
        (Some("https://user@raw.githubusercontent.com/koyhq/koy/main/a.koyext"), false),
        (Some(""), false),
        (None, false),
    ];
    for (url, trusted) in rows {
        assert_eq!(trusted, StoreUrlPolicy::is_trusted_asset_url(url), "{url:?}");
        if let Some(uri) = url.and_then(Uri::try_create) {
            assert_eq!(trusted, StoreUrlPolicy::is_trusted_asset_uri(Some(&uri)), "{url:?}");
        }
    }
}

#[test]
fn final_response_uri_after_a_redirect_is_checked_the_same_way() {
    assert!(!StoreUrlPolicy::is_trusted_asset_uri(Some(&Uri::new("https://cdn.example.com/koyhq/koy/a.koyext").unwrap())));
    assert!(!StoreUrlPolicy::is_trusted_asset_uri(None));
}

#[test]
fn index_item_with_untrusted_package_host_is_skipped() {
    let item = default_item_json().replace(PACKAGE_URL, "https://example.com/koyhq/koy/p.koyext");

    let result = StoreIndexParser::parse(Some(&index_json(&[item])));

    assert_eq!(1, result.skipped_items);
    assert!(result.index.unwrap().items.is_empty());
}

#[test]
fn index_item_with_untrusted_preview_keeps_the_item_but_drops_the_preview() {
    let hash = "a".repeat(64);
    let json = index_json(&[item_json(
        "koyhq.ornek",
        "theme",
        &format!(", \"preview\": {{ \"url\": \"https://example.com/koyhq/koy/p.png\", \"sha256\": \"{hash}\" }}"),
    )]);

    let result = StoreIndexParser::parse(Some(&json));

    assert_eq!(0, result.skipped_items);
    let items = result.index.unwrap().items;
    assert_eq!(1, items.len());
    assert!(items[0].preview.is_none());
}

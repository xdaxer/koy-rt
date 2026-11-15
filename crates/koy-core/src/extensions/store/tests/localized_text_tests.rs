//! Karşılığı: `Extensions/Store/LocalizedTextTests.cs`.

use super::store_test_kit::*;
use crate::extensions::store::*;

fn pairs(values: &[(&str, &str)]) -> LocalizedText {
    LocalizedText::new(values.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

fn both() -> LocalizedText {
    pairs(&[("en", "Hello"), ("tr", "Merhaba")])
}

#[test]
fn resolve_picks_the_requested_language_then_english_then_anything() {
    assert_eq!("Merhaba", both().resolve(Some("tr")));
    assert_eq!("Merhaba", both().resolve(Some("TR")));
    assert_eq!("Hello", both().resolve(Some("de")));
    assert_eq!("Hello", both().resolve(None));
    assert_eq!("Hello", both().resolve(Some("")));
    assert_eq!("Hallo", pairs(&[("de", "Hallo")]).resolve(Some("fr")));
    assert_eq!("", LocalizedText::empty().resolve(Some("tr")));
}

#[test]
fn plain_text_is_the_same_in_every_language() {
    let plain: LocalizedText = "Ozet".into();

    assert_eq!("Ozet", plain.resolve(Some("tr")));
    assert_eq!("Ozet", plain.resolve(Some("en")));
}

#[test]
fn index_accepts_plain_string_and_language_object() {
    let plain = StoreIndexParser::parse(Some(&index_json(&[default_item_json()])));
    let localized = StoreIndexParser::parse(Some(&index_json(&[
        default_item_json().replace("\"summary\": \"Ozet\"", "\"summary\": { \"en\": \"Summary\", \"tr\": \"Özet\" }"),
    ])));

    let plain_items = plain.index.unwrap().items;
    assert_eq!(1, plain_items.len());
    assert_eq!("Ozet", plain_items[0].summary.resolve(Some("tr")));
    let items = localized.index.unwrap().items;
    assert_eq!(1, items.len());
    assert_eq!("Özet", items[0].summary.resolve(Some("tr")));
    assert_eq!("Summary", items[0].summary.resolve(Some("fr")));
}

#[test]
fn description_may_be_a_language_object_too() {
    let json = index_json(&[item_json("koyhq.ornek", "theme", ", \"description\": { \"en\": \"a\\nb\", \"tr\": \"c\\nd\" }")]);

    let items = StoreIndexParser::parse(Some(&json)).index.unwrap().items;

    assert_eq!(1, items.len());
    assert_eq!("c\nd", items[0].description.resolve(Some("tr")));
    assert_eq!("a\nb", items[0].description.resolve(Some("en")));
}

#[test]
fn bad_localized_summary_skips_the_item() {
    let rows = [
        "{ \"tr\": \"Özet\" }", // "en" yok
        "{}",
        "{ \"en\": \"\" }", // boş özet
        "{ \"en\": \"x\", \"tr\": \"\" }",
        "{ \"en\": \"x\", \"turkish\": \"y\" }", // dil kodu iki harf değil
        "{ \"en\": \"x\", \"TR\": \"y\" }",      // büyük harf
        "{ \"en\": \"x\", \"tr\": 5 }",
        "{ \"en\": \"x\", \"en\": \"y\" }",     // yinelenen dil
        "{ \"en\": \"x\", \"tr\": \"a\\nb\" }", // özette satır sonu yok
        "{ \"en\": \"x\", \"tr\": \"a\\u202Eb\" }", // yön karakteri
        "[\"x\"]",
        "5",
        "null",
    ];
    for summary in rows {
        let item = default_item_json().replace("\"summary\": \"Ozet\"", &format!("\"summary\": {summary}"));

        let result = StoreIndexParser::parse(Some(&index_json(&[item])));

        assert_eq!(1, result.skipped_items, "{summary}");
        assert!(result.index.unwrap().items.is_empty(), "{summary}");
    }
}

#[test]
fn every_language_value_obeys_the_length_limit() {
    let too_long = "a".repeat(201);
    let item = default_item_json().replace("\"summary\": \"Ozet\"", &format!("\"summary\": {{ \"en\": \"x\", \"tr\": \"{too_long}\" }}"));
    let long_description = "a".repeat(2001);

    assert_eq!(1, StoreIndexParser::parse(Some(&index_json(&[item]))).skipped_items);
    assert_eq!(
        1,
        StoreIndexParser::parse(Some(&index_json(&[item_json(
            "koyhq.ornek",
            "theme",
            &format!(", \"description\": {{ \"en\": \"x\", \"tr\": \"{long_description}\" }}")
        )])))
        .skipped_items
    );
}

#[test]
fn catalog_shows_the_ui_language_and_searches_it() {
    let mut item = item("aa", ExtensionKind::Theme, "1.0.0");
    item.name = "Alpha".to_string();
    item.summary = pairs(&[("en", "Dark blue"), ("tr", "Koyu mavi")]);
    item.description = pairs(&[("en", "Long"), ("tr", "Uzun")]);
    let state = StoreListState { index: Some(StoreIndex::new(None, vec![item])), ..StoreListState::initial() };

    let en = StoreListState { language: "en".to_string(), ..state.clone() }.visible();
    let tr = StoreListState { language: "tr".to_string(), ..state.clone() }.visible();
    let search_tr = StoreListState {
        language: "tr".to_string(),
        query: StoreQuery { search: Some("koyu".to_string()), ..StoreQuery::default() },
        ..state.clone()
    }
    .visible();
    let search_en = StoreListState {
        language: "en".to_string(),
        query: StoreQuery { search: Some("koyu".to_string()), ..StoreQuery::default() },
        ..state.clone()
    }
    .visible();

    assert_eq!(1, en.len());
    assert_eq!(1, tr.len());
    assert_eq!(("Dark blue", "Long"), (en[0].summary.as_str(), en[0].description.as_str()));
    assert_eq!(("Koyu mavi", "Uzun"), (tr[0].summary.as_str(), tr[0].description.as_str()));
    assert_eq!(1, search_tr.len());
    assert!(search_en.is_empty());
}

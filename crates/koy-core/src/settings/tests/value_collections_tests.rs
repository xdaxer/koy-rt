//! `ValueCollections.cs` ve JSON katmanı için Rust'a özgü sınamalar (C#'ta doğrudan testi yoktu; davranış
//! `AppSettingsEqualityTests` / `AppIconOverrideTests` / altın dosya testleriyle dolaylı korunuyordu).

use serde_json::Value;

use crate::appearance::IconOverrideMap;
use crate::settings::json_tree::{self, Seg};
use crate::settings::*;

#[test]
fn value_list_equality_is_by_content_and_order() {
    assert_eq!(ValueList::<String>::from(["a", "b"]), ValueList::<String>::from(["a", "b"]));
    assert_ne!(ValueList::<String>::from(["a", "b"]), ValueList::<String>::from(["b", "a"]));
}

#[test]
fn value_dictionary_keys_are_case_insensitive_and_keep_insertion_order() {
    let mut d = ValueDictionary::new();
    d.insert("Zeta", 1);
    d.insert("alpha", 2);
    // Var olan anahtar değeri yerinde değiştirir, ilk yazım ve sıra kalır.
    d.insert("ZETA", 3);

    assert_eq!(2, d.len());
    assert_eq!(3, d["zeta"]);
    assert_eq!(vec!["Zeta", "alpha"], d.keys().map(String::as_str).collect::<Vec<_>>());
    assert!(d.contains_key("ALPHA"));
    assert_eq!(Some(2), d.remove("Alpha"));
    assert!(!d.contains_key("alpha"));
}

#[test]
fn value_dictionary_equality_ignores_order_and_key_case() {
    let a = ValueDictionary::from_items([("a".to_string(), 1), ("B".to_string(), 2)]);
    let b = ValueDictionary::from_items([("b".to_string(), 2), ("A".to_string(), 1)]);

    assert_eq!(a, b);
    assert_ne!(a, ValueDictionary::from_items([("a".to_string(), 1)]));
}

#[test]
fn icon_override_map_is_case_insensitive() {
    let mut d = ValueDictionary::<String>::new();
    d.set_value("Notepad.lnk", "n.png".to_string());

    assert_eq!(Some("n.png"), d.get_value("NOTEPAD.LNK"));
    d.remove_value("notepad.lnk");
    assert_eq!(None, d.get_value("Notepad.lnk"));
}

#[test]
fn json_parser_rejects_duplicates_and_reads_overflowing_numbers_as_infinity() {
    assert!(json_tree::parse(r#"{"a":1,"a":2}"#).is_err());
    assert!(json_tree::parse("{ not json").is_err());
    assert!(json_tree::parse("[1,]").is_err());
    let v = json_tree::parse("[1e400, 1.5, 7]").unwrap();
    let Value::Array(items) = v else { panic!() };
    assert_eq!(json_tree::POSITIVE_INFINITY_TOKEN, items[0].as_str().unwrap());
    assert_eq!(Some(1.5), items[1].as_f64());
    assert_eq!(Some(7), items[2].as_i64());
}

#[test]
fn json_writer_matches_system_text_json_formatting() {
    let v = json_tree::parse(r#"{"a":[1,2.5,12.0],"b":{},"c":[],"d":"x\"y\\z\n é+"}"#).unwrap();
    let text = json_tree::to_pretty_string(&v);
    let nl = if cfg!(windows) { "\r\n" } else { "\n" };

    let expected = [
        "{",
        "  \"a\": [",
        "    1,",
        "    2.5,",
        "    12",
        "  ],",
        "  \"b\": {},",
        "  \"c\": [],",
        "  \"d\": \"x\\u0022y\\\\z\\n \\u00E9\\u002B\"",
        "}",
    ]
    .join(nl);
    assert_eq!(expected, text);
}

#[test]
fn drop_at_removes_the_innermost_list_entry_or_the_leaf_field() {
    let mut root = json_tree::parse(r#"{"A":{"L":[{"X":1},{"X":2}],"B":true}}"#).unwrap();

    // Liste öğesinin içindeki alan: öğenin tamamı düşer.
    assert!(json_tree::drop_at(&mut root, &[Seg::Prop("A".into()), Seg::Prop("L".into()), Seg::Index(0), Seg::Prop("X".into())]));
    assert_eq!(r#"{"A":{"L":[{"X":2}],"B":true}}"#, serde_json::to_string(&root).unwrap());

    // Listeden geçmeyen yol: yalnız alan düşer.
    assert!(json_tree::drop_at(&mut root, &[Seg::Prop("A".into()), Seg::Prop("B".into())]));
    assert_eq!(r#"{"A":{"L":[{"X":2}]}}"#, serde_json::to_string(&root).unwrap());

    // Özel karakterli sözlük anahtarı: üst düzey bölüm düşer.
    assert!(json_tree::drop_at(&mut root, &[Seg::Prop("A".into()), Seg::Key("a.b".into())]));
    assert_eq!("{}", serde_json::to_string(&root).unwrap());
}

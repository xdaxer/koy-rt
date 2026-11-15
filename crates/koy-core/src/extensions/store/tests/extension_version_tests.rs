//! Karşılığı: `Extensions/Store/ExtensionVersionTests.cs`.

use crate::extensions::store::ExtensionVersion;

#[test]
fn numeric_comparison() {
    let rows = [
        ("1.0.0", "1.0.0", 0),
        ("1.0.1", "1.0.0", 1),
        ("1.0.0", "1.0.1", -1),
        ("1.10.0", "1.9.0", 1),
        ("1.9.0", "1.10.0", -1),
        ("2.0.0", "1.99.99", 1),
        ("0.0.9", "0.0.10", -1),
        ("10.0.0", "9.0.0", 1),
        ("999999.999999.999999", "999999.999999.999998", 1),
    ];
    for (left, right, sign) in rows {
        assert_eq!(sign, ExtensionVersion::compare(Some(left), Some(right)).signum(), "{left} {right}");
    }
}

#[test]
fn invalid_versions_do_not_parse() {
    let rows = [
        Some("1.0"),
        Some("1.0.0.0"),
        Some("01.0.0"),
        Some("1.0.x"),
        Some("v1.0.0"),
        Some("1.0.0-beta"),
        Some("-1.0.0"),
        Some("1234567.0.0"),
        Some(""),
        None,
    ];
    for text in rows {
        assert!(ExtensionVersion::try_parse(text).is_none(), "{text:?}");
    }
}

#[test]
fn invalid_sorts_below_valid_and_two_invalids_are_equal() {
    assert!(ExtensionVersion::compare(Some("bad"), Some("0.0.1")) < 0);
    assert!(ExtensionVersion::compare(Some("0.0.1"), Some("bad")) > 0);
    assert_eq!(0, ExtensionVersion::compare(Some("bad"), None));
}

#[test]
fn is_newer_is_strict_and_false_for_invalid() {
    assert!(ExtensionVersion::is_newer(Some("1.1.0"), Some("1.0.9")));
    assert!(!ExtensionVersion::is_newer(Some("1.0.0"), Some("1.0.0")));
    assert!(!ExtensionVersion::is_newer(Some("0.9.0"), Some("1.0.0")));
    assert!(!ExtensionVersion::is_newer(Some("bad"), Some("1.0.0")));
    assert!(!ExtensionVersion::is_newer(Some("2.0.0"), Some("bad")));
}

#[test]
fn parsed_parts_and_to_string_round_trip() {
    let v = ExtensionVersion::try_parse(Some("3.14.159")).expect("geçerli sürüm");
    assert_eq!(ExtensionVersion::new(3, 14, 159), v);
    assert_eq!("3.14.159", v.to_string());
}

//! Karşılığı: `Extensions/Store/PackageManifestParserTests.cs`.

use super::store_test_kit::*;
use crate::extensions::store::*;
use crate::extensions::{ExtensionEntryKind, ExtensionManifestError};

#[test]
fn valid_manifest_is_parsed_with_kind_and_license() {
    let result = PackageManifestParser::parse(Some(&manifest("koyhq.nord-theme", "theme", "1.0.0", None)));

    assert!(result.is_valid());
    let parsed = result.manifest.unwrap();
    assert_eq!("koyhq.nord-theme", parsed.manifest.id());
    assert_eq!(ExtensionKind::Theme, parsed.kind);
    assert_eq!("MIT", parsed.license);
    assert_eq!(ExtensionEntryKind::Data, parsed.manifest.entry_kind());
}

#[test]
fn missing_entry_kind_and_capabilities_default_to_data_with_none() {
    let json = "{ \"id\": \"ab\", \"name\": \"N\", \"version\": \"1.0.0\", \"author\": \"A\", \"kind\": \"profile\", \"license\": \"MIT\" }";

    assert!(PackageManifestParser::parse(Some(json)).is_valid());
}

#[test]
fn capabilities_are_refused_for_data_packages() {
    let json = manifest("ab", "theme", "1.0.0", None).replace("\"capabilities\": []", "\"capabilities\": [\"network\"]");

    assert_eq!(ExtensionInstallError::CapabilitiesNotAllowed, PackageManifestParser::parse(Some(&json)).error);
}

#[test]
fn unknown_capability_is_an_invalid_manifest() {
    let json = manifest("ab", "theme", "1.0.0", None).replace("\"capabilities\": []", "\"capabilities\": [\"shell\"]");

    let result = PackageManifestParser::parse(Some(&json));

    assert_eq!(ExtensionInstallError::InvalidManifest, result.error);
    assert_eq!(ExtensionManifestError::UnknownCapability, result.manifest_error);
}

#[test]
fn invalid_fields_make_manifest_invalid() {
    let rows = [
        ("\"id\": \"ab\"", "\"id\": \"Bad Id\""),
        ("\"version\": \"1.0.0\"", "\"version\": \"1.0\""),
        ("\"kind\": \"theme\"", "\"kind\": \"widget\""),
        ("\"license\": \"MIT\"", "\"license\": \"\""),
        ("\"entryKind\": \"data\"", "\"entryKind\": \"process\""),
        ("\"capabilities\": []", "\"capabilities\": \"network\""),
    ];
    for (from, to) in rows {
        let json = manifest("ab", "theme", "1.0.0", Some(", \"entryKind\": \"data\"")).replace(from, to);

        assert_eq!(ExtensionInstallError::InvalidManifest, PackageManifestParser::parse(Some(&json)).error, "{from} -> {to}");
    }
}

#[test]
fn garbage_is_an_invalid_manifest() {
    for json in ["", "not json", "[]", "null", "{}"] {
        assert_eq!(ExtensionInstallError::InvalidManifest, PackageManifestParser::parse(Some(json)).error, "{json:?}");
    }
}

#[test]
fn null_text_does_not_throw() {
    assert!(!PackageManifestParser::parse(None).is_valid());
}

#[test]
fn oversized_manifest_is_refused() {
    let json = manifest("ab", "theme", "1.0.0", None) + &" ".repeat(PackageEntryPolicy::MAX_MANIFEST_BYTES as usize);

    assert_eq!(ExtensionInstallError::FileTooLarge, PackageManifestParser::parse(Some(&json)).error);
}

#[test]
fn match_requires_same_id_version_and_kind() {
    let package = PackageManifestParser::parse(Some(&manifest("koyhq.ornek", "theme", "1.0.0", None))).manifest.unwrap();

    assert_eq!(ExtensionInstallError::None, PackageManifestParser::match_item(&package, &default_item()));
    assert_eq!(
        ExtensionInstallError::ManifestMismatch,
        PackageManifestParser::match_item(&package, &item("koyhq.baska", ExtensionKind::Theme, "1.0.0"))
    );
    assert_eq!(
        ExtensionInstallError::ManifestMismatch,
        PackageManifestParser::match_item(&package, &item("koyhq.ornek", ExtensionKind::Theme, "1.0.1"))
    );
    assert_eq!(
        ExtensionInstallError::ManifestMismatch,
        PackageManifestParser::match_item(&package, &item("koyhq.ornek", ExtensionKind::Profile, "1.0.0"))
    );
}

#[test]
fn name_and_author_differences_are_not_a_mismatch() {
    let package = PackageManifestParser::parse(Some(&manifest("koyhq.ornek", "theme", "1.0.0", None))).manifest.unwrap();
    let mut other = default_item();
    other.name = "Baska ad".to_string();
    other.author = "Baska".to_string();

    assert_eq!(ExtensionInstallError::None, PackageManifestParser::match_item(&package, &other));
}

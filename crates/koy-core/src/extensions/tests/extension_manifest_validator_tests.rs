//! Karşılığı: `Extensions/ExtensionManifestValidatorTests.cs`.

use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::extensions::*;

/// C# adlandırılmış isteğe bağlı parametreler: varsayılanlar `my-theme`, `My Theme`, `1.0.0`, `Ada`, yetki yok, `data`.
struct Args<'a> {
    id: Option<&'a str>,
    name: Option<&'a str>,
    version: Option<&'a str>,
    author: Option<&'a str>,
    capabilities: Vec<Option<&'a str>>,
    entry_kind: Option<&'a str>,
}

impl Default for Args<'_> {
    fn default() -> Self {
        Args {
            id: Some("my-theme"),
            name: Some("My Theme"),
            version: Some("1.0.0"),
            author: Some("Ada"),
            capabilities: Vec::new(),
            entry_kind: Some("data"),
        }
    }
}

fn try_create(a: Args) -> Result<ExtensionManifest, ExtensionManifestError> {
    ExtensionManifestValidator::try_create(a.id, a.name, a.version, a.author, a.capabilities, a.entry_kind)
}

fn create(a: Args) -> bool {
    try_create(a).is_ok()
}

fn error_of(a: Args) -> ExtensionManifestError {
    try_create(a).err().unwrap_or(ExtensionManifestError::None)
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn manifest_with(capabilities: Vec<Option<&str>>) -> ExtensionManifest {
    try_create(Args { capabilities, ..Args::default() }).unwrap()
}

#[test]
fn valid_manifest_keeps_fields_as_given() {
    let manifest = ExtensionManifestValidator::try_create(
        Some("my-theme"),
        Some("My Theme"),
        Some("0.2.6"),
        Some("Ada"),
        [Some("network"), Some("clipboard")],
        Some("data"),
    )
    .unwrap();

    assert_eq!("my-theme", manifest.id());
    assert_eq!("My Theme", manifest.name());
    assert_eq!("0.2.6", manifest.version());
    assert_eq!("Ada", manifest.author());
    assert_eq!(&[ExtensionCapability::Network, ExtensionCapability::Clipboard], manifest.capabilities());
    assert_eq!(ExtensionEntryKind::Data, manifest.entry_kind());
}

#[test]
fn failed_create_returns_no_manifest() {
    let result = ExtensionManifestValidator::try_create(Some("A"), Some("x"), Some("1.0.0"), Some("a"), std::iter::empty(), Some("data"));

    assert_eq!(Err(ExtensionManifestError::InvalidId), result);
}

#[test]
fn missing_capabilities_mean_no_capabilities() {
    let manifest = ExtensionManifestValidator::try_create(
        Some("my-theme"),
        Some("My Theme"),
        Some("1.0.0"),
        Some("Ada"),
        std::iter::empty(),
        Some("data"),
    )
    .unwrap();

    assert!(manifest.capabilities().is_empty());
}

#[test]
fn name_with_surrounding_whitespace_is_rejected_not_trimmed() {
    for name in [" My Theme", "My Theme ", "\tMy Theme", "My Theme\n"] {
        assert_eq!(ExtensionManifestError::InvalidName, error_of(Args { name: Some(name), ..Args::default() }), "{name:?}");
    }
}

#[test]
fn author_with_surrounding_whitespace_is_rejected_not_trimmed() {
    for author in [" Ada", "Ada ", "\tAda"] {
        assert_eq!(ExtensionManifestError::InvalidAuthor, error_of(Args { author: Some(author), ..Args::default() }), "{author:?}");
    }
}

#[test]
fn capabilities_read_stops_once_limit_is_exceeded() {
    // Sonsuz bir kaynak: sınır aşılınca okuma durmazsa bu test takılır ya da sayım yanlış olur.
    let pulled = Cell::new(0usize);
    let endless = std::iter::from_fn(|| {
        if pulled.get() < 10_000 {
            pulled.set(pulled.get() + 1);
            Some(Some("network"))
        } else {
            None
        }
    });

    let result = ExtensionManifestValidator::try_create(Some("my-theme"), Some("My Theme"), Some("1.0.0"), Some("Ada"), endless, Some("data"));

    assert_eq!(Err(ExtensionManifestError::TooManyCapabilities), result);
    assert_eq!(ExtensionManifestValidator::MAX_CAPABILITIES + 1, pulled.get());
}

#[test]
fn manifests_with_same_content_are_equal_and_hash_the_same() {
    let first = manifest_with(vec![Some("network"), Some("files")]);
    let second = manifest_with(vec![Some("network"), Some("files")]);

    assert_eq!(first, second);
    assert_eq!(hash_of(&first), hash_of(&second));
}

#[test]
fn manifests_with_different_capabilities_are_not_equal() {
    assert_ne!(manifest_with(vec![Some("network")]), manifest_with(vec![Some("files")]));
}

#[test]
fn valid_ids_are_accepted() {
    for id in ["ab", "my-theme", "a1-b2", "x9", "koyhq.nord-theme", "org.example.breeze", "a.b", "a1.2b", "console.com10"] {
        assert!(create(Args { id: Some(id), ..Args::default() }), "{id}");
    }
}

#[test]
fn invalid_ids_are_rejected() {
    let rows = [
        None,
        Some(""),
        Some("a"),
        Some("Theme"),
        Some("1abc"),
        Some("-abc"),
        Some("my-"),
        Some("my_theme"),
        Some("my theme"),
        Some("tema-ğ"),
        Some(".abc"),
        Some("abc."),
        Some("a..b"),
        Some("a.-b."),
        Some("ab-.cd."),
        Some("a.B"),
        Some("a/b"),
        Some("a:b"),
        Some("a\\b"),
    ];
    for id in rows {
        assert_eq!(ExtensionManifestError::InvalidId, error_of(Args { id, ..Args::default() }), "{id:?}");
    }
}

#[test]
fn id_longer_than_limit_is_rejected_and_limit_itself_is_accepted() {
    let at_limit = format!("a{}", "b".repeat(ExtensionManifestValidator::MAX_ID_LENGTH - 1));
    let over_limit = format!("{at_limit}b");

    assert!(create(Args { id: Some(&at_limit), ..Args::default() }));
    assert_eq!(ExtensionManifestError::InvalidId, error_of(Args { id: Some(&over_limit), ..Args::default() }));
}

#[test]
fn valid_versions_are_accepted() {
    for version in ["1.0.0", "0.2.6", "10.20.300", "0.0.0"] {
        assert!(create(Args { version: Some(version), ..Args::default() }), "{version}");
    }
}

#[test]
fn invalid_versions_are_rejected() {
    let rows = [
        None,
        Some(""),
        Some("1.0"),
        Some("1.0.0.0"),
        Some("v1.0.0"),
        Some("01.0.0"),
        Some("1.0.x"),
        Some("1..0"),
        Some("1.0.-1"),
        Some("1.0.1234567"),
        Some(" 1.0.0"),
    ];
    for version in rows {
        assert_eq!(ExtensionManifestError::InvalidVersion, error_of(Args { version, ..Args::default() }), "{version:?}");
    }
}

#[test]
fn invalid_names_are_rejected() {
    for name in [None, Some(""), Some("   "), Some("bad\tname")] {
        assert_eq!(ExtensionManifestError::InvalidName, error_of(Args { name, ..Args::default() }), "{name:?}");
    }
}

#[test]
fn name_longer_than_limit_is_rejected() {
    let name = "n".repeat(ExtensionManifestValidator::MAX_NAME_LENGTH + 1);

    assert_eq!(ExtensionManifestError::InvalidName, error_of(Args { name: Some(&name), ..Args::default() }));
}

#[test]
fn invalid_authors_are_rejected() {
    for author in [None, Some(""), Some("  ")] {
        assert_eq!(ExtensionManifestError::InvalidAuthor, error_of(Args { author, ..Args::default() }), "{author:?}");
    }
}

#[test]
fn author_longer_than_limit_is_rejected() {
    let author = "a".repeat(ExtensionManifestValidator::MAX_AUTHOR_LENGTH + 1);

    assert_eq!(ExtensionManifestError::InvalidAuthor, error_of(Args { author: Some(&author), ..Args::default() }));
}

#[test]
fn unknown_capabilities_are_rejected() {
    for capability in [Some("shell"), Some("Network"), Some("network "), Some(""), None] {
        assert_eq!(
            ExtensionManifestError::UnknownCapability,
            error_of(Args { capabilities: vec![Some("clipboard"), capability], ..Args::default() }),
            "{capability:?}"
        );
    }
}

#[test]
fn duplicate_capability_is_rejected() {
    assert_eq!(
        ExtensionManifestError::DuplicateCapability,
        error_of(Args { capabilities: vec![Some("files"), Some("files")], ..Args::default() })
    );
}

#[test]
fn count_limit_is_checked_before_capability_names() {
    // Hepsi bilinmeyen olsa bile sayı sınırı önce bakılır: tek tek çözümlemeye gerek kalmaz.
    let too_many = vec![Some("shell"); ExtensionManifestValidator::MAX_CAPABILITIES + 1];

    assert_eq!(ExtensionManifestError::TooManyCapabilities, error_of(Args { capabilities: too_many, ..Args::default() }));
}

#[test]
fn count_at_limit_is_not_reported_as_too_many() {
    // Tam sınırdaki liste sayı denetimini geçer; sonra yetki adı bilinmediği için UnknownCapability döner.
    let at_limit = vec![Some("shell"); ExtensionManifestValidator::MAX_CAPABILITIES];

    assert_eq!(ExtensionManifestError::UnknownCapability, error_of(Args { capabilities: at_limit, ..Args::default() }));
}

#[test]
fn full_closed_set_of_capabilities_is_accepted() {
    assert_eq!(
        ExtensionManifestError::None,
        error_of(Args { capabilities: vec![Some("network"), Some("files"), Some("clipboard")], ..Args::default() })
    );
}

#[test]
fn unknown_entry_kinds_are_rejected() {
    for entry_kind in [Some("exe"), Some("out-of-process"), Some("Data"), None] {
        assert_eq!(ExtensionManifestError::UnknownEntryKind, error_of(Args { entry_kind, ..Args::default() }), "{entry_kind:?}");
    }
}

#[test]
fn first_bad_field_is_reported() {
    // Kimlik ve sürüm birlikte bozuk: kimlik önce bildirilir.
    assert_eq!(ExtensionManifestError::InvalidId, error_of(Args { id: Some("X"), version: Some("bad"), ..Args::default() }));
}

#[test]
fn windows_device_names_are_rejected_as_ids() {
    for id in ["con", "nul", "aux", "prn", "com1", "lpt9", "koyhq.con", "nul.theme", "a.com1.b", "org.lpt3"] {
        assert_eq!(ExtensionManifestError::InvalidId, error_of(Args { id: Some(id), ..Args::default() }), "{id}");
    }
}

#[test]
fn names_that_only_resemble_device_names_are_accepted() {
    for id in ["com0", "com10", "console", "my-con", "koyhq.my-con", "koyhq.com10"] {
        assert!(create(Args { id: Some(id), ..Args::default() }), "{id}");
    }
}

#[test]
fn version_with_trailing_dot_is_rejected() {
    assert_eq!(ExtensionManifestError::InvalidVersion, error_of(Args { version: Some("1.0.0."), ..Args::default() }));
}

#[test]
fn huge_version_is_rejected_by_length() {
    let version = vec!["1"; 10_000].join(".");

    assert_eq!(ExtensionManifestError::InvalidVersion, error_of(Args { version: Some(&version), ..Args::default() }));
}

// 0xD800 ve 0xDC00 (eşleşmemiş üst/alt vekil) satırları Rust'ta ifade edilemez: `&str` geçerli Unicode taşır, eşleşmemiş
// vekil oluşamaz (C#'ta `Rune.TryGetRuneAt` başarısızlığıyla reddedilirdi).
#[test]
fn name_and_author_with_invisible_or_malformed_characters_are_rejected() {
    let code_units: [u32; 4] = [
        0x202E, // sağdan sola geçersiz kılma
        0x200B, // sıfır genişlikli boşluk
        0x2028, // satır ayırıcı
        0xE000, // özel kullanım
    ];
    for code_unit in code_units {
        let text = format!("Ada{}Lovelace", char::from_u32(code_unit).unwrap());

        assert_eq!(ExtensionManifestError::InvalidName, error_of(Args { name: Some(&text), ..Args::default() }), "{code_unit:04X}");
        assert_eq!(ExtensionManifestError::InvalidAuthor, error_of(Args { author: Some(&text), ..Args::default() }), "{code_unit:04X}");
    }
}

#[test]
fn name_with_ordinary_unicode_is_accepted() {
    let text = format!("Çağrı Işık {}", char::from_u32(0x1F600).unwrap());

    assert!(create(Args { name: Some(&text), author: Some(&text), ..Args::default() }));
}

#[test]
fn capabilities_are_sorted_so_order_of_writing_does_not_matter() {
    let first = manifest_with(vec![Some("clipboard"), Some("network")]);
    let second = manifest_with(vec![Some("network"), Some("clipboard")]);

    assert_eq!(first, second);
    assert_eq!(hash_of(&first), hash_of(&second));
    assert_eq!(&[ExtensionCapability::Network, ExtensionCapability::Clipboard], first.capabilities());
}

// C# `Capabilities_cannot_be_mutated_through_a_cast`: dizi dönüştürmesiyle yazılamaması. Rust'ta alan özel ve dışarıya
// yalnız `&[T]` verilir (derleyici yazmayı reddeder); çalışma zamanı testi gerekmez. Burada kopyanın bağımsızlığı sınanır.
#[test]
fn capabilities_are_exposed_read_only_and_manifest_clones_are_independent() {
    let manifest = manifest_with(vec![Some("network")]);
    let copy = manifest.clone();

    assert_eq!(manifest.capabilities(), copy.capabilities());
    assert_eq!(1, manifest.capabilities().len());
}

//! Karşılığı: `Extensions/Store/PackageManifestParser.cs`.

use super::json_document::{self, Json};
use super::store_field_rules::StoreFieldRules;
use super::{ExtensionInstallError, ExtensionKind, ExtensionKinds, PackageEntryPolicy, StoreItem};
use crate::extensions::{ExtensionManifest, ExtensionManifestError, ExtensionManifestValidator};

/// Paketin manifest.json'u: mevcut [`ExtensionManifest`] (kimlik, ad, sürüm, yazar, yetkiler) artı
/// mağazaya özgü `kind` ve `license`. `ExtensionManifest`'e dokunulmaz:
/// o kayıt dışarıdan yalnız doğrulayıcıyla kurulur, bu sarmalayıcı onu içerir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageManifest {
    pub manifest: ExtensionManifest,
    pub kind: ExtensionKind,
    pub license: String,
}

/// `manifest_error`: hata `InvalidManifest` iken doğrulayıcının ilk hatalı alanı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageManifestResult {
    pub error: ExtensionInstallError,
    pub manifest: Option<PackageManifest>,
    pub manifest_error: ExtensionManifestError,
}

impl PackageManifestResult {
    pub fn new(error: ExtensionInstallError, manifest: Option<PackageManifest>, manifest_error: ExtensionManifestError) -> Self {
        PackageManifestResult { error, manifest, manifest_error }
    }

    pub fn is_valid(&self) -> bool {
        self.error == ExtensionInstallError::None && self.manifest.is_some()
    }
}

/// manifest.json okuma ve indeks öğesiyle eşleştirme. Veri paketleri yetki istemez: boş olmayan yetki listesi
/// `ExtensionInstallError::CapabilitiesNotAllowed` ile reddedilir. Asla fırlatmaz.
pub struct PackageManifestParser;

impl PackageManifestParser {
    /// `Parse(ReadOnlySpan<byte> utf8)`.
    pub fn parse_bytes(utf8: &[u8]) -> PackageManifestResult {
        if utf8.len() as i64 > PackageEntryPolicy::MAX_MANIFEST_BYTES {
            return PackageManifestResult::new(ExtensionInstallError::FileTooLarge, None, ExtensionManifestError::None);
        }

        let utf8 = utf8.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(utf8);

        match json_document::parse(utf8, 8) {
            Ok(root) => Self::parse_root(&root),
            Err(()) => Self::invalid(ExtensionManifestError::None),
        }
    }

    /// `Parse(string? json)`.
    pub fn parse(json: Option<&str>) -> PackageManifestResult {
        match json {
            None => Self::invalid(ExtensionManifestError::None),
            Some(json) => Self::parse_bytes(json.as_bytes()),
        }
    }

    fn parse_root(root: &Json) -> PackageManifestResult {
        if !root.is_object() {
            return Self::invalid(ExtensionManifestError::None);
        }

        // Yetki listesi yoksa boş sayılır (doğrulayıcının kuralı); varsa metin dizisi olmalı.
        let mut capabilities: Vec<Option<&str>> = Vec::new();
        if let Some(element) = root.try_get_property("capabilities")
            && !element.is_null()
        {
            let Some(items) = element.as_array() else {
                return Self::invalid(ExtensionManifestError::UnknownCapability);
            };

            for item in items {
                // Sınırı aşan liste doğrulayıcıda reddedilir; fazlası kopyalanmaz.
                if capabilities.len() > ExtensionManifestValidator::MAX_CAPABILITIES {
                    break;
                }

                capabilities.push(item.as_str());
            }
        }

        // entryKind verilmezse "data": bu mağaza yalnız veri paketi alır.
        let entry_kind = if root.try_get_property("entryKind").is_some() { Self::string_of(root, "entryKind") } else { Some("data") };

        let manifest = match ExtensionManifestValidator::try_create(
            Self::string_of(root, "id"),
            Self::string_of(root, "name"),
            Self::string_of(root, "version"),
            Self::string_of(root, "author"),
            capabilities,
            entry_kind,
        ) {
            Ok(manifest) => manifest,
            Err(error) => return Self::invalid(error),
        };

        let Some(kind) = ExtensionKinds::try_parse(Self::string_of(root, "kind")) else {
            return Self::invalid(ExtensionManifestError::None);
        };
        if !StoreFieldRules::is_valid_license(Self::string_of(root, "license")) {
            return Self::invalid(ExtensionManifestError::None);
        }

        if !manifest.capabilities().is_empty() {
            return PackageManifestResult::new(ExtensionInstallError::CapabilitiesNotAllowed, None, ExtensionManifestError::None);
        }

        let license = Self::string_of(root, "license").unwrap_or_default().to_string();
        PackageManifestResult::new(
            ExtensionInstallError::None,
            Some(PackageManifest { manifest, kind, license }),
            ExtensionManifestError::None,
        )
    }

    /// Paketteki kimlik, sürüm ve tür indeksteki öğeyle birebir aynı mı. Ad, yazar ve lisans farkı hata değildir
    /// (indeks özeti elle yazılır), ama kimlik/sürüm/tür farkı başka bir paketin sahte etiketle gelmesidir.
    pub fn match_item(package: &PackageManifest, item: &StoreItem) -> ExtensionInstallError {
        if package.manifest.id() == item.id && package.manifest.version() == item.version && package.kind == item.kind {
            ExtensionInstallError::None
        } else {
            ExtensionInstallError::ManifestMismatch
        }
    }

    fn string_of<'a>(obj: &'a Json, name: &str) -> Option<&'a str> {
        obj.try_get_property(name).and_then(Json::as_str)
    }

    fn invalid(error: ExtensionManifestError) -> PackageManifestResult {
        PackageManifestResult::new(ExtensionInstallError::InvalidManifest, None, error)
    }
}

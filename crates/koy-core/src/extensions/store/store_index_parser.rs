//! Karşılığı: `Extensions/Store/StoreIndexParser.cs`.

use super::iso_time;
use super::json_document::{self, Json};
use super::store_field_rules::StoreFieldRules;
use super::{
    ExtensionKinds, LocalizedText, PackageEntryPolicy, StoreIndex, StoreIndexError, StoreIndexResult, StoreItem, StorePackage, StorePreview,
    StoreUrlPolicy,
};
use crate::extensions::ExtensionManifestValidator;
use crate::text;

/// `store/index.json` ayrıştırıcısı (docs/EXTENSION-STORE.md). Bütün indeks reddedilir: yanlış biçim adı,
/// büyük biçim sürümü, 1 MB üstü, 500 öğe üstü, bozuk JSON. Tek bir öğe bozuksa yalnız o atlanır ve sayılır;
/// aynı kimlik ikinci kez gelirse ikincisi atlanır. Bilinmeyen alanlar yok sayılır. Asla fırlatmaz.
pub struct StoreIndexParser;

impl StoreIndexParser {
    pub const FORMAT_NAME: &'static str = "koy-store-index";
    pub const SUPPORTED_FORMAT_VERSION: i32 = 1;
    pub const MAX_BYTES: usize = 1024 * 1024;
    pub const MAX_ITEMS: usize = 500;

    /// En çok bu kadar dil (kötü niyetli dev nesne ayrıştırılmaz).
    const MAX_LANGUAGES: usize = 32;

    /// `Parse(string? json)`.
    pub fn parse(json: Option<&str>) -> StoreIndexResult {
        let Some(json) = json else {
            return Self::fail(StoreIndexError::NotAStoreIndex);
        };

        // Bir karakter en çok 3 bayttır: dev metin kodlanmadan reddedilir. (Rust `len()` zaten UTF-8 bayt sayısıdır;
        // C#'taki önce `Length` sonra `GetByteCount` denetimi aynı sonucu verir.)
        if json.len() > Self::MAX_BYTES {
            return Self::fail(StoreIndexError::TooLarge);
        }

        Self::parse_bytes(json.as_bytes())
    }

    /// `Parse(ReadOnlySpan<byte> utf8)`.
    pub fn parse_bytes(utf8: &[u8]) -> StoreIndexResult {
        if utf8.len() > Self::MAX_BYTES {
            return Self::fail(StoreIndexError::TooLarge);
        }

        // UTF-8 BOM atlanır (Windows editörleri ekler); JsonDocument kabul etmez.
        let utf8 = utf8.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(utf8);

        match json_document::parse(utf8, 16) {
            Ok(root) => Self::parse_root(&root),
            Err(()) => Self::fail(StoreIndexError::MalformedJson),
        }
    }

    fn parse_root(root: &Json) -> StoreIndexResult {
        if !root.is_object() {
            return Self::fail(StoreIndexError::NotAStoreIndex);
        }

        if Self::try_string(root, "format") != Some(Self::FORMAT_NAME) {
            return Self::fail(StoreIndexError::NotAStoreIndex);
        }

        let format_version = match root.try_get_property("formatVersion") {
            Some(element) if element.is_number() => element.try_get_i32(),
            _ => None,
        };
        let Some(format_version) = format_version.filter(|&v| v >= 1) else {
            return Self::fail(StoreIndexError::NotAStoreIndex);
        };

        if format_version > Self::SUPPORTED_FORMAT_VERSION {
            return Self::fail(StoreIndexError::UnsupportedFormatVersion);
        }

        let Some(items_element) = root.try_get_property("items").and_then(Json::as_array) else {
            return Self::fail(StoreIndexError::NotAStoreIndex);
        };

        if items_element.len() > Self::MAX_ITEMS {
            return Self::fail(StoreIndexError::TooManyItems);
        }

        let generated = Self::try_string(root, "generated").and_then(iso_time::parse_date_time_offset_assume_universal);

        let mut items: Vec<StoreItem> = Vec::with_capacity(items_element.len());
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut skipped = 0;
        for element in items_element {
            match Self::try_parse_item(element) {
                Some(item) if seen.insert(item.id.clone()) => items.push(item),
                _ => skipped += 1,
            }
        }

        StoreIndexResult::new(StoreIndexError::None, Some(StoreIndex::new(generated, items)), skipped)
    }

    fn try_parse_item(element: &Json) -> Option<StoreItem> {
        if !element.is_object() {
            return None;
        }

        let id = Self::try_string(element, "id").filter(|id| ExtensionManifestValidator::is_valid_id(Some(id)))?;
        let name = Self::try_string(element, "name")
            .filter(|name| ExtensionManifestValidator::is_valid_text(Some(name), ExtensionManifestValidator::MAX_NAME_LENGTH))?;
        let kind = Self::try_string(element, "kind").and_then(|k| ExtensionKinds::try_parse(Some(k)))?;
        let version = Self::try_string(element, "version").filter(|v| ExtensionManifestValidator::is_valid_version(Some(v)))?;
        let author = Self::try_string(element, "author")
            .filter(|author| ExtensionManifestValidator::is_valid_text(Some(author), ExtensionManifestValidator::MAX_AUTHOR_LENGTH))?;
        let license = Self::try_string(element, "license").filter(|l| StoreFieldRules::is_valid_license(Some(l)))?;
        let summary_element = element.try_get_property("summary")?;
        let summary = Self::try_localized(summary_element, StoreFieldRules::MAX_SUMMARY_LENGTH, false, false)?;
        let updated = Self::try_string(element, "updated").and_then(iso_time::parse_date_only_exact)?;

        // Uzun açıklama isteğe bağlı; var ama bozuksa öğe bozuktur.
        let mut description = LocalizedText::empty();
        if let Some(description_element) = element.try_get_property("description") {
            description = Self::try_localized(description_element, StoreFieldRules::MAX_DESCRIPTION_LENGTH, true, true)?;
        }

        // Dolu ama https olmayan adres öğeyi bozuk yapar (tıklanabilir bağlantı olacak).
        let mut homepage: Option<String> = None;
        if let Some(homepage_element) = element.try_get_property("homepage")
            && !homepage_element.is_null()
        {
            let text = Self::try_string(element, "homepage").filter(|h| StoreFieldRules::is_https_url(Some(h)))?;
            homepage = Some(text.to_string());
        }

        let mut downloads: i64 = 0;
        if let Some(downloads_element) = element.try_get_property("downloads")
            && !downloads_element.is_null()
        {
            if !downloads_element.is_number() {
                return None;
            }

            downloads = downloads_element.try_get_i64().filter(|&d| d >= 0)?;
        }

        let package_element = element.try_get_property("package").filter(|p| p.is_object())?;
        let package_url = Self::try_string(package_element, "url").filter(|u| StoreUrlPolicy::is_trusted_asset_url(Some(u)))?;
        let package_hash = Self::try_string(package_element, "sha256").filter(|h| StoreFieldRules::is_sha256(Some(h)))?;
        let size_element = package_element.try_get_property("size").filter(|s| s.is_number())?;
        let size = size_element.try_get_i64().filter(|&s| s > 0 && s <= PackageEntryPolicy::MAX_DOWNLOAD_BYTES)?;

        // Önizleme isteğe bağlı: bozuksa öğe değil yalnız önizleme düşer (güvensiz adres hiç kullanılmaz).
        let mut preview: Option<StorePreview> = None;
        if let Some(preview_element) = element.try_get_property("preview")
            && preview_element.is_object()
            && let Some(preview_url) = Self::try_string(preview_element, "url").filter(|u| StoreUrlPolicy::is_trusted_asset_url(Some(u)))
            && let Some(preview_hash) = Self::try_string(preview_element, "sha256").filter(|h| StoreFieldRules::is_sha256(Some(h)))
        {
            preview = Some(StorePreview { url: preview_url.to_string(), sha256: text::to_lower_invariant(preview_hash) });
        }

        Some(StoreItem {
            id: id.to_string(),
            name: name.to_string(),
            kind,
            version: version.to_string(),
            author: author.to_string(),
            license: license.to_string(),
            summary,
            description,
            homepage,
            updated,
            downloads,
            package: StorePackage { url: package_url.to_string(), sha256: text::to_lower_invariant(package_hash), size },
            preview,
        })
    }

    // Düz metin ya da { "dil": "metin" } nesnesi. Nesne biçiminde "en" zorunlu; dil kodu iki küçük harf; her değer
    // alanın düz metin kuralından geçer. Yinelenen dil ya da boş nesne bozuktur.
    fn try_localized(element: &Json, max_length: usize, allow_newlines: bool, allow_empty: bool) -> Option<LocalizedText> {
        if let Some(plain) = element.as_str() {
            if !StoreFieldRules::is_plain_text(Some(plain), max_length, allow_newlines, allow_empty) {
                return None;
            }

            return Some(LocalizedText::plain(plain));
        }

        let properties = element.as_object()?;

        let mut values: Vec<(String, String)> = Vec::new();
        for (code, value) in properties {
            let mut code_chars = code.chars();
            let two_lower_letters = match (code_chars.next(), code_chars.next(), code_chars.next()) {
                (Some(a), Some(b), None) => a.is_ascii_lowercase() && b.is_ascii_lowercase(),
                _ => false,
            };
            if values.len() >= Self::MAX_LANGUAGES || !two_lower_letters {
                return None;
            }

            let value = value.as_str()?;
            if !StoreFieldRules::is_plain_text(Some(value), max_length, allow_newlines, allow_empty) {
                return None;
            }

            // `TryAdd`: yinelenen dil bozuk.
            if values.iter().any(|(k, _)| k == code) {
                return None;
            }
            values.push((code.clone(), value.to_string()));
        }

        if !values.iter().any(|(k, _)| k == LocalizedText::FALLBACK_LANGUAGE) {
            return None;
        }

        Some(LocalizedText::new(values))
    }

    // Alan yoksa ya da metin değilse None (null/sayı bozuk sayılır, sessizce metne çevrilmez).
    fn try_string<'a>(obj: &'a Json, name: &str) -> Option<&'a str> {
        obj.try_get_property(name).and_then(Json::as_str)
    }

    fn fail(error: StoreIndexError) -> StoreIndexResult {
        StoreIndexResult::new(error, None, 0)
    }
}

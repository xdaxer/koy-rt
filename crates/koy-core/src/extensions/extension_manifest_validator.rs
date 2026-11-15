//! Karşılığı: `Extensions/ExtensionManifestValidator.cs`.

use serde::{Deserialize, Serialize};

use super::unicode_category::is_disallowed_in_display_text;
use super::{ExtensionCapability, ExtensionEntryKind, ExtensionManifest};

/// Manifest doğrulamasının ilk hatalı alanını söyler. Hata yoksa `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ExtensionManifestError {
    #[default]
    None,
    InvalidId,
    InvalidName,
    InvalidVersion,
    InvalidAuthor,
    TooManyCapabilities,
    UnknownCapability,
    DuplicateCapability,
    UnknownEntryKind,
}

/// Dışarıdan gelen ham değerleri (ör. JSON'dan okunmuş metinler) sıkı kurallarla doğrular.
/// Bilinmeyen yetki ya da tür, geçersiz kimlik ve sürüm, sınırları aşan metin reddedilir; sessizce düzeltilmez.
/// Ad ve yazarın baştaki ya da sondaki boşluğu da reddedilir, kırpılmaz.
pub struct ExtensionManifestValidator;

impl ExtensionManifestValidator {
    pub const MIN_ID_LENGTH: usize = 2;
    pub const MAX_ID_LENGTH: usize = 64;
    pub const MAX_NAME_LENGTH: usize = 80;
    pub const MAX_AUTHOR_LENGTH: usize = 80;

    // Sürüm parçası en çok 6 hane: "999999.999999.999999" gibi aşırı değerleri ve taşmayı engeller.
    pub const MAX_VERSION_PART_DIGITS: usize = 6;

    // Üç parça ve iki nokta: en uzun geçerli sürümün uzunluğu.
    const MAX_VERSION_LENGTH: usize = (3 * Self::MAX_VERSION_PART_DIGITS) + 2;

    // Yetki sayısı kapalı kümenin büyüklüğünden çok daha yüksekse, şişirilmiş bir manifesttir.
    pub const MAX_CAPABILITIES: usize = 16;

    /// Ham değerleri doğrular. Başarılıysa manifest döner, değilse `Err` ilk hatalı alanı söyler
    /// (C#'taki `out manifest` / `out error` çifti; hata değeri `None` hiç dönmez).
    /// Yetki alanı hiç verilmezse (C# `null`) bu açık bir kuraldır: boş liste, yani hiç yetki istemeyen
    /// eklenti. Bu bir düzeltme değildir; eksik alan ile boş liste aynı anlama gelir. Bu yüzden C# `null`
    /// için boş bir yineleyici (`std::iter::empty()`) verilir. Yineleyici tembel okunur: sınır aşılınca durur.
    pub fn try_create<'a>(
        id: Option<&str>,
        name: Option<&str>,
        version: Option<&str>,
        author: Option<&str>,
        capabilities: impl IntoIterator<Item = Option<&'a str>>,
        entry_kind: Option<&str>,
    ) -> Result<ExtensionManifest, ExtensionManifestError> {
        if !Self::is_valid_id(id) {
            return Err(ExtensionManifestError::InvalidId);
        }

        if !Self::is_valid_text(name, Self::MAX_NAME_LENGTH) {
            return Err(ExtensionManifestError::InvalidName);
        }

        if !Self::is_valid_version(version) {
            return Err(ExtensionManifestError::InvalidVersion);
        }

        if !Self::is_valid_text(author, Self::MAX_AUTHOR_LENGTH) {
            return Err(ExtensionManifestError::InvalidAuthor);
        }

        // Sayı en fazla MaxCapabilities + 1 öğeye kadar okunur: sınır aşılınca numaralandırma durur ve
        // fazlası hiç kopyalanmaz. Sayı denetimi, yetki adlarının çözümlemesinden önce yapılır.
        let mut list: Vec<Option<&str>> = Vec::new();
        for value in capabilities {
            if list.len() == Self::MAX_CAPABILITIES {
                return Err(ExtensionManifestError::TooManyCapabilities);
            }

            list.push(value);
        }

        let mut parsed: Vec<ExtensionCapability> = Vec::with_capacity(list.len());
        for value in list {
            let Some(capability) = Self::try_parse_capability(value) else {
                return Err(ExtensionManifestError::UnknownCapability);
            };

            // Aynı yetki iki kez yazılırsa kullanıcıya gösterilen liste yanıltıcı olur.
            if parsed.contains(&capability) {
                return Err(ExtensionManifestError::DuplicateCapability);
            }

            parsed.push(capability);
        }

        let Some(kind) = Self::try_parse_entry_kind(entry_kind) else {
            return Err(ExtensionManifestError::UnknownEntryKind);
        };

        Ok(ExtensionManifest::new(
            id.unwrap_or_default().to_string(),
            name.unwrap_or_default().to_string(),
            version.unwrap_or_default().to_string(),
            author.unwrap_or_default().to_string(),
            parsed,
            kind,
        ))
    }

    /// Kimlik: küçük harf ile başlar; yalnız küçük harf, rakam, tire ve nokta (ters alan adı biçimi,
    /// "koyhq.nord-theme"). Tireyle ya da noktayla bitmez, art arda iki nokta ve boş parça yok. Uzunluk 2 ile 64
    /// arasında. Büyük harf ya da boşluk kabul edilmez, çünkü kimlik dosya adı ve karşılaştırma anahtarı olarak
    /// kullanılır. Her nokta-ayrılmış parça için Windows aygıt adları (con, nul, com1…) reddedilir: "con.x" da
    /// klasör olarak aygıt adına çözülür.
    pub(crate) fn is_valid_id(id: Option<&str>) -> bool {
        let Some(id) = id else {
            return false;
        };

        // C# `Length`: UTF-16 birimi.
        let length = id.encode_utf16().count();
        if !(Self::MIN_ID_LENGTH..=Self::MAX_ID_LENGTH).contains(&length) {
            return false;
        }

        if !id.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
            return false;
        }

        for c in id.chars() {
            if !c.is_ascii_lowercase() && !c.is_ascii_digit() && !matches!(c, '-' | '.') {
                return false;
            }
        }

        if id.ends_with(['-', '.']) {
            return false;
        }

        for part in id.split('.') {
            if part.is_empty() || Self::is_reserved_device_name(part) {
                return false;
            }
        }

        true
    }

    // Kimlik dosya adı olarak kullanılacak; Windows'ta bu adlar dosya değil aygıt açar (con, nul, com1…).
    // Yalnız karakter denetiminden geçmiş (ASCII) kimliklerle çağrılır.
    fn is_reserved_device_name(id: &str) -> bool {
        if matches!(id, "con" | "prn" | "aux" | "nul") {
            return true;
        }

        let bytes = id.as_bytes();
        bytes.len() == 4 && (id.starts_with("com") || id.starts_with("lpt")) && (b'1'..=b'9').contains(&bytes[3])
    }

    /// Sürüm tam olarak üç parçalı "major.minor.patch" biçiminde olmalı; her parça yalnız rakam içerir.
    /// Uzunluk önce denetlenir: aşırı uzun bir metin bölünüp parçalara kopyalanmaz.
    pub(crate) fn is_valid_version(version: Option<&str>) -> bool {
        let Some(version) = version else {
            return false;
        };

        if version.encode_utf16().count() > Self::MAX_VERSION_LENGTH {
            return false;
        }

        let mut parts = 0;
        for part in version.split('.') {
            parts += 1;
            if parts > 3 {
                return false;
            }

            let length = part.encode_utf16().count();
            if !(1..=Self::MAX_VERSION_PART_DIGITS).contains(&length) {
                return false;
            }

            if !part.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }

            // "01" gibi baştaki sıfırlar iki farklı yazımın aynı sürüm sayılmasına yol açar.
            if length > 1 && part.starts_with('0') {
                return false;
            }
        }

        parts == 3
    }

    /// Ad ve yazar: boş olmayan, sınırı aşmayan, denetim karakteri içermeyen ve baştaki ya da sondaki
    /// boşluk taşımayan metin. Boşluk sessizce kırpılmaz; böyle bir değer reddedilir.
    pub(crate) fn is_valid_text(value: Option<&str>, max_length: usize) -> bool {
        let Some(value) = value else {
            return false;
        };

        // C# `Length`: UTF-16 birimi.
        if value.is_empty() || value.encode_utf16().count() > max_length {
            return false;
        }

        if value.chars().next().is_some_and(char::is_whitespace) || value.chars().next_back().is_some_and(char::is_whitespace) {
            return false;
        }

        // Kullanıcıya onay ekranında gösterilir: görünmez ya da yön değiştiren karakterler (U+202E, U+200B,
        // U+2028), özel kullanım ve eşleşmemiş vekil karakterler başka bir adı taklit etmek için kullanılabilir.
        // (Eşleşmemiş vekil Rust `&str`'da zaten temsil edilemez.)
        !value.chars().any(is_disallowed_in_display_text)
    }

    // Eşleşme büyük/küçük harfe duyarlı: "Network" ya da "network " kabul edilmez.
    fn try_parse_capability(value: Option<&str>) -> Option<ExtensionCapability> {
        match value {
            Some("network") => Some(ExtensionCapability::Network),
            Some("files") => Some(ExtensionCapability::Files),
            Some("clipboard") => Some(ExtensionCapability::Clipboard),
            _ => None,
        }
    }

    fn try_parse_entry_kind(value: Option<&str>) -> Option<ExtensionEntryKind> {
        if value == Some("data") { Some(ExtensionEntryKind::Data) } else { None }
    }
}

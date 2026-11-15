//! Karşılığı: `Extensions/Store/StoreFieldRules.cs` (C#'ta `internal`).

use super::uri::Uri;
use crate::extensions::unicode_category::is_disallowed_in_display_text;

/// İndeks ve paket manifestinin ortak alan kuralları (saf, fırlatmaz).
pub(crate) struct StoreFieldRules;

impl StoreFieldRules {
    pub const MAX_LICENSE_LENGTH: usize = 64;
    pub const MAX_SUMMARY_LENGTH: usize = 200;
    pub const MAX_DESCRIPTION_LENGTH: usize = 2000;
    pub const MAX_URL_LENGTH: usize = 512;

    /// SPDX ifadesi için yeterince geniş küme: harf, rakam ve ". - + ( ) :" ile boşluk ("MIT OR Apache-2.0").
    /// Baştaki/sondaki boşluk ve boş metin yok.
    pub fn is_valid_license(value: Option<&str>) -> bool {
        let Some(value) = value else {
            return false;
        };

        // C# `Length`: UTF-16 birimi.
        if value.is_empty() || value.encode_utf16().count() > Self::MAX_LICENSE_LENGTH {
            return false;
        }

        if value.starts_with(' ') || value.ends_with(' ') {
            return false;
        }

        value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '(' | ')' | ':' | ' '))
    }

    /// Yalnız https, mutlak, kullanıcı bilgisiz, boşluk/denetim karakterisiz adres.
    pub fn is_https_url(value: Option<&str>) -> bool {
        let Some(value) = value else {
            return false;
        };

        if value.is_empty() || value.encode_utf16().count() > Self::MAX_URL_LENGTH {
            return false;
        }

        if value.chars().any(|c| c <= ' ' || c == '\u{007f}' || c == '\\') {
            return false;
        }

        Uri::try_create(value).is_some_and(|uri| uri.scheme() == "https" && !uri.host().is_empty() && uri.user_info().is_empty())
    }

    pub fn is_sha256(value: Option<&str>) -> bool {
        // C# `Length != 64`: UTF-16 birimi; hex dışı karakter zaten reddedilir.
        value.is_some_and(|v| v.encode_utf16().count() == 64 && v.chars().all(|c| c.is_ascii_hexdigit()))
    }

    /// Düz metin: uzunluk sınırında, eşleşmemiş vekil ve yön/biçim karakteri yok. Denetim karakterinden yalnız
    /// `allow_newlines` açıkken satır sonu ve sekme serbest.
    pub fn is_plain_text(value: Option<&str>, max_length: usize, allow_newlines: bool, allow_empty: bool) -> bool {
        let Some(value) = value else {
            return false;
        };

        if value.encode_utf16().count() > max_length || (!allow_empty && value.trim().is_empty()) {
            return false;
        }

        for c in value.chars() {
            if allow_newlines && matches!(c, '\n' | '\r' | '\t') {
                continue;
            }

            // Eşleşmemiş vekil Rust `&str`'da temsil edilemez; kalan kategoriler tek tek denetlenir.
            if is_disallowed_in_display_text(c) {
                return false;
            }
        }

        true
    }
}

//! Karşılığı: `Extensions/Store/StoreUrlPolicy.cs`.

use super::store_field_rules::StoreFieldRules;
use super::uri::Uri;
use crate::text;

/// İndirilecek varlıkların (paket, önizleme) güvenilir adres kümesi: yalnız https, `ALLOWED_HOST` üzerinde ve
/// `ALLOWED_PATH_PREFIX` yolunun altında. Mağaza deposu dışına, ele geçirilmiş bir indeksle bile indirme yapılmaz.
/// Aynı kural scripts/build-store.py içinde de uygulanır (`--check`). Ana sayfa bağlantısı bu kuralın dışındadır
/// (tarayıcıda açılır, indirilmez).
pub struct StoreUrlPolicy;

impl StoreUrlPolicy {
    pub const ALLOWED_HOST: &'static str = "raw.githubusercontent.com";

    pub const ALLOWED_PATH_PREFIX: &'static str = "/koyhq/koy/";

    /// İndeksteki ham adres güvenilir mi (biçim kuralları `StoreFieldRules::is_https_url` ile aynı).
    pub fn is_trusted_asset_url(value: Option<&str>) -> bool {
        StoreFieldRules::is_https_url(value)
            && value.and_then(Uri::try_create).is_some_and(|uri| Self::is_trusted_asset_uri(Some(&uri)))
    }

    /// Adres güvenilir mi. İndirme sonrası son yanıtın adresi de buradan geçer: yönlendirme başka bir sunucuya
    /// götürdüyse içerik kabul edilmez.
    pub fn is_trusted_asset_uri(uri: Option<&Uri>) -> bool {
        uri.is_some_and(|uri| {
            uri.is_absolute_uri()
                && uri.scheme() == "https"
                && uri.is_default_port()
                && uri.user_info().is_empty()
                && text::eq_ignore_case(uri.host(), Self::ALLOWED_HOST)
                && uri.absolute_path().starts_with(Self::ALLOWED_PATH_PREFIX)
                && !uri.absolute_path().contains("..")
        })
    }
}

//! Karşılığı: `Weather/PlaceNameParser.cs`.

use serde_json::Value;

use crate::text;

/// Şehir düzeyinden geniş düzeye: ilk dolu olan yer adıdır.
const ADDRESS_KEYS: [&str; 7] = ["city", "town", "village", "municipality", "county", "province", "state"];

/// Cihaz konumundan yer adı (ters konumlama, OpenStreetMap Nominatim `reverse` yanıtı).
/// Yalnız şehir/ilçe düzeyinde ad seçilir; sokak ve mahalle hiç kullanılmaz.
pub struct PlaceNameParser;

impl PlaceNameParser {
    /// Yanıttaki yer adı; ayrıştırılamazsa ya da ad yoksa `None`.
    pub fn parse(json: Option<&str>) -> Option<String> {
        if text::is_null_or_white_space(json) {
            return None;
        }

        let doc: Value = serde_json::from_str(json?).ok()?;
        let address = doc.as_object()?.get("address")?.as_object()?;

        for key in ADDRESS_KEYS {
            if let Some(Value::String(value)) = address.get(key) {
                let name = text::trim(value);
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        None
    }

    /// Sorguda gönderilen koordinat: iki ondalık (~1 km). Şehir adı için yeter, tam konum servise gitmez;
    /// aynı zamanda önbellek anahtarıdır. (`MidpointRounding.AwayFromZero`)
    pub fn round(latitude: f64, longitude: f64) -> (f64, f64) {
        (text::round_digits_away(latitude, 2), text::round_digits_away(longitude, 2))
    }

    /// Nominatim reverse sorgu adresi (zoom 10: şehir düzeyi).
    pub fn build_url(latitude: f64, longitude: f64, language: &str) -> String {
        let (lat, lon) = Self::round(latitude, longitude);
        let lang = if text::eq_ignore_case(language, "en") { "en" } else { "tr" };
        format!(
            "https://nominatim.openstreetmap.org/reverse?format=jsonv2&zoom=10&lat={lat:.2}&lon={lon:.2}&accept-language={lang}"
        )
    }
}

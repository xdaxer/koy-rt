//! Open-Meteo geocoding: şehir adından koordinat. Anahtar gerekmez; yalnız kullanıcı arama yapınca çağrılır.

use koy_core::calendar::escape_data_string;
use koy_core::cancel::{CancellationToken, OperationCanceled};
use koy_core::text;
use koy_core::weather::{GeoPlace, IGeocodingService, OpenMeteoParser};

use crate::net::HttpClient;

pub struct GeocodingService {
    http: &'static HttpClient,
}

impl Default for GeocodingService {
    fn default() -> Self {
        Self::new(None)
    }
}

impl GeocodingService {
    pub fn new(http: Option<&'static HttpClient>) -> Self {
        GeocodingService { http: http.unwrap_or_else(HttpClient::shared) }
    }

    fn fetch(&self, name: &str, language: &str, ct: &CancellationToken) -> Option<Vec<GeoPlace>> {
        let lang = if text::eq_ignore_case(language, "en") { "en" } else { "tr" };
        let url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count={}&language={lang}",
            escape_data_string(name),
            OpenMeteoParser::MAX_RESULTS
        );
        let mut response = self.http.get(&url, ct).ok()?;
        if !response.is_success() {
            return Some(Vec::new());
        }
        let body = response.text().ok()?;
        Some(OpenMeteoParser::parse_geocoding(Some(&body)))
    }
}

impl IGeocodingService for GeocodingService {
    fn search(&self, query: &str, language: &str, ct: &CancellationToken) -> Result<Vec<GeoPlace>, OperationCanceled> {
        let name = query.trim();
        // C#: name.Length (UTF-16 birimi).
        if name.encode_utf16().count() < OpenMeteoParser::MIN_QUERY_LENGTH {
            return Ok(Vec::new());
        }
        // Ağ yok, zaman aşımı ya da iptal: boş sonuç; arayüz "bulunamadı" der, çökmez (C#'ta iptal de yutulur).
        Ok(self.fetch(name, language, ct).unwrap_or_default())
    }
}

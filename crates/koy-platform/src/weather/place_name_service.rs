//! OpenStreetMap Nominatim ile cihaz konumunun yer adı. Anahtar gerekmez; koordinat ~1 km'ye yuvarlanarak gönderilir.
//! Yalnız çentik açılıp cihaz konumu kullanılırken çağrılır; aynı koordinat için oturumda tek istek atılır
//! (Nominatim kullanım koşulu: saniyede en çok bir istek, tanıtıcı User-Agent; HttpClient bunu taşır).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use koy_core::cancel::{CancellationToken, OperationCanceled};
use koy_core::weather::{IPlaceNameService, OpenMeteoParser, PlaceNameParser};

use crate::net::HttpClient;

pub struct PlaceNameService {
    http: &'static HttpClient,
    // (anahtar, sonuç). Kilit istek boyunca tutulur: aynı anahtar için süren istek paylaşılır (C#'taki `_pending`).
    cache: Mutex<Option<(String, Option<String>)>>,
}

impl Default for PlaceNameService {
    fn default() -> Self {
        Self::new(None)
    }
}

impl PlaceNameService {
    pub fn new(http: Option<&'static HttpClient>) -> Self {
        PlaceNameService { http: http.unwrap_or_else(HttpClient::shared), cache: Mutex::new(None) }
    }

    fn fetch(&self, latitude: f64, longitude: f64, language: &str) -> Option<String> {
        // C#: istek çağıranın iptalinden bağımsız, kendi 10 sn süresiyle yürür (sonuç paylaşıldığı için).
        let deadline = Instant::now() + Duration::from_secs(10);
        let url = PlaceNameParser::build_url(latitude, longitude, language);
        let mut response = self.http.get(&url, &CancellationToken::none()).ok()?;
        if !response.is_success() {
            return None;
        }
        let body = response.read_to_end_limited(None, Some(deadline), None).ok()?;
        PlaceNameParser::parse(Some(&String::from_utf8_lossy(&body)))
    }
}

impl IPlaceNameService for PlaceNameService {
    fn get_place_name(&self, latitude: f64, longitude: f64, language: &str, _ct: &CancellationToken) -> Result<Option<String>, OperationCanceled> {
        if !OpenMeteoParser::is_valid_coordinate(latitude, longitude) {
            return Ok(None);
        }
        let (lat, lon) = PlaceNameParser::round(latitude, longitude);
        let key = format!("{lat:.2},{lon:.2},{language}");

        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        // Başarısız (None) sonuç da tutulur: ağ yoksa her çentik açılışında yeniden denenmesin; koordinat değişince
        // yeniden sorulur.
        if let Some((k, result)) = cache.as_ref()
            && *k == key
        {
            return Ok(result.clone());
        }
        let result = self.fetch(latitude, longitude, language);
        *cache = Some((key, result.clone()));
        Ok(result)
    }
}

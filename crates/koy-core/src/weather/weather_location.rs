//! Karşılığı: `Weather/WeatherLocation.cs`.

use serde::{Deserialize, Serialize};

use crate::cancel::{CancellationToken, OperationCanceled};
use crate::text;

/// Hava durumunun koordinatı nereden geldi. `None`: konum yok, hava durumu hiç gösterilmez.
/// Adlar settings.json'a metin olarak yazılır (`UseStringEnumConverter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WeatherLocationSource {
    #[default]
    None,
    Device,
    City,
}

/// Şehir aramasından (Open-Meteo geocoding) dönen bir yer.
#[derive(Debug, Clone, PartialEq)]
pub struct GeoPlace {
    pub name: String,
    pub region: Option<String>,
    pub country: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

impl GeoPlace {
    pub fn new(name: impl Into<String>, region: Option<String>, country: Option<String>, latitude: f64, longitude: f64) -> Self {
        GeoPlace { name: name.into(), region, country, latitude, longitude }
    }

    /// Listede gösterilen ad: "Ankara, Ankara, Türkiye" gibi tekrarları ayıklanmış biçim.
    pub fn display_name(&self) -> String {
        let mut parts: Vec<&str> = vec![&self.name];
        if let Some(region) = &self.region {
            if !text::is_null_or_white_space(Some(region)) && !text::eq_ignore_case(region, &self.name) {
                parts.push(region);
            }
        }
        if let Some(country) = &self.country {
            if !text::is_null_or_white_space(Some(country)) {
                parts.push(country);
            }
        }
        parts.join(", ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LocationStatus {
    #[default]
    Success,
    /// Kullanıcı ya da sistem (Windows gizlilik ayarı) konuma izin vermedi.
    Denied,
    /// Konum sensörü ya da konum hizmeti yok / kapalı.
    Unavailable,
    TimedOut,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocationResult {
    pub status: LocationStatus,
    pub latitude: f64,
    pub longitude: f64,
}

impl LocationResult {
    /// C#: `new LocationResult(status, latitude = 0, longitude = 0)`.
    pub fn new(status: LocationStatus, latitude: f64, longitude: f64) -> Self {
        LocationResult { status, latitude, longitude }
    }

    /// Koordinatsız sonuç (`latitude`/`longitude` 0).
    pub fn of(status: LocationStatus) -> Self {
        Self::new(status, 0.0, 0.0)
    }

    pub fn is_success(&self) -> bool {
        self.status == LocationStatus::Success
    }
}

/// Cihaz konumu sözleşmesi. Tek seferlik, düşük doğruluklu sorgu; izin reddi, zaman aşımı ve hatalar
/// istisna fırlatmaz, durum olarak döner. (C#: `GetLocationAsync`; burada bloklayan çağrı.)
pub trait ILocationService: Send + Sync {
    fn get_location(&self, cancellation_token: &CancellationToken) -> Result<LocationResult, OperationCanceled>;
}

/// Koordinattan yer adı (ters konumlama). Ağ hatasında `None` döner, istisna fırlatmaz. (C#: `GetPlaceNameAsync`.)
pub trait IPlaceNameService: Send + Sync {
    fn get_place_name(&self, latitude: f64, longitude: f64, language: &str, cancellation_token: &CancellationToken) -> Result<Option<String>, OperationCanceled>;
}

/// Şehir arama sözleşmesi (Open-Meteo geocoding).
pub trait IGeocodingService: Send + Sync {
    /// Ağ hatasında boş liste döner, istisna fırlatmaz. (C#: `SearchAsync`.)
    fn search(&self, query: &str, language: &str, cancellation_token: &CancellationToken) -> Result<Vec<GeoPlace>, OperationCanceled>;
}

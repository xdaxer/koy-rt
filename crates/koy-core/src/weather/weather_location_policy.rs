//! Karşılığı: `Weather/WeatherLocationPolicy.cs`.

use super::open_meteo_parser::OpenMeteoParser;
use super::weather_location::{GeoPlace, LocationResult, WeatherLocationSource};
use crate::settings::AppSettings;
use crate::time::{DateTimeOffset, TimeSpan, span_between};

/// Hava durumunun hangi koordinatla gösterileceğine ve cihaz konumunun ne zaman yenileneceğine karar verir.
/// Hepsi saf: ayarlar ve zaman girer, yeni ayarlar çıkar; kayıt ve servis çağrıları çağıranın işi.
/// (C# `settings with { ... }` → yeni `AppSettings` döner, girdi değişmez.)
pub struct WeatherLocationPolicy;

impl WeatherLocationPolicy {
    /// Cihaz konumu en sık bu aralıkla yenilenir (yalnız uygulama açılışında bakılır).
    pub const DEVICE_REFRESH_INTERVAL: TimeSpan = TimeSpan::from_hours(24);

    /// Hava durumu gösterilebilir mi: kaynak var ve koordinat geçerli (0,0 "boş" sayılır).
    pub fn has_location(settings: &AppSettings) -> bool {
        let w = &settings.weather;
        w.location_source != WeatherLocationSource::None
            && OpenMeteoParser::is_valid_coordinate(w.latitude, w.longitude)
            && !(w.latitude == 0.0 && w.longitude == 0.0)
    }

    /// Açılışta cihaz konumu sorulsun mu: kullanıcı izin verdiyse ve koordinat henüz cihazdan alınmamışsa ya da
    /// son alınışının üstünden 24 saat geçtiyse. Saat geriye alınmışsa (gelecekte görünen kayıt) da yenilenir.
    pub fn should_refresh_device_location(settings: &AppSettings, now: DateTimeOffset) -> bool {
        if !settings.weather.use_location {
            return false;
        }

        if settings.weather.location_source != WeatherLocationSource::Device || !Self::has_location(settings) {
            return true;
        }

        let updated = from_unix_time_seconds(settings.weather.location_updated_unix.max(0));
        let age = span_between(now, updated);
        age < TimeSpan::ZERO || age >= Self::DEVICE_REFRESH_INTERVAL
    }

    /// Cihaz konumu alındıysa ayarları günceller; hata, ret ya da zaman aşımında olduğu gibi döner.
    pub fn apply_device_location(settings: &AppSettings, result: &LocationResult, now: DateTimeOffset) -> AppSettings {
        if !result.is_success() || !OpenMeteoParser::is_valid_coordinate(result.latitude, result.longitude) {
            return settings.clone();
        }

        let mut s = settings.clone();
        s.weather.location_source = WeatherLocationSource::Device;
        s.weather.latitude = result.latitude;
        s.weather.longitude = result.longitude;
        s.weather.city = String::new();
        s.weather.location_updated_unix = now.timestamp();
        s
    }

    /// Şehir seçildi: cihaz konumu kullanımı kapanır (açık bir seçim, konumdan önce gelir).
    pub fn apply_city(settings: &AppSettings, place: &GeoPlace, now: DateTimeOffset) -> AppSettings {
        let mut s = settings.clone();
        s.weather.location_source = WeatherLocationSource::City;
        s.weather.use_location = false;
        s.weather.latitude = place.latitude;
        s.weather.longitude = place.longitude;
        s.weather.city = place.display_name();
        s.weather.location_updated_unix = now.timestamp();
        s
    }

    /// "Konumu kullan" anahtarı. Açılınca bir sonraki yenilemede cihaz konumu sorulur (seçili şehir, konum gelene
    /// kadar yedek olarak kalır). Kapanınca cihazdan gelen koordinat silinir; seçili şehir korunur.
    pub fn set_use_location(settings: &AppSettings, use_location: bool) -> AppSettings {
        if use_location {
            let mut s = settings.clone();
            s.weather.use_location = true;
            return s;
        }

        let mut base = if settings.weather.location_source == WeatherLocationSource::Device { Self::cleared(settings) } else { settings.clone() };
        base.weather.use_location = false;
        base
    }

    /// Konum bilgisini siler; hava durumu gösterilmez.
    pub fn cleared(settings: &AppSettings) -> AppSettings {
        let mut s = settings.clone();
        s.weather.location_source = WeatherLocationSource::None;
        s.weather.latitude = 0.0;
        s.weather.longitude = 0.0;
        s.weather.city = String::new();
        s.weather.location_updated_unix = 0;
        s
    }
}

/// `DateTimeOffset.FromUnixTimeSeconds`: izin verilen aralığın (yıl 9999) dışı `ArgumentOutOfRangeException` (burada panik).
fn from_unix_time_seconds(seconds: i64) -> DateTimeOffset {
    const MAX_SECONDS: i64 = 253_402_300_799;
    if seconds > MAX_SECONDS {
        panic!("Valid values are between -62135596800 and 253402300799, inclusive.");
    }
    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, 0).unwrap().fixed_offset()
}

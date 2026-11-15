//! Karşılığı: `Weather/OpenMeteoParser.cs`.

use serde_json::Value;

use super::weather_location::GeoPlace;
use crate::text;

/// Open-Meteo yanıtlarını ayrıştırır. Saf: ağ yok, bozuk ya da eksik yanıt istisna fırlatmaz (`None` / boş liste döner),
/// böylece servis çökmeden önbelleğe düşebilir.
///
/// C# ile aynı değil: kök değer nesne değilse C# `TryGetProperty` `InvalidOperationException` fırlatır (yakalanmaz);
/// burada bu durum da "bozuk yanıt" sayılıp `None` / boş liste döner.
pub struct OpenMeteoParser;

impl OpenMeteoParser {
    /// Şehir aramasında dönen en çok sonuç (istekteki `count` ile aynı).
    pub const MAX_RESULTS: usize = 5;

    /// Adın en az bu kadar karakter olması gerekir (Open-Meteo 3+ karakterde önek eşleşmesi yapıyor).
    pub const MIN_QUERY_LENGTH: usize = 2;

    /// `(Temperature, WeatherCode)`; eksik/yanlış tipli alan ya da bozuk JSON için `None`.
    pub fn parse_current(json: Option<&str>) -> Option<(f64, i32)> {
        let doc = Self::parse(json)?;
        let current = doc.get("current")?;
        let temp = current.get("temperature_2m")?;
        let code = current.get("weather_code")?;
        if !temp.is_number() || !code.is_number() {
            return None;
        }

        // GetDouble / GetInt32: tamsayı olmayan ya da Int32'ye sığmayan kod FormatException → null.
        let code = i32::try_from(code.as_i64()?).ok()?;
        Some((temp.as_f64()?, code))
    }

    /// Anlık rüzgâr hızı (`current.wind_speed_10m`, Open-Meteo varsayılanı km/sa). Alan yoksa, sayı değilse ya da
    /// negatifse `None`: rüzgâr yoksa hava durumu yine de gösterilir.
    pub fn parse_wind_speed(json: Option<&str>) -> Option<f64> {
        let doc = Self::parse(json)?;
        let wind = doc.get("current")?.get("wind_speed_10m")?;
        if !wind.is_number() {
            return None;
        }

        let value = wind.as_f64()?;
        if value.is_finite() && value >= 0.0 { Some(value) } else { None }
    }

    pub fn parse_geocoding(json: Option<&str>) -> Vec<GeoPlace> {
        let Some(doc) = Self::parse(json) else {
            return Vec::new();
        };
        let Some(Value::Array(results)) = doc.get("results") else {
            return Vec::new();
        };

        let mut places: Vec<GeoPlace> = Vec::new();
        for item in results {
            if !item.is_object() {
                continue;
            }

            let name = Self::text(item, "name");
            let Some(name) = name.filter(|n| !text::is_null_or_white_space(Some(n))) else {
                continue;
            };
            let (Some(lat), Some(lon)) = (Self::number(item, "latitude"), Self::number(item, "longitude")) else {
                continue;
            };

            if !Self::is_valid_coordinate(lat, lon) {
                continue;
            }

            places.push(GeoPlace::new(name, Self::text(item, "admin1"), Self::text(item, "country"), lat, lon));
            if places.len() == Self::MAX_RESULTS {
                break;
            }
        }

        places
    }

    pub fn is_valid_coordinate(latitude: f64, longitude: f64) -> bool {
        (-90.0..=90.0).contains(&latitude) && (-180.0..=180.0).contains(&longitude)
    }

    fn parse(json: Option<&str>) -> Option<Value> {
        if text::is_null_or_white_space(json) {
            return None;
        }
        let value: Value = serde_json::from_str(json?).ok()?;
        if value.is_object() { Some(value) } else { None }
    }

    fn number(item: &Value, property: &str) -> Option<f64> {
        let v = item.get(property)?;
        if v.is_number() { v.as_f64() } else { None }
    }

    fn text(item: &Value, property: &str) -> Option<String> {
        match item.get(property) {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        }
    }
}

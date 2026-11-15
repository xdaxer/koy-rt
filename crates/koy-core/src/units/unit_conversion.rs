use super::unit_enums::{NetworkSpeedUnit, TemperatureUnit, WindSpeedUnit};

/// Birim dönüşümleri. Saf sayısal işlem: sonuç seçilen birimde bir sayıdır, birim adı ve ondalık ayıracı App'te yazılır.
/// Ağ hızında bit çarpanı 8'dir; App ondalık ön eki (k, M, G) kullanır, ikili `SystemFormat.Speed` bayt içindir.
pub struct UnitConversion;

const KILOMETERS_PER_MILE: f64 = 1.609344;
const KILOMETERS_PER_KNOT: f64 = 1.852;

impl UnitConversion {
    /// Sıcaklık: girdi Celsius, çıktı seçilen birimde.
    pub fn temperature(celsius: f64, unit: TemperatureUnit) -> f64 {
        if unit == TemperatureUnit::Fahrenheit { celsius * 9.0 / 5.0 + 32.0 } else { celsius }
    }

    /// Rüzgâr hızı: girdi km/sa, çıktı seçilen birimde.
    pub fn wind_speed(kilometers_per_hour: f64, unit: WindSpeedUnit) -> f64 {
        match unit {
            WindSpeedUnit::MetersPerSecond => kilometers_per_hour / 3.6,
            WindSpeedUnit::MilesPerHour => kilometers_per_hour / KILOMETERS_PER_MILE,
            WindSpeedUnit::Knots => kilometers_per_hour / KILOMETERS_PER_KNOT,
            _ => kilometers_per_hour,
        }
    }

    /// Ağ hızı: girdi bayt/sn, çıktı seçilen birimde (bitte x8).
    pub fn network_speed(bytes_per_second: f64, unit: NetworkSpeedUnit) -> f64 {
        if unit == NetworkSpeedUnit::Bits { bytes_per_second * 8.0 } else { bytes_per_second }
    }
}

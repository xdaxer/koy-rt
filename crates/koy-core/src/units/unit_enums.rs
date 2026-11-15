use serde::{Deserialize, Serialize};

// Bu dosyadaki enum adları settings.json'a yazılır: yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Sıcaklık birimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Rüzgâr hızı birimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WindSpeedUnit {
    #[default]
    KilometersPerHour,
    MetersPerSecond,
    MilesPerHour,
    Knots,
}

/// Ağ hızı birimi: bayt (B/sn) ya da bit (b/sn).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NetworkSpeedUnit {
    #[default]
    Bytes,
    Bits,
}

//! Karşılığı: `Weather/WeatherRefreshInterval.cs`.

use serde::{Deserialize, Serialize};

// Enum adları settings.json'a yazılır: bir kez yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Hava durumunu yenileme sıklığı. Eski dosyalarda "30m", "1h", "startup" metniydi; göçte ada çevrilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WeatherRefreshInterval {
    #[default]
    Every30Minutes,
    Hourly,
    OnStartup,
}

//! Karşılığı: `Weather/IWeatherService.cs`.

use crate::cancel::{CancellationToken, OperationCanceled};
use crate::event::Event;
use crate::time::DateTimeOffset;

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherInfo {
    pub temperature: f64,
    pub weather_code: i32,
    pub description_key: String,
    pub icon_kind: String,
    pub fetched_at: DateTimeOffset,
    /// C#: `double? WindSpeedKmh = null`.
    pub wind_speed_kmh: Option<f64>,
}

impl WeatherInfo {
    pub fn new(
        temperature: f64,
        weather_code: i32,
        description_key: impl Into<String>,
        icon_kind: impl Into<String>,
        fetched_at: DateTimeOffset,
        wind_speed_kmh: Option<f64>,
    ) -> Self {
        WeatherInfo {
            temperature,
            weather_code,
            description_key: description_key.into(),
            icon_kind: icon_kind.into(),
            fetched_at,
            wind_speed_kmh,
        }
    }
}

/// Open-Meteo hava durumu sözleşmesi.
/// 30 dk önbellekleme ve tek HttpClient kullanımı şarttır.
///
/// C# `Task` dönen yöntemler burada bloklayandır (çağıran çalışan iş parçacığında çağırır). C# `IDisposable` → `dispose`.
pub trait IWeatherService: Send + Sync {
    fn current_weather(&self) -> Option<WeatherInfo>;

    /// C#: `WeatherChanged` (`EventHandler<WeatherInfo?>`).
    fn weather_changed(&self) -> &Event<Option<WeatherInfo>>;

    /// C#: `GetCurrentWeatherAsync(latitude, longitude, cancellationToken = default)`.
    fn get_current_weather(&self, latitude: f64, longitude: f64, cancellation_token: &CancellationToken) -> Result<Option<WeatherInfo>, OperationCanceled>;

    /// C#: `RefreshAsync(latitude, longitude, cancellationToken = default)`.
    fn refresh(&self, latitude: f64, longitude: f64, cancellation_token: &CancellationToken) -> Result<(), OperationCanceled>;

    fn dispose(&self);
}

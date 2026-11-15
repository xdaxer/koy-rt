//! Karşılığı: `Weather/WeatherRefreshPolicy.cs`.

use super::weather_refresh_interval::WeatherRefreshInterval;
use crate::time::TimeSpan;

/// "Hava durumunu yenileme sıklığı" ayarını (`WeatherRefreshInterval`) önbellek süresine çevirir.
/// Önbellek süresi dolmadan panel açılsa da yeni istek atılmaz; "startup" oturum boyunca yalnız ilk açılışta çeker
/// (elle yenile düğmesi yine çalışır).
pub struct WeatherRefreshPolicy;

impl WeatherRefreshPolicy {
    pub fn cache_duration(setting: WeatherRefreshInterval) -> TimeSpan {
        match setting {
            WeatherRefreshInterval::Hourly => TimeSpan::from_hours(1),
            WeatherRefreshInterval::OnStartup => TimeSpan::MAX,
            WeatherRefreshInterval::Every30Minutes => TimeSpan::from_mins(30),
        }
    }
}

//! Karşılığı: `TimerAndWeatherSettingsTests.cs` (hava durumu testleri).

use crate::settings::AppSettings;
use crate::time::TimeSpan;
use crate::weather::{WeatherRefreshInterval, WeatherRefreshPolicy};

#[test]
fn weather_cache_duration_follows_the_setting() {
    for (setting, minutes) in [(WeatherRefreshInterval::Every30Minutes, 30), (WeatherRefreshInterval::Hourly, 60)] {
        assert_eq!(TimeSpan::from_mins(minutes), WeatherRefreshPolicy::cache_duration(setting), "{setting:?}");
    }
}

#[test]
fn weather_startup_only_never_expires() {
    let duration = WeatherRefreshPolicy::cache_duration(WeatherRefreshInterval::OnStartup);

    assert!(TimeSpan::from_days(3650) < duration);
}

#[test]
fn weather_default_setting_is_thirty_minutes() {
    assert_eq!(
        TimeSpan::from_mins(30),
        WeatherRefreshPolicy::cache_duration(AppSettings::default().weather.refresh_interval)
    );
}

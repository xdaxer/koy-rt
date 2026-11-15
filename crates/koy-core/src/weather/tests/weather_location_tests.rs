//! Karşılığı: `WeatherLocationTests.cs` (`WeatherLocationTests` sınıfı).

use chrono::{Duration, FixedOffset, TimeZone};

use crate::settings::{AppSettings, SettingsStore, WeatherSettings};
use crate::time::DateTimeOffset;
use crate::weather::*;

fn now() -> DateTimeOffset {
    FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 6, 9, 0, 0).unwrap()
}

fn settings_with(weather: WeatherSettings) -> AppSettings {
    AppSettings { weather, ..AppSettings::default() }
}

#[test]
fn new_settings_have_no_location_so_weather_is_hidden() {
    let settings = AppSettings::default();

    assert_eq!(WeatherLocationSource::None, settings.weather.location_source);
    assert!(!settings.weather.use_location);
    assert!(!WeatherLocationPolicy::has_location(&settings));
}

#[test]
fn old_hardcoded_istanbul_without_a_source_is_not_a_location() {
    // Eski ayar dosyaları İstanbul'u varsayılan olarak yazmıştı; kaynak yoksa yok sayılır.
    let settings = settings_with(WeatherSettings { latitude: 41.01, longitude: 28.97, city: "İstanbul".to_string(), ..WeatherSettings::default() });

    assert!(!WeatherLocationPolicy::has_location(&settings));
}

#[test]
fn invalid_coordinates_are_not_a_location() {
    for (lat, lon) in [(0.0, 0.0), (91.0, 10.0), (10.0, 181.0)] {
        let settings = settings_with(WeatherSettings {
            location_source: WeatherLocationSource::City,
            latitude: lat,
            longitude: lon,
            ..WeatherSettings::default()
        });

        assert!(!WeatherLocationPolicy::has_location(&settings), "{lat} {lon}");
    }
}

#[test]
fn refresh_is_not_asked_when_user_did_not_allow_location() {
    assert!(!WeatherLocationPolicy::should_refresh_device_location(&AppSettings::default(), now()));
}

#[test]
fn refresh_is_asked_when_allowed_but_no_device_location_yet() {
    let settings = settings_with(WeatherSettings { use_location: true, ..WeatherSettings::default() });

    assert!(WeatherLocationPolicy::should_refresh_device_location(&settings, now()));
}

#[test]
fn refresh_is_asked_when_allowed_and_a_city_is_only_the_fallback() {
    let settings = WeatherLocationPolicy::apply_city(&AppSettings::default(), &GeoPlace::new("Ankara", None, Some("Türkiye".into()), 39.9, 32.8), now());
    let settings = WeatherLocationPolicy::set_use_location(&settings, true);

    assert!(WeatherLocationPolicy::should_refresh_device_location(&settings, now()));
}

#[test]
fn device_location_is_refreshed_at_most_once_a_day() {
    for (hours_ago, expected) in [(1, false), (23, false), (24, true), (80, true)] {
        let settings = WeatherLocationPolicy::apply_device_location(
            &settings_with(WeatherSettings { use_location: true, ..WeatherSettings::default() }),
            &LocationResult::new(LocationStatus::Success, 41.0, 29.0),
            now() - Duration::hours(hours_ago),
        );

        assert_eq!(expected, WeatherLocationPolicy::should_refresh_device_location(&settings, now()), "{hours_ago}");
    }
}

#[test]
fn a_record_dated_in_the_future_is_refreshed() {
    let settings = WeatherLocationPolicy::apply_device_location(
        &settings_with(WeatherSettings { use_location: true, ..WeatherSettings::default() }),
        &LocationResult::new(LocationStatus::Success, 41.0, 29.0),
        now() + Duration::hours(5),
    );

    assert!(WeatherLocationPolicy::should_refresh_device_location(&settings, now()));
}

#[test]
fn successful_device_location_sets_coordinates_and_clears_the_city() {
    let before = settings_with(WeatherSettings { use_location: true, city: "Eski".to_string(), ..WeatherSettings::default() });

    let after = WeatherLocationPolicy::apply_device_location(&before, &LocationResult::new(LocationStatus::Success, 40.5, 29.5), now());

    assert_eq!(WeatherLocationSource::Device, after.weather.location_source);
    assert_eq!(40.5, after.weather.latitude);
    assert_eq!(29.5, after.weather.longitude);
    assert_eq!("", after.weather.city);
    assert_eq!(now().timestamp(), after.weather.location_updated_unix);
    assert!(WeatherLocationPolicy::has_location(&after));
}

#[test]
fn failed_device_location_leaves_settings_untouched() {
    for status in [LocationStatus::Denied, LocationStatus::Unavailable, LocationStatus::TimedOut, LocationStatus::Error] {
        let mut before = WeatherLocationPolicy::apply_city(
            &settings_with(WeatherSettings { use_location: true, ..WeatherSettings::default() }),
            &GeoPlace::new("Ankara", None, None, 39.9, 32.8),
            now(),
        );
        before.weather.use_location = true;

        let after = WeatherLocationPolicy::apply_device_location(&before, &LocationResult::of(status), now() + Duration::days(1));

        assert_eq!(before, after, "{status:?}");
    }
}

#[test]
fn out_of_range_device_coordinates_are_rejected() {
    let before = settings_with(WeatherSettings { use_location: true, ..WeatherSettings::default() });

    assert_eq!(before, WeatherLocationPolicy::apply_device_location(&before, &LocationResult::new(LocationStatus::Success, 120.0, 10.0), now()));
}

#[test]
fn choosing_a_city_turns_device_location_off() {
    let before = settings_with(WeatherSettings {
        use_location: true,
        location_source: WeatherLocationSource::Device,
        latitude: 41.0,
        longitude: 29.0,
        ..WeatherSettings::default()
    });

    let after = WeatherLocationPolicy::apply_city(&before, &GeoPlace::new("İzmir", Some("İzmir".into()), Some("Türkiye".into()), 38.4, 27.1), now());

    assert_eq!(WeatherLocationSource::City, after.weather.location_source);
    assert!(!after.weather.use_location);
    assert_eq!("İzmir, Türkiye", after.weather.city);
    assert_eq!(38.4, after.weather.latitude);
}

#[test]
fn turning_location_off_drops_device_coordinates_but_keeps_a_chosen_city() {
    let device = WeatherLocationPolicy::apply_device_location(
        &settings_with(WeatherSettings { use_location: true, ..WeatherSettings::default() }),
        &LocationResult::new(LocationStatus::Success, 41.0, 29.0),
        now(),
    );
    let mut city = WeatherLocationPolicy::apply_city(&AppSettings::default(), &GeoPlace::new("Bursa", None, None, 40.2, 29.0), now());

    let off_device = WeatherLocationPolicy::set_use_location(&device, false);
    city.weather.use_location = true;
    let off_city = WeatherLocationPolicy::set_use_location(&city, false);

    assert!(!WeatherLocationPolicy::has_location(&off_device));
    assert_eq!(0.0, off_device.weather.latitude);
    assert!(!off_device.weather.use_location);
    assert!(WeatherLocationPolicy::has_location(&off_city));
    assert_eq!(WeatherLocationSource::City, off_city.weather.location_source);
}

#[test]
fn place_display_name_skips_repeats_and_missing_parts() {
    assert_eq!("Ankara, Türkiye", GeoPlace::new("Ankara", Some("Ankara".into()), Some("Türkiye".into()), 1.0, 1.0).display_name());
    assert_eq!(
        "Kadıköy, İstanbul, Türkiye",
        GeoPlace::new("Kadıköy", Some("İstanbul".into()), Some("Türkiye".into()), 1.0, 1.0).display_name()
    );
    assert_eq!("Yer", GeoPlace::new("Yer", None, None, 1.0, 1.0).display_name());
}

#[test]
fn settings_with_location_survive_a_save_and_load() {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("koy-weather-tests-{nanos:x}-{}", std::process::id()));
    let result = std::panic::catch_unwind(|| {
        let store = SettingsStore::new(dir.join("settings.json"));
        let saved = WeatherLocationPolicy::apply_device_location(
            &settings_with(WeatherSettings { use_location: true, ..WeatherSettings::default() }),
            &LocationResult::new(LocationStatus::Success, 41.25, 29.75),
            now(),
        );
        store.save(&saved).unwrap();

        let loaded = store.load();

        assert_eq!(WeatherLocationSource::Device, loaded.weather.location_source);
        assert!(loaded.weather.use_location);
        assert_eq!(41.25, loaded.weather.latitude);
        assert_eq!(now().timestamp(), loaded.weather.location_updated_unix);
    });
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

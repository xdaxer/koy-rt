//! `Settings/ClockUnitsSettingsTests.cs`: saat, birim ve tepsi ayarlarının varsayılanları eski görünümü korur ve dosyada gidip gelir.

use super::TempDir;
use crate::clock::{ClockHourFormat, WeekStartSetting};
use crate::settings::*;
use crate::tray::TrayIconVisibility;
use crate::units::{NetworkSpeedUnit, TemperatureUnit, WindSpeedUnit};

#[test]
fn defaults_keep_the_current_look() {
    let settings = AppSettings::default();

    assert_eq!(ClockHourFormat::Auto, settings.notch.clock_hour_format);
    assert!(!settings.notch.clock_show_seconds);
    assert!(!settings.notch.clock_show_date);
    assert!(!settings.notch.clock_show_week_number);
    assert_eq!(WeekStartSetting::System, settings.notch.clock_first_day_of_week);
    assert_eq!("", settings.notch.clock_custom_date_format);
    assert_eq!("", settings.notch.clock_second_time_zone_id);
    assert_eq!(TemperatureUnit::Celsius, settings.general.temperature_unit);
    assert_eq!(WindSpeedUnit::KilometersPerHour, settings.general.wind_speed_unit);
    assert_eq!(NetworkSpeedUnit::Bytes, settings.general.network_speed_unit);
    assert_eq!(TrayIconVisibility::Overflow, settings.dock.new_tray_icon_visibility);
    assert!(settings.dock.tray_app_visibility.is_empty());
}

#[test]
fn new_fields_round_trip_through_the_settings_file() {
    let dir = TempDir::unique("koy-tests-");
    let store = SettingsStore::new(dir.file("settings.json"));
    let mut tray = ValueDictionary::new();
    tray.insert("discord.exe", TrayIconVisibility::Shown);
    let settings = AppSettings {
        notch: NotchSettings {
            clock_hour_format: ClockHourFormat::Hour12,
            clock_show_seconds: true,
            clock_show_date: true,
            clock_show_week_number: true,
            clock_first_day_of_week: WeekStartSetting::Monday,
            clock_custom_date_format: "dd.MM.yyyy".to_string(),
            clock_second_time_zone_id: "Tokyo Standard Time".to_string(),
            ..Default::default()
        },
        general: GeneralSettings {
            temperature_unit: TemperatureUnit::Fahrenheit,
            wind_speed_unit: WindSpeedUnit::Knots,
            network_speed_unit: NetworkSpeedUnit::Bits,
            ..Default::default()
        },
        dock: DockSettings {
            new_tray_icon_visibility: TrayIconVisibility::Hidden,
            tray_app_visibility: tray,
            ..Default::default()
        },
        ..Default::default()
    };

    store.save(&settings).unwrap();

    assert_eq!(settings, store.load());
}

#[test]
fn enum_values_are_written_as_names() {
    let dir = TempDir::unique("koy-tests-");
    let store = SettingsStore::new(dir.file("settings.json"));
    store
        .save(&AppSettings { general: GeneralSettings { temperature_unit: TemperatureUnit::Fahrenheit, ..Default::default() }, ..Default::default() })
        .unwrap();

    let text = std::fs::read_to_string(dir.file("settings.json")).unwrap();

    assert!(text.contains("\"TemperatureUnit\": \"Fahrenheit\""));
}

#[test]
fn tray_app_visibility_changes_settings_equality() {
    let empty = AppSettings::default();
    let mut tray = ValueDictionary::new();
    tray.insert("a.exe", TrayIconVisibility::Hidden);
    let changed = AppSettings { dock: DockSettings { tray_app_visibility: tray, ..Default::default() }, ..Default::default() };

    assert_ne!(empty, changed);
}

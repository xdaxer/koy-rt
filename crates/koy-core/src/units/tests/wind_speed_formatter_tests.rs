//! Karşılığı: `Units/WindSpeedFormatterTests.cs`.
//! Taşınmayan: `Default_unit_keeps_provider_value` (Settings: GeneralSettings) ve `OpenMeteoWindParserTests`
//! (Weather: OpenMeteoParser) — o modüller henüz taşınmadı.

use crate::clock::Culture;
use crate::units::*;

#[test]
fn every_unit_has_a_symbol() {
    let rows = [
        (WindSpeedUnit::KilometersPerHour, "km/h"),
        (WindSpeedUnit::MetersPerSecond, "m/s"),
        (WindSpeedUnit::MilesPerHour, "mph"),
        (WindSpeedUnit::Knots, "kn"),
    ];
    for (unit, symbol) in rows {
        assert_eq!(symbol, WindSpeedFormatter::symbol(unit));
    }
}

#[test]
fn converts_from_kilometers_per_hour_and_formats() {
    let rows = [
        (18.0, WindSpeedUnit::KilometersPerHour, "18 km/h"),
        (18.4, WindSpeedUnit::KilometersPerHour, "18 km/h"),
        (18.5, WindSpeedUnit::KilometersPerHour, "19 km/h"),
        (36.0, WindSpeedUnit::MetersPerSecond, "10.0 m/s"),
        (12.0, WindSpeedUnit::MetersPerSecond, "3.3 m/s"),
        (160.9344, WindSpeedUnit::MilesPerHour, "100 mph"),
        (0.0, WindSpeedUnit::Knots, "0 kn"),
    ];
    for (kmh, unit, expected) in rows {
        assert_eq!(Some(expected.to_string()), WindSpeedFormatter::format(kmh, unit, &Culture::invariant()), "{kmh} {unit:?}");
    }
}

#[test]
fn uses_the_culture_decimal_separator() {
    assert_eq!(
        Some("3,3 m/s".to_string()),
        WindSpeedFormatter::format(12.0, WindSpeedUnit::MetersPerSecond, &Culture::from_name("tr-TR").unwrap())
    );
}

#[test]
fn invalid_values_give_null() {
    for kmh in [f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(None, WindSpeedFormatter::format(kmh, WindSpeedUnit::KilometersPerHour, &Culture::invariant()), "{kmh}");
    }
}

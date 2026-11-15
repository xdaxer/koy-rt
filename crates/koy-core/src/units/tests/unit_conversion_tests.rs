//! Karşılığı: `Units/UnitConversionTests.cs`. `Assert.Equal(expected, actual, 6)` = 6 ondalık basamakta eşitlik.

use crate::units::*;

fn assert_close(expected: f64, actual: f64) {
    // xUnit: yuvarlanmış (6 basamak) değerler eşit olmalı.
    assert_eq!(crate::text::round_digits(expected, 6), crate::text::round_digits(actual, 6), "{expected} vs {actual}");
}

#[test]
fn celsius_converts_to_fahrenheit() {
    for (celsius, expected) in [(0.0, 32.0), (100.0, 212.0), (-40.0, -40.0)] {
        assert_close(expected, UnitConversion::temperature(celsius, TemperatureUnit::Fahrenheit));
        assert_close(celsius, UnitConversion::temperature(celsius, TemperatureUnit::Celsius));
    }
}

#[test]
fn wind_speed_converts_from_kilometers_per_hour() {
    let rows = [
        (WindSpeedUnit::KilometersPerHour, 36.0, 36.0),
        (WindSpeedUnit::MetersPerSecond, 36.0, 10.0),
        (WindSpeedUnit::MilesPerHour, 160.9344, 100.0),
        (WindSpeedUnit::Knots, 1.852, 1.0),
    ];
    for (unit, kmh, expected) in rows {
        assert_close(expected, UnitConversion::wind_speed(kmh, unit));
    }
}

#[test]
fn network_speed_in_bits_is_eight_times_bytes() {
    assert_close(8.0, UnitConversion::network_speed(1.0, NetworkSpeedUnit::Bits));
    assert_close(1_000_000.0 * 8.0, UnitConversion::network_speed(1_000_000.0, NetworkSpeedUnit::Bits));
}

#[test]
fn network_speed_in_bytes_is_unchanged() {
    assert_close(512.0, UnitConversion::network_speed(512.0, NetworkSpeedUnit::Bytes));
}

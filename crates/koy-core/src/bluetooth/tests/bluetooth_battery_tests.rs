use std::panic::catch_unwind;

use crate::bluetooth::*;

#[test]
fn gatt_battery_parser_valid_bytes_returns_expected_percent() {
    let cases: [(u8, i32); 5] = [(0, 0), (1, 1), (50, 50), (86, 86), (100, 100)];
    for (raw, expected) in cases {
        assert_eq!(GattBatteryParser::parse(&[raw]), Some(expected), "raw = {raw}");
    }
}

#[test]
fn gatt_battery_parser_reserved_or_invalid_bytes_returns_null() {
    for raw in [101u8, 150, 255] {
        assert_eq!(GattBatteryParser::parse(&[raw]), None, "raw = {raw}");
    }
}

#[test]
fn gatt_battery_parser_empty_span_returns_null() {
    assert_eq!(GattBatteryParser::parse(&[]), None);
}

#[test]
fn gatt_battery_parser_multi_byte_payload_uses_first_byte() {
    assert_eq!(GattBatteryParser::parse(&[77, 0x01, 0x02]), Some(77));
}

#[test]
fn gatt_battery_parser_parse_lowest_picks_minimum_valid() {
    let readings: [&[u8]; 4] = [
        &[85],
        &[62],
        &[90],
        &[255], // geçersiz, atlanmalı
    ];

    assert_eq!(GattBatteryParser::parse_lowest(Some(&readings)), Some(62));
}

#[test]
fn gatt_battery_parser_parse_lowest_all_invalid_returns_null() {
    let readings: [&[u8]; 2] = [&[105], &[200]];

    assert_eq!(GattBatteryParser::parse_lowest(Some(&readings)), None);
}

#[test]
fn bluetooth_battery_filter_first_reading_returns_sample_immediately() {
    let filter = BluetoothBatteryFilter::default();
    assert_eq!(filter.filter("dev-1", 75), 75);
}

#[test]
fn bluetooth_battery_filter_clamps_out_of_bounds_input() {
    let filter = BluetoothBatteryFilter::default();
    assert_eq!(filter.filter("dev-1", -15), 0);
    filter.reset("dev-1");
    assert_eq!(filter.filter("dev-1", 130), 100);
}

#[test]
fn bluetooth_battery_filter_outlier_voltage_drop_suppressed_by_median() {
    // 3'lük pencerede tekil voltaj çöküşü (örn. RGB ya da sensör hareketiyle anlık 40'a düşüş)
    let filter = BluetoothBatteryFilter::new(3, 2, 3);

    assert_eq!(filter.filter("dev-mouse", 80), 80);
    assert_eq!(filter.filter("dev-mouse", 80), 80);

    // Tekil voltaj çöküşü: örnekler [80, 80, 40] -> medyan = 80
    assert_eq!(filter.filter("dev-mouse", 40), 80);

    // Tekrar normal okuma: örnekler [80, 40, 80] -> medyan = 80
    assert_eq!(filter.filter("dev-mouse", 80), 80);
}

#[test]
fn bluetooth_battery_filter_single_spike_suppressed_by_median() {
    // Yük kalkınca anlık voltaj toparlanması sıçraması
    let filter = BluetoothBatteryFilter::new(3, 2, 3);

    assert_eq!(filter.filter("dev-1", 40), 40);
    assert_eq!(filter.filter("dev-1", 40), 40);

    // Anlık sıçrama: örnekler [40, 40, 75] -> medyan = 40
    assert_eq!(filter.filter("dev-1", 75), 40);

    // Örnekler [40, 75, 40] -> medyan = 40
    assert_eq!(filter.filter("dev-1", 40), 40);
}

#[test]
fn bluetooth_battery_filter_hysteresis_suppresses_small_jitter() {
    // 70 ile 71 arasında tek haneli ADC titreşimi
    let filter = BluetoothBatteryFilter::new(3, 2, 3);

    assert_eq!(filter.filter("dev-kb", 70), 70);
    assert_eq!(filter.filter("dev-kb", 70), 70);
    assert_eq!(filter.filter("dev-kb", 70), 70);

    // 71 geldiğinde medyan 70..71 olur ama fark 1 (< 2) olduğu için bastırılır
    assert_eq!(filter.filter("dev-kb", 71), 70);
    assert_eq!(filter.filter("dev-kb", 70), 70);
    assert_eq!(filter.filter("dev-kb", 71), 70);
}

#[test]
fn bluetooth_battery_filter_persistent_small_change_accepted_after_consecutive_count() {
    // 70'den 69'a kalıcı olarak düşüyor (1 birim fark), 3 ardışık okuma sonra güncellenmeli
    let filter = BluetoothBatteryFilter::new(1, 2, 3);

    assert_eq!(filter.filter("dev-1", 70), 70);

    // 1. okuma (delta = -1, eşik 2 altında, beklemede)
    assert_eq!(filter.filter("dev-1", 69), 70);
    // 2. okuma (aynı yönde 2. kez)
    assert_eq!(filter.filter("dev-1", 69), 70);
    // 3. okuma (aynı yönde 3. kez -> artık güncellenir)
    assert_eq!(filter.filter("dev-1", 69), 69);
}

#[test]
fn bluetooth_battery_filter_large_delta_updates_immediately() {
    // Şarja takıldı: 40 -> 60 (+20 birim)
    let filter = BluetoothBatteryFilter::new(1, 2, 3);

    assert_eq!(filter.filter("dev-1", 40), 40);
    assert_eq!(filter.filter("dev-1", 60), 60);
}

#[test]
fn bluetooth_battery_filter_multiple_devices_isolated() {
    let filter = BluetoothBatteryFilter::default();

    assert_eq!(filter.filter("dev-mouse", 95), 95);
    assert_eq!(filter.filter("dev-keyboard", 30), 30);

    assert_eq!(filter.filter("dev-mouse", 95), 95);
    assert_eq!(filter.filter("dev-keyboard", 30), 30);
}

#[test]
fn bluetooth_battery_filter_reset_clears_device_state() {
    let filter = BluetoothBatteryFilter::new(3, 2, 3);

    assert_eq!(filter.filter("dev-1", 80), 80);
    assert_eq!(filter.filter("dev-1", 80), 80);

    filter.reset("dev-1");

    // Reset sonrası ilk okuma hemen yeni değer olarak atanır
    assert_eq!(filter.filter("dev-1", 50), 50);
}

#[test]
fn bluetooth_battery_filter_hysteresis_threshold_one_updates_immediately_on_direction_change() {
    // consecutiveThreshold = 1 iken yön değiştiğinde ilk okumada güncellenmeli
    let filter = BluetoothBatteryFilter::new(1, 2, 1);

    assert_eq!(filter.filter("dev-1", 70), 70);
    // Delta = -1, consecutiveThreshold = 1 olduğu için anında güncellenir
    assert_eq!(filter.filter("dev-1", 69), 69);
}

#[test]
fn gatt_battery_parser_parse_lowest_null_readings_returns_null() {
    assert_eq!(GattBatteryParser::parse_lowest(None), None);
}

#[test]
fn bluetooth_battery_filter_invalid_window_size_throws_argument_out_of_range_exception() {
    // C# `Assert.Throws<ArgumentOutOfRangeException>`: Rust'ta panik.
    for size in [0, -1, 65, 100] {
        let result = catch_unwind(|| BluetoothBatteryFilter::new(size, 2, 3));
        assert!(result.is_err(), "windowSize = {size}");
    }
}

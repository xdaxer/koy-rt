use std::sync::Arc;

use crate::bluetooth::*;
use crate::time::testing::FakeClock;
use crate::time::TimeSpan;

fn create_device(
    id: &str,
    name: &str,
    kind: BluetoothDeviceKind,
    is_connected: bool,
    battery_percent: Option<i32>,
    container_id: Option<&str>,
) -> BluetoothDeviceInfo {
    let device = BluetoothDeviceInfo::new(id, name, kind, is_connected, battery_percent);
    match container_id {
        Some(c) => device.with_container_id(c),
        None => device,
    }
}

fn headphones(id: &str, name: &str, is_connected: bool) -> BluetoothDeviceInfo {
    create_device(id, name, BluetoothDeviceKind::Headphones, is_connected, None, None)
}

fn secs(s: f64) -> TimeSpan {
    TimeSpan::from_seconds_f64(s)
}

fn connected(d: &BluetoothDeviceInfo, battery: Option<i32>) -> BluetoothDeviceInfo {
    BluetoothDeviceInfo { is_connected: true, battery_percent: battery, ..d.clone() }
}

fn with_connected(d: &BluetoothDeviceInfo, is_connected: bool) -> BluetoothDeviceInfo {
    BluetoothDeviceInfo { is_connected, ..d.clone() }
}

#[test]
fn initial_call_seeds_state_does_not_emit_events() {
    let mut policy = BluetoothHudPolicy::new(None);
    let devices = [
        create_device("dev1", "Sony WH-1000XM4", BluetoothDeviceKind::Headphones, true, Some(85), None),
        create_device("dev2", "AirPods Pro", BluetoothDeviceKind::Headphones, false, None, None),
    ];

    let events = policy.update_with_now(&devices, secs(1.0));

    assert!(events.is_empty());
}

#[test]
fn audio_device_connect_emits_connected_event() {
    let mut policy = BluetoothHudPolicy::new(None);
    let dev = headphones("dev1", "Sony WH-1000XM4", false);

    // Seeding
    policy.update_with_now(&[dev.clone()], secs(0.0));

    // Connects
    let conn = connected(&dev, Some(90));
    let events = policy.update_with_now(&[conn], secs(1.0));

    assert_eq!(events.len(), 1);
    let evt = &events[0];
    assert_eq!(evt.name, "Sony WH-1000XM4");
    assert!(evt.connected);
    assert_eq!(evt.battery_percent, Some(90));
}

#[test]
fn audio_device_disconnect_emits_disconnected_event() {
    let mut policy = BluetoothHudPolicy::new(None);
    let dev = create_device("dev1", "JBL Flip 6", BluetoothDeviceKind::Speaker, true, Some(70), None);

    // Seeding
    policy.update_with_now(&[dev.clone()], secs(0.0));

    // Disconnects
    let disconnected = BluetoothDeviceInfo { is_connected: false, battery_percent: None, ..dev.clone() };
    let events = policy.update_with_now(&[disconnected], secs(1.0));

    assert_eq!(events.len(), 1);
    let evt = &events[0];
    assert_eq!(evt.name, "JBL Flip 6");
    assert!(!evt.connected);
    assert_eq!(evt.battery_percent, None);
}

#[test]
fn non_audio_device_ignored_produces_no_events() {
    let mut policy = BluetoothHudPolicy::new(None);
    let mouse = create_device("mouse1", "Logitech MX Master 3", BluetoothDeviceKind::Mouse, false, None, None);
    let keyboard = create_device("kb1", "Keychron K2", BluetoothDeviceKind::Keyboard, false, None, None);

    // Seeding
    policy.update_with_now(&[mouse.clone(), keyboard.clone()], secs(0.0));

    // Connect both non-audio devices
    let mouse_conn = connected(&mouse, Some(80));
    let kb_conn = connected(&keyboard, Some(60));
    let events = policy.update_with_now(&[mouse_conn, kb_conn], secs(1.0));

    assert!(events.is_empty());
}

#[test]
fn debounce_rapid_bounce_suppresses_second_transition() {
    let mut policy = BluetoothHudPolicy::new(None);
    let dev = headphones("dev1", "Bose QC45", false);

    // Seeding at t=0
    policy.update_with_now(&[dev.clone()], secs(0.0));

    // Connects at t=1.0s -> emits
    let conn = connected(&dev, Some(100));
    let events1 = policy.update_with_now(&[conn], secs(1.0));
    assert_eq!(events1.len(), 1);
    assert!(events1[0].connected);

    // Rapid disconnect at t=2.0s (1.0s later, within 3.0s window) -> suppressed
    let disconn = with_connected(&dev, false);
    let events2 = policy.update_with_now(&[disconn.clone()], secs(2.0));
    assert!(events2.is_empty());

    // Still disconnected at t=3.5s (2.5s later from t=1.0s, still < 3.0s) -> suppressed
    let events3 = policy.update_with_now(&[disconn], secs(3.5));
    assert!(events3.is_empty());
}

#[test]
fn debounce_valid_event_after_3_seconds_emitted() {
    let mut policy = BluetoothHudPolicy::new(None);
    let dev = headphones("dev1", "Bose QC45", false);

    // Seeding at t=0
    policy.update_with_now(&[dev.clone()], secs(0.0));

    // Connects at t=1.0s -> emits
    let conn = connected(&dev, Some(100));
    let events1 = policy.update_with_now(&[conn], secs(1.0));
    assert_eq!(events1.len(), 1);

    // Disconnects at t=4.5s (3.5s later, > 3.0s window) -> should emit
    let disconn = with_connected(&dev, false);
    let events2 = policy.update_with_now(&[disconn], secs(4.5));
    assert_eq!(events2.len(), 1);
    assert!(!events2[0].connected);
    assert_eq!(events2[0].name, "Bose QC45");
}

#[test]
fn battery_change_while_connected_does_not_emit() {
    let mut policy = BluetoothHudPolicy::new(None);
    let dev = create_device("dev1", "AirPods Max", BluetoothDeviceKind::Headphones, true, Some(90), None);

    // Seeding
    policy.update_with_now(&[dev.clone()], secs(0.0));

    // Battery updates from 90% to 85%, connection state remains connected
    let updated = BluetoothDeviceInfo { battery_percent: Some(85), ..dev.clone() };
    let events = policy.update_with_now(&[updated], secs(5.0));

    assert!(events.is_empty());
}

#[test]
fn multiple_devices_tracked_independently() {
    let mut policy = BluetoothHudPolicy::new(None);
    let dev1 = headphones("dev1", "Headphones 1", false);
    let dev2 = create_device("dev2", "Speaker 2", BluetoothDeviceKind::Speaker, false, None, None);

    // Seeding
    policy.update_with_now(&[dev1.clone(), dev2.clone()], secs(0.0));

    // Dev1 connects at t=1.0s -> emits
    let events1 = policy.update_with_now(&[with_connected(&dev1, true), dev2.clone()], secs(1.0));
    assert_eq!(events1.len(), 1);
    assert_eq!(events1[0].name, "Headphones 1");
    assert!(events1[0].connected);

    // Dev2 connects at t=2.0s (1.0s after dev1, but dev2 has independent debounce) -> emits
    let events2 = policy.update_with_now(&[with_connected(&dev1, true), with_connected(&dev2, true)], secs(2.0));
    assert_eq!(events2.len(), 1);
    assert_eq!(events2[0].name, "Speaker 2");
    assert!(events2[0].connected);

    // Dev1 rapidly disconnects at t=2.5s (1.5s after dev1 connect) -> dev1 suppressed
    let events3 = policy.update_with_now(&[with_connected(&dev1, false), with_connected(&dev2, true)], secs(2.5));
    assert!(events3.is_empty());
}

#[test]
fn device_disappeared_while_connected_emits_disconnected() {
    let mut policy = BluetoothHudPolicy::new(None);
    let dev = create_device("dev1", "Sony LinkBuds", BluetoothDeviceKind::Headphones, true, Some(50), None);

    // Seeding
    policy.update_with_now(&[dev], secs(0.0));

    // Device completely disappears from scan at t=5.0s (e.g. out of range / turned off)
    let events = policy.update_with_now(&[], secs(5.0));

    assert_eq!(events.len(), 1);
    let evt = &events[0];
    assert_eq!(evt.name, "Sony LinkBuds");
    assert!(!evt.connected);
    assert_eq!(evt.battery_percent, None);
}

#[test]
fn dual_mode_device_normalized_by_container_id_tracks_unified_device() {
    let mut policy = BluetoothHudPolicy::new(None);
    let classic = create_device("dev_classic", "Marshall Major IV", BluetoothDeviceKind::Headphones, false, None, Some("container-1"));
    let le = create_device("dev_le", "Marshall Major IV", BluetoothDeviceKind::Headphones, false, None, Some("container-1")).with_low_energy(true);

    // Seeding
    policy.update_with_now(&[classic.clone(), le.clone()], secs(0.0));

    // Both update to connected
    let classic_conn = connected(&classic, Some(75));
    let le_conn = connected(&le, Some(75));
    let events = policy.update_with_now(&[classic_conn, le_conn], secs(1.0));

    // Only one normalized event emitted
    assert_eq!(events.len(), 1);
    let evt = &events[0];
    assert_eq!(evt.name, "Marshall Major IV");
    assert!(evt.connected);
    assert_eq!(evt.battery_percent, Some(75));
}

#[test]
fn fake_clock_time_provider_integration() {
    let clock = Arc::new(FakeClock::new());
    let mut policy = BluetoothHudPolicy::new(Some(clock.clone()));

    let dev = headphones("dev1", "Pixel Buds Pro", false);

    // Seeding via parameterless Update()
    policy.update(&[dev.clone()]);

    clock.advance(TimeSpan::from_secs(1));
    let events1 = policy.update(&[connected(&dev, Some(95))]);
    assert_eq!(events1.len(), 1);
    assert!(events1[0].connected);

    // Flap within 2s -> suppressed
    clock.advance(TimeSpan::from_secs(2));
    let events2 = policy.update(&[with_connected(&dev, false)]);
    assert!(events2.is_empty());

    // Advance 2 more seconds (total 4s from connect) -> emits disconnect
    clock.advance(TimeSpan::from_secs(2));
    let events3 = policy.update(&[with_connected(&dev, false)]);
    assert_eq!(events3.len(), 1);
    assert!(!events3[0].connected);
}

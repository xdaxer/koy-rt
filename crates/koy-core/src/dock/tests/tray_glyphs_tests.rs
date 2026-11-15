use crate::dock::TrayGlyphs;
use crate::network::{NetworkKind, NetworkStatus};

#[test]
fn battery_maps_percent_to_ten_levels() {
    let cases = [
        (0, false, "\u{EBA0}"),
        (54, false, "\u{EBA5}"),
        (100, false, "\u{EBAA}"),
        (0, true, "\u{EBAB}"),
        (100, true, "\u{EBB5}"),
        (150, false, "\u{EBAA}"),
    ];
    for (percent, charging, expected) in cases {
        assert_eq!(expected, TrayGlyphs::battery(percent, charging), "({percent}, {charging})");
    }
}

#[test]
fn volume_maps_level_and_mute() {
    let cases = [
        (0.5f32, true, "\u{E74F}"),
        (0.0, false, "\u{E74F}"),
        (0.2, false, "\u{E993}"),
        (0.5, false, "\u{E994}"),
        (0.9, false, "\u{E995}"),
    ];
    for (volume, muted, expected) in cases {
        assert_eq!(expected, TrayGlyphs::volume(volume, muted), "({volume}, {muted})");
    }
}

#[test]
fn network_shows_offline_when_disconnected() {
    assert_eq!(TrayGlyphs::OFFLINE, TrayGlyphs::network(&NetworkStatus::disconnected()));
    assert_eq!("", TrayGlyphs::wifi_level(&NetworkStatus::disconnected()));
}

#[test]
fn network_shows_offline_when_connected_without_internet() {
    let status = NetworkStatus::new(NetworkKind::WiFi, false, 4);
    assert_eq!(TrayGlyphs::OFFLINE, TrayGlyphs::network(&status));
    assert_eq!("", TrayGlyphs::wifi_level(&status));
}

#[test]
fn network_shows_ethernet_without_signal_overlay() {
    let status = NetworkStatus::new(NetworkKind::Ethernet, true, 0);
    assert_eq!(TrayGlyphs::ETHERNET, TrayGlyphs::network(&status));
    assert_eq!("", TrayGlyphs::wifi_level(&status));
}

#[test]
fn wifi_level_maps_signal_bars() {
    for (bars, expected) in [(1, "\u{E872}"), (2, "\u{E873}"), (4, "\u{E874}"), (5, "")] {
        let status = NetworkStatus::new(NetworkKind::WiFi, true, bars);
        assert_eq!(TrayGlyphs::WIFI_FULL, TrayGlyphs::network(&status), "{bars}");
        assert_eq!(expected, TrayGlyphs::wifi_level(&status), "{bars}");
    }
}

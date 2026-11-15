use crate::network::*;

#[test]
fn disconnected_status_is_described_as_disconnected() {
    assert_eq!(NetworkDisplay::Disconnected, NetworkSummary::describe(&NetworkStatus::disconnected()));
    assert!(!NetworkSummary::is_connected(&NetworkStatus::disconnected()));
}

#[test]
fn connection_without_internet_is_flagged_even_for_wifi() {
    let status = NetworkStatus::with_name(NetworkKind::WiFi, false, 3, Some("Ev"));

    assert_eq!(NetworkDisplay::NoInternet, NetworkSummary::describe(&status));
    assert!(!NetworkSummary::is_connected(&status));
}

#[test]
fn connected_kinds_map_to_their_display() {
    let cases = [
        (NetworkKind::WiFi, NetworkDisplay::WiFi),
        (NetworkKind::Ethernet, NetworkDisplay::Ethernet),
        (NetworkKind::Cellular, NetworkDisplay::Cellular),
    ];
    for (kind, expected) in cases {
        let status = NetworkStatus::new(kind, true, 4);

        assert_eq!(expected, NetworkSummary::describe(&status), "{kind:?}");
        assert!(NetworkSummary::is_connected(&status), "{kind:?}");
    }
}

#[test]
fn wifi_name_is_trimmed_and_only_given_for_wifi() {
    assert_eq!(Some("Ev Ağı".to_owned()), NetworkSummary::wifi_name(&NetworkStatus::with_name(NetworkKind::WiFi, true, 4, Some("  Ev Ağı "))));
    assert_eq!(None, NetworkSummary::wifi_name(&NetworkStatus::with_name(NetworkKind::WiFi, true, 4, Some("   "))));
    assert_eq!(None, NetworkSummary::wifi_name(&NetworkStatus::new(NetworkKind::WiFi, true, 4)));
    assert_eq!(None, NetworkSummary::wifi_name(&NetworkStatus::with_name(NetworkKind::Ethernet, true, 0, Some("Ethernet 2"))));
}

#[test]
fn status_with_a_different_name_is_a_different_status() {
    // Servis yalnız durum değişince olay verir; ağ adı değişince de verilmeli (ev -> ofis aynı tür, aynı sinyal).
    assert_ne!(
        NetworkStatus::with_name(NetworkKind::WiFi, true, 4, Some("Ev")),
        NetworkStatus::with_name(NetworkKind::WiFi, true, 4, Some("Ofis"))
    );
}

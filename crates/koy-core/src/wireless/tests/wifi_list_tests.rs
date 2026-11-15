use crate::wireless::*;
use std::collections::HashSet;

fn net(ssid: &str) -> WifiNetwork {
    WifiNetwork::new(ssid, 60, true, false, false)
}

fn net_q(ssid: &str, quality: i32) -> WifiNetwork {
    WifiNetwork::new(ssid, quality, true, false, false)
}

fn scan(networks: Vec<WifiNetwork>) -> WifiScanResult {
    WifiScanResult::new(WifiAccess::Allowed, true, false, networks)
}

#[test]
fn quality_from_rssi_dogrusal_ve_sinirli() {
    for (dbm, expected) in [(-100.0, 0), (-120.0, 0), (-75.0, 50), (-50.0, 100), (-30.0, 100)] {
        assert_eq!(expected, WifiList::quality_from_rssi(dbm), "{dbm}");
    }
}

#[test]
fn quality_from_rssi_nan_sifir() {
    assert_eq!(0, WifiList::quality_from_rssi(f64::NAN));
}

#[test]
fn signal_bars_esikler() {
    for (quality, bars) in [(0, 0), (1, 1), (25, 1), (26, 2), (50, 2), (51, 3), (75, 3), (76, 4), (100, 4)] {
        assert_eq!(bars, WifiList::signal_bars(quality), "{quality}");
    }
}

#[test]
fn normalize_ayni_ssid_tek_satir_en_guclu_sinyal() {
    let list = WifiList::normalize(&[
        net_q("Ev", 40),
        WifiNetwork::new("Ev", 80, false, false, false),
        net_q("Ofis", 60),
    ]);

    assert_eq!(2, list.len());
    let homes: Vec<_> = list.iter().filter(|n| n.ssid == "Ev").collect();
    assert_eq!(1, homes.len());
    let home = homes[0];
    assert_eq!(80, home.signal_quality);
    assert!(home.is_secured);
}

#[test]
fn normalize_bagli_en_ustte_sonra_sinyal_sirasi_gizli_en_sonda() {
    let list = WifiList::normalize(&[
        net_q("", 99),
        net_q("A", 90),
        WifiNetwork::new("B", 30, true, true, false),
        net_q("C", 70),
    ]);

    let ssids: Vec<&str> = list.iter().map(|n| n.ssid.as_str()).collect();
    assert_eq!(vec!["B", "A", "C", ""], ssids);
}

#[test]
fn normalize_gizli_aglar_tek_satirda_toplanir() {
    let list = WifiList::normalize(&[net_q("", 20), net_q("  ", 50), net_q("X", 10)]);

    assert_eq!(2, list.len());
    let hidden: Vec<_> = list.iter().filter(|n| n.is_hidden()).collect();
    assert_eq!(1, hidden.len());
    assert_eq!(50, hidden[0].signal_quality);
}

#[test]
fn normalize_bagli_ve_kayitli_bayraklari_birlesir() {
    let list = WifiList::normalize(&[
        WifiNetwork::new("Ev", 40, true, false, true),
        WifiNetwork::new("Ev", 70, true, true, false),
    ]);

    assert_eq!(1, list.len());
    let home = &list[0];
    assert!(home.is_connected);
    assert!(home.has_profile);
}

#[test]
fn resolve_sonuc_yok_yukleniyor() {
    assert_eq!(WifiListState::Loading, WifiList::resolve(None, true));
}

#[test]
fn resolve_adaptor_yok() {
    let result = WifiScanResult::new(WifiAccess::Allowed, false, false, vec![]);
    assert_eq!(WifiListState::NoAdapter, WifiList::resolve(Some(&result), true));
}

#[test]
fn resolve_erisim_reddi() {
    for access in [WifiAccess::DeniedByUser, WifiAccess::DeniedBySystem] {
        let result = WifiScanResult::new(access, true, false, vec![net("A")]);
        assert_eq!(WifiListState::NoAccess, WifiList::resolve(Some(&result), true), "{access:?}");
    }
}

#[test]
fn resolve_radyo_kapali() {
    assert_eq!(WifiListState::RadioOff, WifiList::resolve(Some(&scan(vec![net("A")])), false));
}

#[test]
fn resolve_tarama_hatasi() {
    let result = WifiScanResult::new(WifiAccess::Allowed, true, true, vec![]);
    assert_eq!(WifiListState::ScanFailed, WifiList::resolve(Some(&result), true));
}

#[test]
fn resolve_bos_liste() {
    assert_eq!(WifiListState::Empty, WifiList::resolve(Some(&scan(vec![])), true));
}

#[test]
fn resolve_hepsi_adsiz_konum_izni_yok() {
    assert_eq!(WifiListState::LocationBlocked, WifiList::resolve(Some(&scan(vec![net(""), net(" ")])), true));
}

#[test]
fn resolve_adli_ag_var_hazir() {
    assert_eq!(WifiListState::Ready, WifiList::resolve(Some(&scan(vec![net(""), net("Ev")])), true));
}

#[test]
fn action_for_bagli_gizli_hicbir() {
    for n in [WifiNetwork::new("A", 60, true, true, false), net("")] {
        assert_eq!(WifiRowAction::None, WifiList::action_for(&n), "{n:?}");
    }
}

#[test]
fn action_for_kayitli_ya_da_acik_dogrudan() {
    assert_eq!(WifiRowAction::ConnectDirect, WifiList::action_for(&WifiNetwork::new("A", 60, true, false, true)));
    assert_eq!(WifiRowAction::ConnectDirect, WifiList::action_for(&WifiNetwork::new("A", 60, false, false, false)));
}

#[test]
fn action_for_guvenli_kayitsiz_sifre_sorar() {
    assert_eq!(WifiRowAction::AskPassword, WifiList::action_for(&WifiNetwork::new("A", 60, true, false, false)));
}

#[test]
fn status_key_bagli_guvenli_acik_kayitli() {
    assert_eq!(Some("WifiConnectedSecure"), WifiList::status_key(&WifiNetwork::new("A", 60, true, true, false)));
    assert_eq!(Some("WifiConnectedOpen"), WifiList::status_key(&WifiNetwork::new("A", 60, false, true, false)));
    assert_eq!(Some("WifiSaved"), WifiList::status_key(&WifiNetwork::new("A", 60, true, false, true)));
    assert_eq!(None, WifiList::status_key(&net("A")));
}

#[test]
fn error_key_basari_null_digerleri_anahtar() {
    assert_eq!(None, WifiList::error_key(WifiConnectStatus::Connected));
    assert_eq!(Some("WifiErrPassword"), WifiList::error_key(WifiConnectStatus::WrongPassword));
    assert_eq!(Some("WifiErrFailed"), WifiList::error_key(WifiConnectStatus::Failed));
    for status in WifiConnectStatus::ALL.into_iter().filter(|s| *s != WifiConnectStatus::Connected) {
        assert!(WifiList::error_key(status).is_some_and(|k| !k.is_empty()), "{status:?}");
    }
}

#[test]
fn has_profile_bos_ssid_kayitli_sayilmaz() {
    let names: HashSet<String> = ["Ev".to_string(), String::new()].into_iter().collect();
    assert!(WifiList::has_profile(&names, "Ev"));
    assert!(!WifiList::has_profile(&names, ""));
    assert!(!WifiList::has_profile(&names, "Ofis"));
}

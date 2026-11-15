use crate::bluetooth::*;
use crate::clock::Culture;
use crate::radio::RadioTileStatus;

/// C# `Dev(id, name, kind = Headphones, connected = false, battery = null, container = null, le = false)`.
fn dev_full(
    id: &str,
    name: &str,
    kind: BluetoothDeviceKind,
    connected: bool,
    battery: Option<i32>,
    container: Option<&str>,
    le: bool,
) -> BluetoothDeviceInfo {
    let device = BluetoothDeviceInfo::new(id, name, kind, connected, battery).with_low_energy(le);
    match container {
        Some(c) => device.with_container_id(c),
        None => device,
    }
}

fn dev(id: &str, name: &str) -> BluetoothDeviceInfo {
    dev_full(id, name, BluetoothDeviceKind::Headphones, false, None, None, false)
}

fn dev_kind(id: &str, name: &str, kind: BluetoothDeviceKind) -> BluetoothDeviceInfo {
    dev_full(id, name, kind, false, None, None, false)
}

fn dev_conn(id: &str, name: &str, connected: bool) -> BluetoothDeviceInfo {
    dev_full(id, name, BluetoothDeviceKind::Headphones, connected, None, None, false)
}

fn dev_state(id: &str, name: &str, kind: BluetoothDeviceKind, connected: bool, battery: Option<i32>) -> BluetoothDeviceInfo {
    dev_full(id, name, kind, connected, battery, None, false)
}

fn names(list: &[BluetoothDeviceInfo]) -> Vec<&str> {
    list.iter().map(|d| d.name.as_str()).collect()
}

#[test]
fn from_class_of_device_ana_ve_alt_sinif() {
    use BluetoothDeviceKind::*;
    let cases: [(u32, BluetoothDeviceKind); 13] = [
        (0x418, Headphones), // ses/video, kulaklık
        (0x404, Headphones), // ses/video, kulaklık takımı
        (0x408, Headphones), // eller serbest
        (0x414, Speaker),    // hoparlör
        (0x41C, Speaker),    // taşınabilir ses
        (0x540, Keyboard),
        (0x580, Mouse),
        (0x5C0, Keyboard), // klavye + fare birleşik
        (0x500, Other),    // ayrıntısız çevre birimi
        (0x20C, Phone),
        (0x11C, Tablet),
        (0x104, Other), // masaüstü bilgisayar
        (0x0, Other),
    ];
    for (cod, expected) in cases {
        assert_eq!(BluetoothList::from_class_of_device(cod), expected, "cod = {cod:#x}");
    }
}

#[test]
fn from_appearance_kategori_ve_alt_kategori() {
    use BluetoothDeviceKind::*;
    let cases: [(u32, BluetoothDeviceKind); 11] = [
        (0x0040, Phone),
        (0x0087, Tablet),
        (0x0083, Other), // dizüstü
        (0x03C1, Keyboard),
        (0x03C2, Mouse),
        (0x03C9, Mouse), // dokunmatik yüzey
        (0x03C4, Other), // oyun kolu
        (0x0841, Speaker),
        (0x0943, Headphones),
        (0x0941, Headphones),
        (0x0000, Other),
    ];
    for (appearance, expected) in cases {
        assert_eq!(BluetoothList::from_appearance(appearance), expected, "appearance = {appearance:#x}");
    }
}

#[test]
fn is_audio_yalnizca_kulaklik_ve_hoparlor() {
    assert!(dev_kind("1", "K", BluetoothDeviceKind::Headphones).is_audio());
    assert!(dev_kind("1", "H", BluetoothDeviceKind::Speaker).is_audio());
    assert!(!dev_kind("1", "F", BluetoothDeviceKind::Mouse).is_audio());
    assert!(!dev_kind("1", "T", BluetoothDeviceKind::Phone).is_audio());
    assert!(!dev_kind("1", "D", BluetoothDeviceKind::Other).is_audio());
}

#[test]
fn normalize_bagli_once_sonra_ada_gore() {
    let list = BluetoothList::normalize(&[dev("1", "Zeta"), dev("2", "Alfa"), dev_conn("3", "Orta", true)]);

    assert_eq!(names(&list), vec!["Orta", "Alfa", "Zeta"]);
}

#[test]
fn normalize_adsiz_kayitlari_atar() {
    let list = BluetoothList::normalize(&[dev("1", ""), dev("2", "  "), dev("3", "Kulaklık")]);

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Kulaklık");
}

#[test]
fn normalize_ayni_konteyner_klasik_tercih_edilir_durum_birlesir() {
    let classic = dev_full("c", "Kulaklık", BluetoothDeviceKind::Headphones, false, None, Some("AAA"), false);
    let le = dev_full("l", "Kulaklık", BluetoothDeviceKind::Headphones, true, Some(86), Some("aaa"), true);

    let list = BluetoothList::normalize(&[le, classic]);

    assert_eq!(list.len(), 1);
    let merged = &list[0];
    assert_eq!(merged.id, "c");
    assert!(merged.is_connected);
    assert_eq!(merged.battery_percent, Some(86));
}

#[test]
fn normalize_konteyner_yoksa_birlestirmez_ayni_adli_iki_cihaz() {
    let list = BluetoothList::normalize(&[
        dev_kind("1", "Fare", BluetoothDeviceKind::Mouse),
        dev_kind("2", "Fare", BluetoothDeviceKind::Mouse),
    ]);

    assert_eq!(list.len(), 2);
}

#[test]
fn primary_audio_bagli_ses_cihazi_pili_olan_once() {
    let devices = [
        dev_state("1", "Fare", BluetoothDeviceKind::Mouse, true, Some(40)),
        dev_state("2", "Hoparlör", BluetoothDeviceKind::Speaker, true, None),
        dev_state("3", "Kulaklık", BluetoothDeviceKind::Headphones, true, Some(86)),
        dev_state("4", "Eski", BluetoothDeviceKind::Headphones, false, Some(99)),
    ];

    assert_eq!(BluetoothList::primary_audio(&devices).unwrap().name, "Kulaklık");
}

#[test]
fn primary_audio_bagli_ses_cihazi_yoksa_null() {
    let devices = [dev_state("1", "Fare", BluetoothDeviceKind::Mouse, true, None), dev_conn("2", "K", false)];
    assert!(BluetoothList::primary_audio(&devices).is_none());
}

#[test]
fn battery_text_gecerli() {
    let tr = Culture::tr_tr();
    let cases: [(i32, &str); 3] = [(86, "%86"), (0, "%0"), (100, "%100")];
    for (percent, expected) in cases {
        assert_eq!(BluetoothList::battery_text(Some(percent), &tr).as_deref(), Some(expected), "percent = {percent}");
    }
}

#[test]
fn battery_text_gecersiz_null() {
    let tr = Culture::tr_tr();
    for percent in [None, Some(-1), Some(101)] {
        assert_eq!(BluetoothList::battery_text(percent, &tr), None, "percent = {percent:?}");
    }
}

#[test]
fn parse_battery_bayt_ve_gecersiz_degerler() {
    assert_eq!(BluetoothList::parse_battery(Some(&BatteryRawValue::Byte(86))), Some(86));
    assert_eq!(BluetoothList::parse_battery(Some(&BatteryRawValue::Int(55))), Some(55));
    assert_eq!(BluetoothList::parse_battery(None), None);
    assert_eq!(BluetoothList::parse_battery(Some(&BatteryRawValue::Byte(255))), None);
    assert_eq!(BluetoothList::parse_battery(Some(&BatteryRawValue::Text("abc".to_owned()))), None);
    assert_eq!(BluetoothList::parse_battery(Some(&BatteryRawValue::Unsupported)), None);
}

#[test]
fn action_for_ses_cihazi_bagla_ya_da_kes_digeri_ayarlar() {
    assert_eq!(BluetoothList::action_for(&dev_conn("1", "K", false)), BluetoothRowAction::Connect);
    assert_eq!(BluetoothList::action_for(&dev_conn("1", "K", true)), BluetoothRowAction::Disconnect);
    assert_eq!(
        BluetoothList::action_for(&dev_state("1", "F", BluetoothDeviceKind::Mouse, true, None)),
        BluetoothRowAction::OpenSettings
    );
    assert_eq!(BluetoothList::action_for(&dev_kind("1", "T", BluetoothDeviceKind::Phone)), BluetoothRowAction::OpenSettings);
}

#[test]
fn error_key_dogrulanmis_basari_null_digerleri_anahtar() {
    assert_eq!(BluetoothList::error_key(BluetoothActionResult::Confirmed, true), None);
    assert_eq!(BluetoothList::error_key(BluetoothActionResult::Unsupported, true), Some("BluetoothErrUnsupported"));
    assert_eq!(BluetoothList::error_key(BluetoothActionResult::NotConfirmed, true), Some("BluetoothErrConnect"));
    assert_eq!(BluetoothList::error_key(BluetoothActionResult::Failed, false), Some("BluetoothErrDisconnect"));
}

#[test]
fn resolve_radyo_ve_servis_durumundan() {
    use BluetoothListState as S;
    use RadioTileStatus as R;
    let cases: [(RadioTileStatus, bool, i32, BluetoothListState); 9] = [
        (R::NoAccess, true, 3, S::NoAccess),
        (R::NotFound, true, 3, S::NotFound),
        (R::Off, true, 3, S::RadioOff),
        (R::HardwareOff, true, 3, S::RadioOff),
        (R::Loading, true, 3, S::Loading),
        (R::On, false, 0, S::Loading),
        (R::On, true, 0, S::Empty),
        (R::On, true, 2, S::Ready),
        (R::Failed, true, 2, S::Ready),
    ];
    for (radio, ready, count, expected) in cases {
        assert_eq!(BluetoothList::resolve(radio, ready, count), expected, "radio = {radio:?}, ready = {ready}, count = {count}");
    }
}

#[test]
fn status_key_bagli_bagli_degil() {
    assert_eq!(BluetoothList::status_key(&dev_conn("1", "K", true)), "BluetoothConnected");
    assert_eq!(BluetoothList::status_key(&dev_conn("1", "K", false)), "BluetoothNotConnected");
}

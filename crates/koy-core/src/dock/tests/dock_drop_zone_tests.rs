use crate::dock::{DockDropZone, DockDropZoneTracker, DockDropZones};
use crate::time::TimeSpan;

#[test]
fn detect_zone_varsayilan_yuzde_50_orta_bolgeyi_hesaplar() {
    // (width, relativeX, expected)
    let cases = [
        (40.0, 5.0, DockDropZone::Before),  // 5 < 10 (sol %25)
        (40.0, 9.9, DockDropZone::Before),  // < 10
        (40.0, 10.0, DockDropZone::Merge),  // tam %25 sınır
        (40.0, 20.0, DockDropZone::Merge),  // orta
        (40.0, 30.0, DockDropZone::Merge),  // tam %75 sınır
        (40.0, 30.1, DockDropZone::After),  // > 30 (sağ %25)
        (40.0, 39.0, DockDropZone::After),  // sağ kenar
    ];
    for (width, relative_x, expected) in cases {
        let zone = DockDropZones::detect_zone(width, relative_x, 0.5);
        assert_eq!(expected, zone, "({width}, {relative_x})");
    }
}

#[test]
fn detect_zone_sifir_veya_negatif_genislikte_merge_doner() {
    assert_eq!(DockDropZone::Merge, DockDropZones::detect_zone(0.0, 10.0, 0.5));
    assert_eq!(DockDropZone::Merge, DockDropZones::detect_zone(-10.0, 5.0, 0.5));
}

#[test]
fn resolve_zone_mutlak_koordinatlari_dogru_bagil_konuma_cevirir() {
    // Yuva 100..140 arası, imleç 105 (bağıl 5 -> Before)
    assert_eq!(DockDropZone::Before, DockDropZones::resolve_zone(105.0, 100.0, 40.0, 0.5));

    // İmleç 120 (bağıl 20 -> Merge)
    assert_eq!(DockDropZone::Merge, DockDropZones::resolve_zone(120.0, 100.0, 40.0, 0.5));

    // İmleç 135 (bağıl 35 -> After)
    assert_eq!(DockDropZone::After, DockDropZones::resolve_zone(135.0, 100.0, 40.0, 0.5));
}

#[test]
fn tracker_400ms_altinda_silahlanmaz() {
    let mut tracker = DockDropZoneTracker::default();
    let t0 = TimeSpan::from_secs(10);

    // İlk giriş: timer başlar, henüz silahlanmadı
    assert!(!tracker.update(1, DockDropZone::Merge, t0));
    assert!(!tracker.is_merge_armed());

    // 200 ms sonra: hala silahlanmadı
    assert!(!tracker.update(1, DockDropZone::Merge, t0 + TimeSpan::from_millis(200)));
    assert!(!tracker.is_merge_armed());

    // 399 ms sonra: hala silahlanmadı
    assert!(!tracker.update(1, DockDropZone::Merge, t0 + TimeSpan::from_millis(399)));
    assert!(!tracker.is_merge_armed());
}

#[test]
fn tracker_400ms_tamamlaninca_silahlanir() {
    let mut tracker = DockDropZoneTracker::default();
    let t0 = TimeSpan::from_secs(10);

    tracker.update(2, DockDropZone::Merge, t0);

    // 400 ms doldu
    let armed = tracker.update(2, DockDropZone::Merge, t0 + TimeSpan::from_millis(400));
    assert!(armed);
    assert!(tracker.is_merge_armed());

    // Sonrasında merge'de kaldığı sürece armed true kalır
    assert!(tracker.update(2, DockDropZone::Merge, t0 + TimeSpan::from_millis(600)));
    assert!(tracker.is_merge_armed());
}

#[test]
fn tracker_kenara_kayinca_veya_cikinca_silahlanma_sifirlanir() {
    let mut tracker = DockDropZoneTracker::default();
    let t0 = TimeSpan::from_secs(10);

    tracker.update(3, DockDropZone::Merge, t0);
    tracker.update(3, DockDropZone::Merge, t0 + TimeSpan::from_millis(400));
    assert!(tracker.is_merge_armed());

    // Kenara (Before) kaydı -> hemen sıfırlanmalı
    let armed = tracker.update(3, DockDropZone::Before, t0 + TimeSpan::from_millis(450));
    assert!(!armed);
    assert!(!tracker.is_merge_armed());
}

#[test]
fn tracker_hedef_yuva_degisince_bekleme_suresi_bastan_baslar() {
    let mut tracker = DockDropZoneTracker::default();
    let t0 = TimeSpan::from_secs(10);

    tracker.update(1, DockDropZone::Merge, t0);
    tracker.update(1, DockDropZone::Merge, t0 + TimeSpan::from_millis(300));

    // 350. ms'de başka bir yuvaya geçti (yuva 2)
    let t1 = t0 + TimeSpan::from_millis(350);
    assert!(!tracker.update(2, DockDropZone::Merge, t1));

    // t1'den 100 ms sonra (toplam 450 ms geçti ama yeni yuvada yalnız 100 ms) -> silahlanmamalı
    assert!(!tracker.update(2, DockDropZone::Merge, t1 + TimeSpan::from_millis(100)));
    assert!(!tracker.is_merge_armed());

    // Yeni yuvada 400 ms dolunca silahlanmalı
    assert!(tracker.update(2, DockDropZone::Merge, t1 + TimeSpan::from_millis(400)));
    assert!(tracker.is_merge_armed());
}

#[test]
fn tracker_anahtar_metin_tabanli_takip() {
    let mut tracker = DockDropZoneTracker::default();
    let t0 = TimeSpan::from_secs(5);

    assert!(!tracker.update_key("itemA", DockDropZone::Merge, t0));
    assert!(tracker.update_key("itemA", DockDropZone::Merge, t0 + TimeSpan::from_millis(450)));
    assert_eq!(Some("itemA"), tracker.target_key());

    tracker.reset();
    assert!(!tracker.is_merge_armed());
    assert_eq!(None, tracker.target_key());
}

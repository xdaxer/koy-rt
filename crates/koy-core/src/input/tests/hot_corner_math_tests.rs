//! Karşılığı: `HotCornerMathTests.cs`.

use crate::input::*;

// Birincil ekran benzeri, sol üstü (0,0) olmayan bir monitör: köşeler ofsetle birlikte hesaplanmalı.
const SCREEN: ScreenRect = ScreenRect::new(1920, 0, 1920, 1080);

#[test]
fn hit_test_dort_koseyi_bulur() {
    assert_eq!(Some(ScreenCorner::TopLeft), HotCornerMath::hit_test(1920, 0, SCREEN, 4));
    assert_eq!(Some(ScreenCorner::TopRight), HotCornerMath::hit_test(3839, 0, SCREEN, 4));
    assert_eq!(Some(ScreenCorner::BottomLeft), HotCornerMath::hit_test(1920, 1079, SCREEN, 4));
    assert_eq!(Some(ScreenCorner::BottomRight), HotCornerMath::hit_test(3839, 1079, SCREEN, 4));
}

#[test]
fn hit_test_kose_boyunun_disinda_null() {
    // Sol üst köşe 4 px; 4. piksel (indeks 4) köşenin dışındadır.
    assert_eq!(None, HotCornerMath::hit_test(1924, 0, SCREEN, 4));
    assert_eq!(None, HotCornerMath::hit_test(1920, 4, SCREEN, 4));
    assert_eq!(Some(ScreenCorner::TopLeft), HotCornerMath::hit_test(1923, 3, SCREEN, 4));
}

#[test]
fn hit_test_kenar_ortasi_kose_degil() {
    assert_eq!(None, HotCornerMath::hit_test(2880, 0, SCREEN, 4));
    assert_eq!(None, HotCornerMath::hit_test(1920, 540, SCREEN, 4));
}

#[test]
fn hit_test_ekran_disi_null() {
    assert_eq!(None, HotCornerMath::hit_test(1919, 0, SCREEN, 4));
    assert_eq!(None, HotCornerMath::hit_test(3840, 0, SCREEN, 4));
    assert_eq!(None, HotCornerMath::hit_test(1920, -1, SCREEN, 4));
    assert_eq!(None, HotCornerMath::hit_test(1920, 1080, SCREEN, 4));
}

#[test]
fn hit_test_gecersiz_boyut_null() {
    assert_eq!(None, HotCornerMath::hit_test(1920, 0, SCREEN, 0));
    assert_eq!(None, HotCornerMath::hit_test(1920, 0, ScreenRect { width: 0, ..SCREEN }, 4));
}

#[test]
fn hit_test_buyuk_kose_iki_koseyi_cakistirmaz() {
    // Köşe boyu ekranın yarısından büyük: nokta en yakın kenara göre seçilir.
    let small = ScreenRect::new(0, 0, 10, 10);

    assert_eq!(Some(ScreenCorner::TopLeft), HotCornerMath::hit_test(2, 2, small, 8));
    assert_eq!(Some(ScreenCorner::TopRight), HotCornerMath::hit_test(7, 2, small, 8));
    assert_eq!(Some(ScreenCorner::BottomLeft), HotCornerMath::hit_test(2, 7, small, 8));
    assert_eq!(Some(ScreenCorner::BottomRight), HotCornerMath::hit_test(7, 7, small, 8));
}

#[test]
fn is_dwell_complete_sure_dolunca_true() {
    assert!(!HotCornerMath::is_dwell_complete(1000, 1299, 300));
    assert!(HotCornerMath::is_dwell_complete(1000, 1300, 300));
    assert!(HotCornerMath::is_dwell_complete(1000, 1000, 0));
}

#[test]
fn is_dwell_complete_geriye_donen_zamanda_false() {
    assert!(!HotCornerMath::is_dwell_complete(2000, 1000, 0));
}

#[test]
fn hit_test_negatif_koordinatli_sol_monitor() {
    let left = ScreenRect::new(-1920, 0, 1920, 1080);

    assert_eq!(Some(ScreenCorner::TopLeft), HotCornerMath::hit_test(-1920, 0, left, 4));
    assert_eq!(Some(ScreenCorner::BottomRight), HotCornerMath::hit_test(-1, 1079, left, 4));
    assert_eq!(None, HotCornerMath::hit_test(0, 0, left, 4));
}

#[test]
fn hit_test_cok_ekran_ortak_kenardaki_kose_sayilmaz() {
    let screens = [ScreenRect::new(-1920, 0, 1920, 1080), ScreenRect::new(0, 0, 1920, 1080)];

    // Ortak kenar: imleç komşuya geçebilir, köşe değil.
    assert_eq!(None, HotCornerMath::hit_test_screens(-1, 0, &screens, 4));
    assert_eq!(None, HotCornerMath::hit_test_screens(0, 1079, &screens, 4));

    // Dış köşeler çalışır ve doğru ekranı bildirir.
    assert_eq!(Some(CornerHit::new(0, ScreenCorner::TopLeft)), HotCornerMath::hit_test_screens(-1920, 0, &screens, 4));
    assert_eq!(Some(CornerHit::new(1, ScreenCorner::BottomRight)), HotCornerMath::hit_test_screens(1919, 1079, &screens, 4));
}

#[test]
fn hit_test_cok_ekran_kaymis_komsu_icin_ortak_olmayan_kose_calisir() {
    // Sağdaki monitör 500 px aşağıda: soldakinin sağ üst köşesi komşuya açık değil.
    let screens = [ScreenRect::new(0, 0, 1920, 1080), ScreenRect::new(1920, 500, 1920, 1080)];

    assert_eq!(Some(CornerHit::new(0, ScreenCorner::TopRight)), HotCornerMath::hit_test_screens(1919, 0, &screens, 4));
    assert_eq!(None, HotCornerMath::hit_test_screens(1919, 1079, &screens, 4));
}

#[test]
fn hit_test_cok_ekran_ust_uste_komsu_alt_kenar() {
    let screens = [ScreenRect::new(0, 0, 1920, 1080), ScreenRect::new(0, 1080, 1920, 1080)];

    assert_eq!(None, HotCornerMath::hit_test_screens(0, 1079, &screens, 4));
    assert_eq!(None, HotCornerMath::hit_test_screens(0, 1080, &screens, 4));
    assert_eq!(Some(CornerHit::new(1, ScreenCorner::BottomRight)), HotCornerMath::hit_test_screens(1919, 2159, &screens, 4));
}

#[test]
fn hit_test_cok_ekran_bos_liste_ve_sifir_boyutlu_ekran() {
    assert_eq!(None, HotCornerMath::hit_test_screens(0, 0, &[], 4));

    // Sıfır boyutlu ekran komşu sayılmaz.
    let screens = [ScreenRect::new(0, 0, 1920, 1080), ScreenRect::new(1920, 0, 0, 0)];
    assert_eq!(Some(CornerHit::new(0, ScreenCorner::TopRight)), HotCornerMath::hit_test_screens(1919, 0, &screens, 4));
}

#[test]
fn is_dwell_complete_negatif_sure_sifir_gibi() {
    assert!(HotCornerMath::is_dwell_complete(1000, 1000, -50));
}

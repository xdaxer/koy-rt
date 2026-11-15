use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::fakes::FakePins;
use super::strs;
use crate::dock::{DockReorderMath, IReorderablePinnedApps, IPinnedAppsService, OrderedPinnedAppsService, PinnedApp, PinnedOrder};

fn app(file: &str) -> PinnedApp {
    // Path.GetFileNameWithoutExtension(file)
    let title = crate::win_path::get_file_name_without_extension(file);
    PinnedApp::new(title, format!(r"C:\Pinned\{file}"), None, None, None)
}

fn ids(apps: &[PinnedApp]) -> Vec<String> {
    apps.iter().map(|a| PinnedOrder::id_of(&a.shortcut_path)).collect()
}

#[test]
fn id_of_yoldan_bagimsiz_dosya_adi() {
    assert_eq!("Obsidian.lnk", PinnedOrder::id_of(r"C:\Users\x\AppData\Obsidian.lnk"));
}

#[test]
fn apply_kayitli_sirayi_yenileri_sona_ekler() {
    let apps = [app("A.lnk"), app("B.lnk"), app("C.lnk"), app("D.lnk")];

    let result = PinnedOrder::apply(&apps, &strs(&["C.lnk", "A.lnk"]));

    assert_eq!(strs(&["C.lnk", "A.lnk", "B.lnk", "D.lnk"]), ids(&result));
}

#[test]
fn apply_kayit_bossa_klasor_sirasini_korur() {
    let apps = [app("B.lnk"), app("A.lnk")];

    assert_eq!(strs(&["B.lnk", "A.lnk"]), ids(&PinnedOrder::apply(&apps, &[])));
}

#[test]
fn apply_buyuk_kucuk_harfe_duyarsiz() {
    let apps = [app("A.lnk"), app("b.LNK")];

    assert_eq!(strs(&["b.LNK", "A.lnk"]), ids(&PinnedOrder::apply(&apps, &strs(&["B.lnk"]))));
}

#[test]
fn reconcile_kaldirilani_dusurup_yeniyi_sona_ekler() {
    let result = PinnedOrder::reconcile(&strs(&["C.lnk", "X.lnk", "A.lnk"]), &strs(&["A.lnk", "B.lnk", "C.lnk"]));

    assert_eq!(strs(&["C.lnk", "A.lnk", "B.lnk"]), result);
}

#[test]
fn reconcile_mevcut_liste_bossa_kayiti_korur() {
    let result = PinnedOrder::reconcile(&strs(&["C.lnk", "A.lnk"]), &[]);

    assert_eq!(strs(&["C.lnk", "A.lnk"]), result);
}

#[test]
fn reconcile_tekrarlari_tekeindirir() {
    assert_eq!(strs(&["A.lnk", "B.lnk"]), PinnedOrder::reconcile(&strs(&["A.lnk", "a.lnk"]), &strs(&["A.lnk", "B.lnk"])));
}

#[test]
fn move_ogeyi_yeni_dizine_tasir() {
    // (id, index, expected)
    let cases: [(&str, i32, [&str; 3]); 5] = [
        ("A.lnk", 2, ["B.lnk", "C.lnk", "A.lnk"]),
        ("C.lnk", 0, ["C.lnk", "A.lnk", "B.lnk"]),
        ("B.lnk", 99, ["A.lnk", "C.lnk", "B.lnk"]),
        ("B.lnk", -5, ["B.lnk", "A.lnk", "C.lnk"]),
        ("YOK.lnk", 1, ["A.lnk", "B.lnk", "C.lnk"]),
    ];
    for (id, index, expected) in cases {
        assert_eq!(strs(&expected), PinnedOrder::move_item(&strs(&["A.lnk", "B.lnk", "C.lnk"]), id, index), "({id}, {index})");
    }
}

// Gerçek Taskband değeri gibi: ad öncesinde uzunluk baytı gibi ikili artıklar, aralarda başka dizgiler.
fn favorites(leading_bytes: usize, entries: &[&str]) -> Vec<u8> {
    fn unicode(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    let mut bytes = vec![0u8; leading_bytes];
    for entry in entries {
        bytes.extend(unicode(&format!("U{entry}")));
        bytes.extend(unicode("\0\0Microsoft.Something\0UserPinned\0"));
        bytes.extend([0x00, 0x12, 0xBE, 0xEF]);
    }

    bytes
}

#[test]
fn seed_from_taskband_degerdeki_siranin_kullanir() {
    let data = favorites(8, &["Obsidian.lnk", "Visual Studio Code.lnk", "Antigravity.lnk"]);

    let seed = PinnedOrder::seed_from_taskband(
        Some(&data),
        &strs(&["Antigravity.lnk", "Docker Desktop.lnk", "Obsidian.lnk", "Visual Studio Code.lnk"]),
    );

    // Bulunanlar değerdeki sırayla, bulunamayan (Docker) klasör sırasıyla arkadan.
    assert_eq!(strs(&["Obsidian.lnk", "Visual Studio Code.lnk", "Antigravity.lnk", "Docker Desktop.lnk"]), seed);
}

#[test]
fn seed_from_taskband_tek_hizalamada_da_calisir() {
    let data = favorites(9, &["B.lnk", "A.lnk"]);

    assert_eq!(strs(&["B.lnk", "A.lnk"]), PinnedOrder::seed_from_taskband(Some(&data), &strs(&["A.lnk", "B.lnk"])));
}

#[test]
fn seed_from_taskband_kisa_ad_uzun_adin_icinde_yanlis_eslesmez() {
    let data = favorites(8, &["Visual Studio Code.lnk", "Code.lnk"]);

    let seed = PinnedOrder::seed_from_taskband(Some(&data), &strs(&["Code.lnk", "Visual Studio Code.lnk"]));

    assert_eq!(strs(&["Visual Studio Code.lnk", "Code.lnk"]), seed);
}

#[test]
fn seed_from_taskband_hic_eslesmezse_klasor_sirasi() {
    let data = favorites(8, &["Baska.lnk"]);

    assert_eq!(strs(&["B.lnk", "A.lnk"]), PinnedOrder::seed_from_taskband(Some(&data), &strs(&["B.lnk", "A.lnk"])));
}

#[test]
fn seed_from_taskband_bos_ve_bozuk_degerde_klasor_sirasi() {
    let cases: [Option<&[u8]>; 3] = [None, Some(&[]), Some(&[1, 2, 3])];
    for data in cases {
        assert_eq!(strs(&["B.lnk", "A.lnk"]), PinnedOrder::seed_from_taskband(data, &strs(&["B.lnk", "A.lnk"])), "{data:?}");
    }
}

#[test]
fn seed_from_taskband_rastgele_baytlarda_cokmez() {
    // new Random(42) yerine sabit tohumlu basit üreteç (aynı amaç: rastgele bayt dizileri).
    let mut state: u64 = 42;
    let mut next = move || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 33) as u32
    };
    for _ in 0..50 {
        let len = (next() % 4000) as usize;
        let data: Vec<u8> = (0..len).map(|_| next() as u8).collect();

        let seed = PinnedOrder::seed_from_taskband(Some(&data), &strs(&["A.lnk", "B.lnk"]));

        assert_eq!(2, seed.len());
    }
}

/// `FakeInner`: dışarıdan değiştirilebilen sabitli uygulama listesi.
fn fake_inner(apps: Vec<PinnedApp>) -> Arc<FakePins> {
    FakePins::new(apps)
}

type Stored = Arc<Mutex<Vec<String>>>;

fn stored(items: &[&str]) -> Stored {
    Arc::new(Mutex::new(strs(items)))
}

fn service(inner: Arc<FakePins>, stored: &Stored, favorites: Option<Box<dyn Fn() -> io::Result<Option<Vec<u8>>> + Send + Sync>>) -> OrderedPinnedAppsService {
    let (load, save) = (Arc::clone(stored), Arc::clone(stored));
    OrderedPinnedAppsService::new(
        inner,
        move || load.lock().unwrap().clone(),
        move |o| {
            *save.lock().unwrap() = o.to_vec();
            Ok(())
        },
        favorites,
    )
}

#[test]
fn service_ilk_acilista_tohumu_yazar_sonra_kayit_esastir() {
    let inner = fake_inner(vec![app("A.lnk"), app("B.lnk"), app("C.lnk")]);
    let stored = stored(&[]);
    let saves = Arc::new(AtomicUsize::new(0));
    let (load, save, s) = (Arc::clone(&stored), Arc::clone(&stored), Arc::clone(&saves));
    let service = OrderedPinnedAppsService::new(
        inner,
        move || load.lock().unwrap().clone(),
        move |o| {
            *save.lock().unwrap() = o.to_vec();
            s.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
        Some(Box::new(|| Ok(Some(favorites(8, &["C.lnk", "A.lnk", "B.lnk"]))))),
    );

    assert_eq!(strs(&["C.lnk", "A.lnk", "B.lnk"]), ids(&service.get_pinned_apps()));
    assert_eq!(strs(&["C.lnk", "A.lnk", "B.lnk"]), *stored.lock().unwrap());

    // Windows tarafı bir daha okunmaz: kayıt esas. Aynı sırada tekrar kaydedilmez.
    assert_eq!(strs(&["C.lnk", "A.lnk", "B.lnk"]), ids(&service.get_pinned_apps()));
    assert_eq!(1, saves.load(Ordering::SeqCst));
}

#[test]
fn service_kayit_okuma_hatasinda_klasor_sirasina_duser() {
    let inner = fake_inner(vec![app("B.lnk"), app("A.lnk")]);
    let stored = stored(&[]);
    let service = service(
        inner,
        &stored,
        Some(Box::new(|| Err(io::Error::other("kayıt defteri okunamadı")))),
    );

    assert_eq!(strs(&["B.lnk", "A.lnk"]), ids(&service.get_pinned_apps()));
}

#[test]
fn service_yeni_sabitlenen_sona_eklenir_kaldirilan_duser() {
    let inner = fake_inner(vec![app("A.lnk"), app("B.lnk")]);
    let stored = stored(&["B.lnk", "A.lnk"]);
    let service = service(inner.clone(), &stored, None);

    inner.apps.lock().unwrap().push(app("N.lnk"));
    assert_eq!(strs(&["B.lnk", "A.lnk", "N.lnk"]), ids(&service.get_pinned_apps()));

    inner.apps.lock().unwrap().retain(|a| a.title != "B");
    assert_eq!(strs(&["A.lnk", "N.lnk"]), ids(&service.get_pinned_apps()));
    assert_eq!(strs(&["A.lnk", "N.lnk"]), *stored.lock().unwrap());
}

#[test]
fn service_klasor_bos_okunursa_kayit_silinmez() {
    let inner = fake_inner(vec![app("A.lnk"), app("B.lnk")]);
    let stored = stored(&["B.lnk", "A.lnk"]);
    let service = service(inner.clone(), &stored, None);

    inner.apps.lock().unwrap().clear();
    assert!(service.get_pinned_apps().is_empty());
    assert_eq!(strs(&["B.lnk", "A.lnk"]), *stored.lock().unwrap());
}

#[test]
fn service_move_to_sirayi_degistirir_ve_yazar() {
    let inner = fake_inner(vec![app("A.lnk"), app("B.lnk"), app("C.lnk")]);
    let stored = stored(&["A.lnk", "B.lnk", "C.lnk"]);
    let service = service(inner, &stored, None);
    service.get_pinned_apps();

    service.move_to("A.lnk", 2);

    assert_eq!(strs(&["B.lnk", "C.lnk", "A.lnk"]), *stored.lock().unwrap());
    assert_eq!(strs(&["B.lnk", "C.lnk", "A.lnk"]), ids(&service.get_pinned_apps()));
}

#[test]
fn service_yazma_hatasi_oturumda_sira_korunur() {
    let inner = fake_inner(vec![app("A.lnk"), app("B.lnk")]);
    let service = OrderedPinnedAppsService::new(
        inner,
        || strs(&["A.lnk", "B.lnk"]),
        |_| Err(io::Error::other("disk dolu")),
        None,
    );

    service.move_to("A.lnk", 1);

    assert_eq!(strs(&["B.lnk", "A.lnk"]), ids(&service.get_pinned_apps()));
}

// --- DockReorderMathTests ---

#[test]
fn target_index_yuvaya_yuvarlar_ve_kirpar() {
    // (count, original, offset, slot, expected)
    let cases = [
        (5, 2, 0.0, 40.0, 2),
        (5, 2, 19.0, 40.0, 2),
        (5, 2, 20.0, 40.0, 3),
        (5, 2, 85.0, 40.0, 4),
        (5, 2, 500.0, 40.0, 4),
        (5, 2, -45.0, 40.0, 1),
        (5, 2, -500.0, 40.0, 0),
        (0, 0, 10.0, 40.0, 0),
    ];
    for (count, original, offset, slot, expected) in cases {
        assert_eq!(expected, DockReorderMath::target_index(count, original, offset, slot), "({count}, {original}, {offset}, {slot})");
    }
}

#[test]
fn shift_for_saga_surukleyince_aradakiler_sola_kayar() {
    // Sürüklenen 1 → hedef 3: 2 ve 3 sola.
    assert_eq!(0.0, DockReorderMath::shift_for(0, 1, 3, 40.0));
    assert_eq!(0.0, DockReorderMath::shift_for(1, 1, 3, 40.0));
    assert_eq!(-40.0, DockReorderMath::shift_for(2, 1, 3, 40.0));
    assert_eq!(-40.0, DockReorderMath::shift_for(3, 1, 3, 40.0));
    assert_eq!(0.0, DockReorderMath::shift_for(4, 1, 3, 40.0));
}

#[test]
fn shift_for_sola_surukleyince_aradakiler_saga_kayar() {
    // Sürüklenen 3 → hedef 1: 1 ve 2 sağa.
    assert_eq!(0.0, DockReorderMath::shift_for(0, 3, 1, 40.0));
    assert_eq!(40.0, DockReorderMath::shift_for(1, 3, 1, 40.0));
    assert_eq!(40.0, DockReorderMath::shift_for(2, 3, 1, 40.0));
    assert_eq!(0.0, DockReorderMath::shift_for(3, 3, 1, 40.0));
    assert_eq!(0.0, DockReorderMath::shift_for(4, 3, 1, 40.0));
}

#[test]
fn shift_for_hedef_ayniysa_kayma_yok() {
    for i in 0..4 {
        assert_eq!(0.0, DockReorderMath::shift_for(i, 2, 2, 40.0), "{i}");
    }
}

#[test]
fn exceeds_threshold_tiklama_ile_surukleyi_ayirir() {
    assert!(!DockReorderMath::exceeds_threshold(2.0, -3.0, 4.0));
    assert!(DockReorderMath::exceeds_threshold(-4.0, 0.0, 4.0));
    assert!(DockReorderMath::exceeds_threshold(0.0, 5.0, 4.0));
}

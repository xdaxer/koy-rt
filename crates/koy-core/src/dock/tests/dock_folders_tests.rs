use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::{strs, TempDir};
use crate::dock::{DockFolder, DockFolders, PinnedOrder};
use crate::settings::{AppSettings, DockItemsSettings, SettingsStore, ValueList};

fn folder(id: &str, name: &str, items: &[&str]) -> DockFolder {
    DockFolder::new(id, name, strs(items))
}

#[test]
fn create_from_drop_hedefin_konumunda_yeni_klasor_olusturur() {
    let order = strs(&["Code.lnk", "Obsidian.lnk", "Spotify.lnk"]);
    let folders: Vec<DockFolder> = vec![];

    let (new_order, new_folders) = DockFolders::create_from_drop(&order, &folders, "Spotify.lnk", "Obsidian.lnk", "Klasör");

    assert_eq!(1, new_folders.len());
    let folder = &new_folders[0];
    assert_eq!("Klasör", folder.name);
    assert_eq!(strs(&["Obsidian.lnk", "Spotify.lnk"]), folder.items);

    let token = PinnedOrder::to_folder_token(&folder.id);
    assert_eq!(vec!["Code.lnk".to_string(), token], new_order);
}

#[test]
fn create_from_drop_ayni_ogeye_suruklenirse_islem_yapilmaz() {
    let order = strs(&["Code.lnk", "Obsidian.lnk"]);
    let folders: Vec<DockFolder> = vec![];

    let (new_order, new_folders) = DockFolders::create_from_drop(&order, &folders, "Code.lnk", "Code.lnk", "Klasör");

    assert_eq!(order, new_order);
    assert!(new_folders.is_empty());
}

#[test]
fn create_from_drop_klasor_suruklenirse_ic_ice_klasore_izin_verilmez() {
    let order = strs(&["folder:f1", "Obsidian.lnk"]);
    let folders = vec![folder("f1", "Araçlar", &["A.lnk", "B.lnk"])];

    let (new_order, new_folders) = DockFolders::create_from_drop(&order, &folders, "folder:f1", "Obsidian.lnk", "Klasör");

    assert_eq!(order, new_order);
    assert_eq!(folders, new_folders);
}

#[test]
fn create_from_drop_hedef_klasorse_add_to_folder_a_yonlendirir() {
    let order = strs(&["folder:f1", "Spotify.lnk"]);
    let folders = vec![folder("f1", "Araçlar", &["A.lnk", "B.lnk"])];

    let (new_order, new_folders) = DockFolders::create_from_drop(&order, &folders, "Spotify.lnk", "folder:f1", "Klasör");

    assert_eq!(1, new_folders.len());
    assert_eq!(strs(&["A.lnk", "B.lnk", "Spotify.lnk"]), new_folders[0].items);
    assert_eq!(strs(&["folder:f1"]), new_order);
}

#[test]
fn add_to_folder_uygulamayi_ekler_ve_kok_siradan_cikarir() {
    let order = strs(&["Code.lnk", "folder:f1", "Slack.lnk"]);
    let folders = vec![folder("f1", "İletişim", &["Teams.lnk", "Discord.lnk"])];

    let (new_order, new_folders) = DockFolders::add_to_folder(&order, &folders, "Slack.lnk", "f1");

    assert_eq!(strs(&["Code.lnk", "folder:f1"]), new_order);
    assert_eq!(strs(&["Teams.lnk", "Discord.lnk", "Slack.lnk"]), new_folders[0].items);
}

#[test]
fn add_to_folder_zaten_varsa_cift_eklenmez() {
    let order = strs(&["folder:f1"]);
    let folders = vec![folder("f1", "İletişim", &["Teams.lnk", "Discord.lnk"])];

    let (_new_order, new_folders) = DockFolders::add_to_folder(&order, &folders, "Teams.lnk", "f1");

    assert_eq!(strs(&["Teams.lnk", "Discord.lnk"]), new_folders[0].items);
}

#[test]
fn remove_from_folder_iki_veya_daha_fazla_kalirsa_klasor_korunur() {
    let order = strs(&["folder:f1", "Terminal.lnk"]);
    let folders = vec![folder("f1", "Geliştirme", &["VS.lnk", "Code.lnk", "Rider.lnk"])];

    // "Code.lnk" çıkarılıp 0. indekse yerleştiriliyor
    let (new_order, new_folders) = DockFolders::remove_from_folder(&order, &folders, "f1", "Code.lnk", 0);

    assert_eq!(1, new_folders.len());
    assert_eq!(strs(&["VS.lnk", "Rider.lnk"]), new_folders[0].items);
    assert_eq!(strs(&["Code.lnk", "folder:f1", "Terminal.lnk"]), new_order);
}

#[test]
fn remove_from_folder_tek_oge_kaldiginda_klasor_kendiliginden_dagilir() {
    let order = strs(&["Terminal.lnk", "folder:f1", "Browser.lnk"]);
    let folders = vec![folder("f1", "Çift", &["A.lnk", "B.lnk"])];

    // "B.lnk" çıkarılıp en sona (indeks 99) yerleştiriliyor
    let (new_order, new_folders) = DockFolders::remove_from_folder(&order, &folders, "f1", "B.lnk", 99);

    // Klasör dağıldı: folders boş kalmalı
    assert!(new_folders.is_empty());
    // Kalan tek öğe "A.lnk" klasörün yerine geçer, çıkarılan "B.lnk" sona eklenir
    assert_eq!(strs(&["Terminal.lnk", "A.lnk", "Browser.lnk", "B.lnk"]), new_order);
}

#[test]
fn remove_from_folder_sifir_oge_kalirsa_klasor_silinir() {
    let order = strs(&["folder:f1"]);
    let folders = vec![folder("f1", "Tekil", &["A.lnk"])];

    let (new_order, new_folders) = DockFolders::remove_from_folder(&order, &folders, "f1", "A.lnk", 0);

    assert!(new_folders.is_empty());
    assert_eq!(strs(&["A.lnk"]), new_order);
}

#[test]
fn dissolve_klasor_belirtecini_ogeleriyle_degistirir() {
    let order = strs(&["Code.lnk", "folder:f1", "Spotify.lnk"]);
    let folders = vec![folder("f1", "Araçlar", &["Notepad.lnk", "Calc.lnk", "Paint.lnk"])];

    let (new_order, new_folders) = DockFolders::dissolve(&order, &folders, "f1");

    assert!(new_folders.is_empty());
    assert_eq!(strs(&["Code.lnk", "Notepad.lnk", "Calc.lnk", "Paint.lnk", "Spotify.lnk"]), new_order);
}

#[test]
fn dissolve_olmayan_klasorde_degisiklik_yapmaz() {
    let order = strs(&["Code.lnk"]);
    let folders = vec![folder("f1", "Araçlar", &["A.lnk", "B.lnk"])];

    let (new_order, new_folders) = DockFolders::dissolve(&order, &folders, "olmayan");

    assert_eq!(order, new_order);
    assert_eq!(folders, new_folders);
}

#[test]
fn rename_gecerli_yeni_adi_kaydeder() {
    let folders = vec![folder("f1", "Eski Ad", &["A.lnk", "B.lnk"])];

    let result = DockFolders::rename(&folders, "f1", "Yeni Ad");

    assert_eq!("Yeni Ad", result[0].name);
}

#[test]
fn rename_bos_veya_gecersiz_metinde_eski_adi_korur() {
    // C# satırlarından biri `null`; Rust imzası `&str` aldığından null yerine boş metin denenir (aynı yol: Trim → boş).
    for invalid_name in ["", "   "] {
        let folders = vec![folder("f1", "Sabit Ad", &["A.lnk", "B.lnk"])];

        let result = DockFolders::rename(&folders, "f1", invalid_name);

        assert_eq!("Sabit Ad", result[0].name, "{invalid_name:?}");
    }
}

#[test]
fn reorder_in_folder_indekse_gore_siralama() {
    let folders = vec![folder("f1", "Klasör", &["A.lnk", "B.lnk", "C.lnk"])];

    let result = DockFolders::reorder_in_folder(&folders, "f1", 0, 2);

    assert_eq!(strs(&["B.lnk", "C.lnk", "A.lnk"]), result[0].items);
}

#[test]
fn reorder_in_folder_oge_adina_gore_siralama() {
    let folders = vec![folder("f1", "Klasör", &["A.lnk", "B.lnk", "C.lnk"])];

    let result = DockFolders::reorder_in_folder_by_item(&folders, "f1", "A.lnk", 2);

    assert_eq!(strs(&["B.lnk", "C.lnk", "A.lnk"]), result[0].items);
}

#[test]
fn next_default_name_ardisik_ve_eksik_dolduran_numaralandirma() {
    let empty: Vec<DockFolder> = vec![];
    assert_eq!("Klasör", DockFolders::next_default_name(&empty, "Klasör"));

    let one = vec![folder("1", "Klasör", &["A.lnk", "B.lnk"])];
    assert_eq!("Klasör 2", DockFolders::next_default_name(&one, "Klasör"));

    let two = vec![folder("1", "Klasör", &["A.lnk", "B.lnk"]), folder("2", "Klasör 2", &["C.lnk", "D.lnk"])];
    assert_eq!("Klasör 3", DockFolders::next_default_name(&two, "Klasör"));

    // "Klasör 2" silindiyse tekrar 2 seçilmeli
    let gap = vec![folder("1", "Klasör", &["A.lnk", "B.lnk"]), folder("3", "Klasör 3", &["C.lnk", "D.lnk"])];
    assert_eq!("Klasör 2", DockFolders::next_default_name(&gap, "Klasör"));
}

#[test]
fn reconcile_silinmis_kisayollari_klasorlerden_ayiklar() {
    let order = strs(&["folder:f1", "Code.lnk"]);
    let folders = vec![folder("f1", "Araçlar", &["A.lnk", "B.lnk", "Silindi.lnk"])];
    let available = strs(&["A.lnk", "B.lnk", "Code.lnk"]);

    let (new_order, new_folders) = DockFolders::reconcile(&order, &folders, &available);

    assert_eq!(1, new_folders.len());
    assert_eq!(strs(&["A.lnk", "B.lnk"]), new_folders[0].items);
    assert_eq!(strs(&["folder:f1", "Code.lnk"]), new_order);
}

#[test]
fn reconcile_tek_oge_suruklenince_klasor_otomatik_dagilir() {
    let order = strs(&["folder:f1", "Code.lnk"]);
    let folders = vec![folder("f1", "Araçlar", &["A.lnk", "Silindi.lnk"])];
    let available = strs(&["A.lnk", "Code.lnk"]);

    let (new_order, new_folders) = DockFolders::reconcile(&order, &folders, &available);

    assert!(new_folders.is_empty());
    // Klasör dağıldı; A.lnk klasörün yuvasına yerleşti
    assert_eq!(strs(&["A.lnk", "Code.lnk"]), new_order);
}

#[test]
fn reconcile_klasor_icindeki_uygulamalar_kok_duzeyde_tekrar_gorunmez() {
    let order = strs(&["A.lnk", "folder:f1", "B.lnk", "Code.lnk"]);
    let folders = vec![folder("f1", "İletişim", &["Teams.lnk", "Discord.lnk"])];
    let available = strs(&["A.lnk", "Teams.lnk", "Discord.lnk", "Code.lnk"]);

    let (new_order, new_folders) = DockFolders::reconcile(&order, &folders, &available);

    assert_eq!(1, new_folders.len());
    // Teams.lnk ve Discord.lnk klasörde olduğu için kök dock sırasında olmamalı
    assert_eq!(strs(&["A.lnk", "folder:f1", "Code.lnk"]), new_order);
    assert!(!new_order.contains(&"Teams.lnk".to_string()));
    assert!(!new_order.contains(&"Discord.lnk".to_string()));
}

fn hash_of(f: &DockFolder) -> u64 {
    let mut h = DefaultHasher::new();
    f.hash(&mut h);
    h.finish()
}

#[test]
fn dock_folder_deger_tabanli_esitlik_ve_klonlama() {
    let f1 = folder("f1", "Test", &["A.lnk", "B.lnk"]);
    let f2 = folder("f1", "Test", &["A.lnk", "B.lnk"]);
    let f3 = folder("f1", "Farklı", &["A.lnk", "B.lnk"]);

    assert_eq!(f1, f2);
    assert_eq!(hash_of(&f1), hash_of(&f2));
    assert_ne!(f1, f3);

    // 'with' ifadesi Items referansını kopyalamalı
    let mut copy = DockFolder { name: "Yeni".to_string(), ..f1.clone() };
    copy.items.push("C.lnk".to_string());

    assert_eq!(2, f1.items.len());
    assert_eq!(3, copy.items.len());
}

#[test]
fn pinned_order_reconcile_klasor_belirteclerini_korur() {
    let saved = strs(&["folder:f1", "A.lnk", "folder:f2"]);
    let current_ids = strs(&["A.lnk", "B.lnk"]);

    let result = PinnedOrder::reconcile(&saved, &current_ids);

    // folder:f1 ve folder:f2 düşmemeli, B.lnk sona eklenmeli
    assert_eq!(strs(&["folder:f1", "A.lnk", "folder:f2", "B.lnk"]), result);
}

#[test]
fn pinned_order_reconcile_sifir_oge_kalan_klasor_docktan_silinir() {
    let saved = strs(&["folder:f1", "A.lnk"]);
    let folders = vec![folder("f1", "Araçlar", &["Silinen1.lnk", "Silinen2.lnk"])];
    let current_ids = strs(&["A.lnk"]);

    let (order, remaining_folders) = PinnedOrder::reconcile_with_folders(&saved, &current_ids, &folders);

    assert!(remaining_folders.is_empty());
    assert_eq!(strs(&["A.lnk"]), order);
    assert!(!order.contains(&"folder:f1".to_string()));
}

#[test]
fn settings_store_dock_folders_serilestirme_ve_geriye_uyumluluk_testi() {
    let dir = TempDir::new("koy-dockfolder-test-");
    let settings_path = dir.settings_path();
    let store = SettingsStore::new(settings_path.clone());

    // 1. Serileştirme ve geri yükleme (roundtrip)
    let original = AppSettings {
        dock_items: DockItemsSettings {
            folders: ValueList::from(vec![
                folder("f1", "Araçlar", &["calc.lnk", "notepad.lnk"]),
                folder("f2", "Medya", &["spotify.lnk", "vlc.lnk"]),
            ]),
            ..Default::default()
        },
        ..Default::default()
    };

    store.save(&original).unwrap();
    let restored = store.load();

    assert_eq!(original, restored);
    assert_eq!(2, restored.dock_items.folders.len());
    assert_eq!("Araçlar", restored.dock_items.folders[0].name);
    assert_eq!(strs(&["calc.lnk", "notepad.lnk"]), restored.dock_items.folders[0].items);

    // 2. Eski ayar dosyasında DockFolders alanı yoksa boş liste olarak yüklenmeli
    std::fs::write(&settings_path, r#"{ "DockEnabled": true }"#).unwrap();
    let legacy = store.load();
    assert!(legacy.dock_items.folders.is_empty());
}

use super::strs;
use crate::dock::{DockItemEntry, DockItemKind, DockItemRules, DockItemSpec};

fn e(kind: &str, id: &str, command: &str) -> DockItemEntry {
    DockItemEntry::new(kind, id, command)
}

fn k(kind: &str) -> DockItemEntry {
    e(kind, "", "")
}

fn normalize(entries: &[DockItemEntry]) -> Vec<DockItemSpec> {
    DockItemRules::normalize(entries, DockItemRules::MAX_ITEM_COUNT)
}

fn kinds(specs: &[DockItemSpec]) -> Vec<DockItemKind> {
    specs.iter().map(|s| s.kind).collect()
}

fn ids(specs: &[DockItemSpec]) -> Vec<String> {
    specs.iter().map(|s| s.id.clone()).collect()
}

fn specs(ids: &[&str]) -> Vec<DockItemSpec> {
    ids.iter().map(|id| DockItemSpec::new(DockItemKind::App, *id, "")).collect()
}

#[test]
fn try_parse_kind_bilinen_turleri_tanir() {
    let cases = [
        ("app", DockItemKind::App),
        ("Folder", DockItemKind::Folder),
        ("  SEPARATOR ", DockItemKind::Separator),
        ("spacer", DockItemKind::Spacer),
        ("clock", DockItemKind::Clock),
        ("trash", DockItemKind::Trash),
        ("custom", DockItemKind::Custom),
    ];
    for (text, expected) in cases {
        assert_eq!(Some(expected), DockItemRules::try_parse_kind(Some(text)), "{text:?}");
    }
}

#[test]
fn try_parse_kind_bilinmeyen_metni_reddeder() {
    for text in [Some("battery"), Some("5"), Some(""), None] {
        assert_eq!(None, DockItemRules::try_parse_kind(text), "{text:?}");
    }
}

#[test]
fn normalize_bilinmeyen_turu_atar_ve_sirayi_korur() {
    let result = normalize(&[e("app", "A.lnk", ""), k("battery"), k("clock"), e("hologram", "x", ""), e("app", "B.lnk", "")]);

    assert_eq!(vec![DockItemKind::App, DockItemKind::Clock, DockItemKind::App], kinds(&result));
    assert_eq!(strs(&["A.lnk", "Clock", "B.lnk"]), ids(&result));
}

#[test]
fn normalize_bos_kimlikli_uygulama_ve_klasoru_atar() {
    let result = normalize(&[e("app", "   ", ""), e("folder", "", ""), k("app")]);

    assert!(result.is_empty());
}

#[test]
fn normalize_komutsuz_ya_da_cok_uzun_ozel_dugmeyi_atar() {
    let too_long = "x".repeat(DockItemRules::MAX_COMMAND_LENGTH as usize + 1);

    let result = normalize(&[
        e("custom", "c1", ""),
        e("custom", "", "notepad"),
        e("custom", "c2", &too_long),
        e("custom", "c3", "https://example.com"),
    ]);

    assert_eq!(1, result.len());
    assert_eq!(DockItemKind::Custom, result[0].kind);
    assert_eq!("c3", result[0].id);
}

#[test]
fn normalize_ozel_komutu_oldugu_gibi_metin_olarak_saklar() {
    // Core komutu çalıştırmaz; yalnız baştaki ve sondaki boşluğu temizleyip saklar.
    let result = normalize(&[e("custom", "c1", "  calc.exe  ")]);

    assert_eq!(1, result.len());
    assert_eq!("calc.exe", result[0].command);
}

#[test]
fn normalize_uygulama_kimligini_buyuk_kucuk_harf_duyarsiz_tekillestirir() {
    let result = normalize(&[e("app", "Obsidian.lnk", ""), e("app", "obsidian.LNK", ""), e("app", "Code.lnk", "")]);

    assert_eq!(strs(&["Obsidian.lnk", "Code.lnk"]), ids(&result));
}

#[test]
fn normalize_ayni_kimlik_farkli_turlerde_cakismaz() {
    let result = normalize(&[e("app", "x", ""), e("folder", "x", "")]);

    assert_eq!(vec![DockItemKind::App, DockItemKind::Folder], kinds(&result));
}

#[test]
fn normalize_saat_ve_cop_kutusunu_en_fazla_bir_kez_tutar() {
    let result = normalize(&[k("clock"), k("trash"), e("clock", "baska", ""), k("trash")]);

    assert_eq!(vec![DockItemKind::Clock, DockItemKind::Trash], kinds(&result));
    assert_eq!(strs(&["Clock", "Trash"]), ids(&result));
}

#[test]
fn normalize_ayirici_ve_bosluk_tekrarlanabilir() {
    let result = normalize(&[k("separator"), k("spacer"), k("separator"), k("spacer")]);

    assert_eq!(
        vec![DockItemKind::Separator, DockItemKind::Spacer, DockItemKind::Separator, DockItemKind::Spacer],
        kinds(&result)
    );
}

#[test]
fn normalize_ozel_dugme_kimligini_tekillestirir() {
    let result = normalize(&[e("custom", "c1", "a"), e("custom", "C1", "b")]);

    assert_eq!(1, result.len());
    assert_eq!("a", result[0].command);
}

#[test]
fn normalize_sinir_asilinca_ilk_gelenleri_tutar() {
    let entries: Vec<DockItemEntry> = (0..10).map(|i| e("app", &format!("app{i}.lnk"), "")).collect();

    let result = DockItemRules::normalize(&entries, 4);

    assert_eq!(strs(&["app0.lnk", "app1.lnk", "app2.lnk", "app3.lnk"]), ids(&result));
}

#[test]
fn normalize_varsayilan_sinir_en_fazla_max_item_count_tutar() {
    let entries: Vec<DockItemEntry> = (0..DockItemRules::MAX_ITEM_COUNT + 5).map(|_| k("spacer")).collect();

    assert_eq!(DockItemRules::MAX_ITEM_COUNT as usize, normalize(&entries).len());
}

#[test]
fn normalize_bos_girdi_ve_null_kaydi_guvenle_atlar() {
    assert!(normalize(&[]).is_empty());

    // C# testi `null` kaydı da verir; Rust'ta `&[DockItemEntry]` null taşıyamaz, yalnız geçerli kayıt denenir.
    let result = normalize(&[k("spacer")]);

    assert_eq!(1, result.len());
    assert_eq!(DockItemKind::Spacer, result[0].kind);
}

#[test]
fn move_ogeyi_ileri_tasir() {
    let items = specs(&["A", "B", "C", "D"]);

    let result = DockItemRules::move_item(&items, 0, 2);

    assert_eq!(strs(&["B", "C", "A", "D"]), ids(&result));
}

#[test]
fn move_ogeyi_geri_tasir() {
    let items = specs(&["A", "B", "C", "D"]);

    let result = DockItemRules::move_item(&items, 3, 1);

    assert_eq!(strs(&["A", "D", "B", "C"]), ids(&result));
}

#[test]
fn move_hedef_sinirin_disindaysa_uca_kenetlenir() {
    let items = specs(&["A", "B", "C"]);

    assert_eq!(strs(&["B", "C", "A"]), ids(&DockItemRules::move_item(&items, 0, 99)));
    assert_eq!(strs(&["C", "A", "B"]), ids(&DockItemRules::move_item(&items, 2, -5)));
}

#[test]
fn move_gecersiz_kaynak_ya_da_ayni_konum_listeyi_degistirmez() {
    let items = specs(&["A", "B"]);

    assert_eq!(strs(&["A", "B"]), ids(&DockItemRules::move_item(&items, 7, 0)));
    assert_eq!(strs(&["A", "B"]), ids(&DockItemRules::move_item(&items, 1, 1)));
    assert!(DockItemRules::move_item(&[], 0, 0).is_empty());
}

#[test]
fn move_girdi_listesini_degistirmez() {
    let items = specs(&["A", "B", "C"]);

    DockItemRules::move_item(&items, 0, 2);

    assert_eq!(strs(&["A", "B", "C"]), ids(&items));
}

#[test]
fn from_pinned_order_sirayi_ve_klasorleri_korur() {
    let entries = DockItemRules::from_pinned_order(&strs(&["Obsidian.lnk", "folder:f1", "Code.lnk"]));

    let result = normalize(&entries);

    assert_eq!(vec![DockItemKind::App, DockItemKind::Folder, DockItemKind::App], kinds(&result));
    assert_eq!(strs(&["Obsidian.lnk", "f1", "Code.lnk"]), ids(&result));
}

#[test]
fn to_pinned_order_pinned_order_ile_gidis_donus_yapar() {
    let original = strs(&["Obsidian.lnk", "folder:f1", "Code.lnk"]);

    let round_trip = DockItemRules::to_pinned_order(&normalize(&DockItemRules::from_pinned_order(&original)));

    assert_eq!(original, round_trip);
}

#[test]
fn to_pinned_order_uygulama_ve_klasor_disindakileri_yazmaz() {
    let items = vec![
        DockItemSpec::new(DockItemKind::App, "A.lnk", ""),
        DockItemSpec::new(DockItemKind::Separator, "", ""),
        DockItemSpec::new(DockItemKind::Custom, "c1", "calc"),
        DockItemSpec::new(DockItemKind::Clock, "Clock", ""),
        DockItemSpec::new(DockItemKind::Folder, "f1", ""),
    ];

    assert_eq!(strs(&["A.lnk", "folder:f1"]), DockItemRules::to_pinned_order(&items));
}

#[test]
fn normalize_dosya_adi_olamayan_uygulama_kimligini_atar() {
    // "folder:f1" uygulaması PinnedOrder'a yazılınca klasöre dönüşürdü.
    for id in ["folder:f1", "FOLDER:f1", "a/b.lnk"] {
        assert!(normalize(&[e("app", id, "")]).is_empty(), "{id}");
    }
}

#[test]
fn resolve_kayit_yoksa_pinned_order_den_turetir_ve_sirayi_korur() {
    let result = DockItemRules::resolve(&[], &strs(&["Obsidian.lnk", "folder:f1", "Code.lnk"]), DockItemRules::MAX_ITEM_COUNT);

    assert_eq!(vec![DockItemKind::App, DockItemKind::Folder, DockItemKind::App], kinds(&result));
    assert_eq!(strs(&["Obsidian.lnk", "f1", "Code.lnk"]), ids(&result));
}

#[test]
fn resolve_kayit_varsa_pinned_order_i_yok_sayar() {
    let result = DockItemRules::resolve(&[k("clock")], &strs(&["Obsidian.lnk"]), DockItemRules::MAX_ITEM_COUNT);

    assert_eq!(1, result.len());
    assert_eq!(DockItemKind::Clock, result[0].kind);
}

#[test]
fn resolve_kayitlar_var_ama_hepsi_gecersizse_bos_doner() {
    assert!(DockItemRules::resolve(&[k("battery")], &strs(&["Obsidian.lnk"]), DockItemRules::MAX_ITEM_COUNT).is_empty());
}

#[test]
fn resolve_gecis_de_ust_siniri_uygular() {
    let pinned: Vec<String> = (0..DockItemRules::MAX_ITEM_COUNT + 3).map(|i| format!("app{i}.lnk")).collect();

    let result = DockItemRules::resolve(&[], &pinned, DockItemRules::MAX_ITEM_COUNT);

    let expected: Vec<String> = pinned.iter().take(DockItemRules::MAX_ITEM_COUNT as usize).cloned().collect();
    assert_eq!(expected, ids(&result));
}

//! `Settings/SettingsSearchTests.cs`.

use crate::settings::*;

fn entry(key: &str, title: &str, description: &str, keywords: &[&str]) -> SettingSearchEntry {
    SettingSearchEntry {
        key: key.to_string(),
        page: "page".to_string(),
        title: title.to_string(),
        description: description.to_string(),
        keywords: keywords.iter().map(|k| k.to_string()).collect(),
    }
}

fn dark_theme() -> SettingSearchEntry {
    entry("Appearance.Theme", "Karanlık tema", "Koy penceresinin rengi", &["koyu", "dark"])
}

fn dock() -> SettingSearchEntry {
    entry("Dock.Enabled", "Dock", "Ekranın alt kenarındaki çubuk", &[])
}

fn hide_taskbar() -> SettingSearchEntry {
    entry("Dock.HideWindowsTaskbar", "Görev çubuğunu gizle", "Dock açıkken Windows görev çubuğu saklanır", &["taskbar"])
}

fn search(entries: &[SettingSearchEntry], query: Option<&str>) -> Vec<SettingSearchHit> {
    SettingsSearch::search(entries, query, i32::MAX)
}

fn keys(hits: &[SettingSearchHit]) -> Vec<&str> {
    hits.iter().map(|h| h.entry.key.as_str()).collect()
}

#[test]
fn bos_veya_bosluk_sorgu_hic_sonuc_dondurmez() {
    assert!(search(&[dark_theme()], None).is_empty());
    assert!(search(&[dark_theme()], Some("   ")).is_empty());
}

#[test]
fn turkce_harfler_farksizdir_ve_buyuk_kucuk_harf_farketmez() {
    let hits = search(&[dark_theme()], Some("KARANLIK"));
    assert_eq!(1, hits.len());
    assert_eq!(1, search(&[dark_theme()], Some("karanlik")).len());
    assert_eq!(1, search(&[entry("x", "Işık", "", &[])], Some("isik")).len());
}

#[test]
fn her_parca_satirda_gecmelidir() {
    // "dock" başlıkta, "gizle" yalnız açıklamada; ikisi de olan satır bulunur.
    let only_dock = entry("a", "Dock", "başka bir şey", &[]);
    let both = entry("b", "Dock", "gizle", &[]);

    let hits = search(&[only_dock, both], Some("dock gizle"));

    assert_eq!(1, hits.len());
    assert_eq!("b", hits[0].entry.key);
}

#[test]
fn baslik_eslesmesi_aciklama_eslesmesinden_yuksek_puan_alir() {
    let in_title = entry("title", "Gizle", "", &[]);
    let in_description = entry("desc", "Başka", "gizle", &[]);

    let hits = search(&[in_description, in_title], Some("gizle"));

    assert_eq!(vec!["title", "desc"], keys(&hits));
    assert!(hits[0].score > hits[1].score);
}

#[test]
fn anahtar_sozcuk_ile_bulunur() {
    let hits = search(&[hide_taskbar()], Some("taskbar"));

    assert_eq!(1, hits.len());
    assert_eq!("Dock.HideWindowsTaskbar", hits[0].entry.key);
}

#[test]
fn basligi_sorguyla_baslayan_satir_one_gecer() {
    let starts_with = entry("start", "Dock konumu", "", &[]);
    let contains = entry("contains", "Yeni dock", "", &[]);

    let hits = search(&[contains, starts_with], Some("dock"));

    assert_eq!("start", hits[0].entry.key);
}

#[test]
fn eslesmeyen_sorgu_bos_doner() {
    assert!(search(&[dock(), hide_taskbar()], Some("bulunmayan")).is_empty());
}

#[test]
fn limit_sonuc_sayisini_sinirlar_ve_sira_korunur() {
    let entries = [entry("1", "ayar", "", &[]), entry("2", "ayar", "", &[]), entry("3", "ayar", "", &[])];

    let hits = SettingsSearch::search(&entries, Some("ayar"), 2);

    assert_eq!(vec!["1", "2"], keys(&hits));
}

#[test]
fn negatif_limit_reddedilir() {
    let result = std::panic::catch_unwind(|| SettingsSearch::search(&[dock()], Some("dock"), -1));
    assert!(result.is_err());
}

#[test]
fn bolunmez_bosluk_da_sorgu_parcalarini_ayirir() {
    let hits = search(&[entry("b", "Dock", "gizle", &[])], Some("dock gizle"));
    assert_eq!(1, hits.len());
}

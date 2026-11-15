use chrono::{Duration, NaiveDate, NaiveDateTime};

use crate::dock::{RecentEntry, RecentFilesPolicy};

fn utc(y: i32, m: u32, d: u32, h: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, 0, 0).unwrap()
}

fn e(file: &str, minutes_ago: i64) -> RecentEntry {
    RecentEntry::new(format!(r"C:\Recent\{file}"), utc(2026, 11, 6, 12) - Duration::minutes(minutes_ago))
}

const MAX: i32 = RecentFilesPolicy::MAX_ITEMS;

#[test]
fn select_yeniden_eskiye_siralar_ve_uzantiyi_atar() {
    let result = RecentFilesPolicy::select(&[e("eski.docx.lnk", 60), e("yeni.pdf.lnk", 1), e("orta.txt.lnk", 10)], MAX);

    let names: Vec<&str> = result.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(vec!["yeni.pdf", "orta.txt", "eski.docx"], names);
    assert_eq!(r"C:\Recent\yeni.pdf.lnk", result[0].open_path);
}

#[test]
fn select_en_cok_on_oge_verir() {
    let entries: Vec<RecentEntry> = (0..25).map(|i| e(&format!("dosya{i}.txt.lnk"), i)).collect();

    assert_eq!(10, RecentFilesPolicy::select(&entries, MAX).len());
    assert_eq!(10, RecentFilesPolicy::select(&entries, 99).len());
    assert_eq!(3, RecentFilesPolicy::select(&entries, 3).len());
    assert!(RecentFilesPolicy::select(&entries, 0).is_empty());
    assert!(RecentFilesPolicy::select(&entries, -4).is_empty());
}

#[test]
fn select_kisayol_olmayanlari_ve_bos_adlari_atar() {
    let now = utc(2026, 11, 6, 12);
    let result = RecentFilesPolicy::select(
        &[e("desktop.ini", 1), e("not.txt", 2), e(".lnk", 3), e("gercek.txt.lnk", 4), RecentEntry::new("", now)],
        MAX,
    );

    let names: Vec<&str> = result.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(vec!["gercek.txt"], names);
}

#[test]
fn select_ayni_adlilardan_en_yeniyi_tutar() {
    let older = RecentEntry::new(r"C:\Recent\a\rapor.pdf.lnk", utc(2026, 1, 1, 0));
    let newer = RecentEntry::new(r"C:\Recent\b\RAPOR.pdf.lnk", utc(2026, 2, 1, 0));

    let result = RecentFilesPolicy::select(&[older, newer.clone()], MAX);

    assert_eq!(1, result.len());
    assert_eq!(newer.shortcut_path, result[0].open_path);
}

#[test]
fn select_bos_girdide_bos_liste() {
    assert!(RecentFilesPolicy::select(&[], MAX).is_empty());
}

use chrono::{NaiveDate, Timelike};

use super::strs;
use crate::clock::Culture;
use crate::dock::{DockClockTick, DockCustomCommand, DockItemKind, DockItemLayout, DockItemRules, DockItemSpec};
use crate::notch::ClockTick;
use crate::time::TimeSpan;

fn app(id: &str) -> DockItemSpec {
    DockItemSpec::new(DockItemKind::App, id, "")
}

fn folder(id: &str) -> DockItemSpec {
    DockItemSpec::new(DockItemKind::Folder, id, "")
}

fn sep() -> DockItemSpec {
    DockItemSpec::new(DockItemKind::Separator, "", "")
}

fn clock() -> DockItemSpec {
    DockItemSpec::new(DockItemKind::Clock, "Clock", "")
}

fn describe(items: &[DockItemSpec]) -> String {
    items
        .iter()
        .map(|i| match i.kind {
            DockItemKind::App => i.id.clone(),
            DockItemKind::Folder => format!("F{}", i.id),
            k => k.to_string(),
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn merge_ek_ogeleri_yerinde_tutar() {
    let items = [app("a.lnk"), sep(), app("b.lnk"), clock()];
    let merged = DockItemLayout::merge(&items, &strs(&["a.lnk", "b.lnk"]));
    assert_eq!("a.lnk,Separator,b.lnk,Clock", describe(&merged));
}

#[test]
fn merge_artik_olmayan_uygulamayi_dusurur() {
    let items = [app("a.lnk"), sep(), app("b.lnk")];
    let merged = DockItemLayout::merge(&items, &strs(&["b.lnk"]));
    assert_eq!("Separator,b.lnk", describe(&merged));
}

#[test]
fn merge_yeni_uygulamayi_son_uygulamanin_arkasina_ekler() {
    let items = [app("a.lnk"), clock()];
    let merged = DockItemLayout::merge(&items, &strs(&["a.lnk", "c.lnk", "folder:x"]));
    assert_eq!("a.lnk,c.lnk,Fx,Clock", describe(&merged));
}

#[test]
fn merge_uygulama_yoksa_yenileri_basa_ekler() {
    let merged = DockItemLayout::merge(&[clock()], &strs(&["a.lnk"]));
    assert_eq!("a.lnk,Clock", describe(&merged));
}

#[test]
fn merge_buyuk_kucuk_harf_duyarsiz_ve_tekrar_yok() {
    let items = [app("A.lnk"), app("a.LNK")];
    let merged = DockItemLayout::merge(&items, &strs(&["a.lnk"]));
    assert_eq!(1, merged.len());
}

#[test]
fn merge_klasoru_belirecinden_eslestirir() {
    let merged = DockItemLayout::merge(&[folder("x"), sep()], &strs(&["folder:x"]));
    assert_eq!("Fx,Separator", describe(&merged));
}

#[test]
fn add_sona_ekler_ve_tekil_ogeyi_ikinci_kez_eklemez() {
    let list = DockItemLayout::add(&[app("a.lnk")], clock());
    assert_eq!("a.lnk,Clock", describe(&list));

    let again = DockItemLayout::add(&list, clock());
    assert_eq!("a.lnk,Clock", describe(&again));
}

#[test]
fn add_ayiriciyi_tekrarlar() {
    let list = DockItemLayout::add(&DockItemLayout::add(&[], sep()), sep());
    assert_eq!(2, list.len());
}

#[test]
fn add_siniri_asmaz() {
    let full: Vec<DockItemSpec> = (0..DockItemRules::MAX_ITEM_COUNT).map(|i| app(&format!("{i}.lnk"))).collect();
    let result = DockItemLayout::add(&full, sep());
    assert_eq!(DockItemRules::MAX_ITEM_COUNT as usize, result.len());
}

#[test]
fn add_bos_komutlu_ozel_dugmeyi_reddeder() {
    let result = DockItemLayout::add(&[app("a.lnk")], DockItemSpec::new(DockItemKind::Custom, "abc", ""));
    assert_eq!(1, result.len());
}

#[test]
fn remove_at_gecersiz_indekste_degistirmez() {
    let items = [app("a.lnk"), sep()];
    assert_eq!(2, DockItemLayout::remove_at(&items, 5).len());
    assert_eq!("a.lnk", describe(&DockItemLayout::remove_at(&items, 1)));
}

#[test]
fn to_stored_entries_ek_ogesiz_listede_bos_doner() {
    assert!(DockItemLayout::to_stored_entries(&[app("a.lnk")]).is_empty());
    let stored = DockItemLayout::to_stored_entries(&[app("a.lnk"), clock()]);
    assert_eq!(2, stored.len());
    assert_eq!("Clock", stored[1].kind);
}

#[test]
fn to_stored_entries_normalize_ile_gidis_donus_ayni_kalir() {
    let items = vec![
        app("a.lnk"),
        sep(),
        folder("x"),
        DockItemSpec::new(DockItemKind::Custom, "c1", r"C:\t.exe"),
        clock(),
    ];
    let round = DockItemRules::normalize(&DockItemLayout::to_stored_entries(&items), DockItemRules::MAX_ITEM_COUNT);
    assert_eq!(items, round);
}

#[test]
fn new_custom_id_bos_degil_ve_benzersiz() {
    let a = DockItemLayout::new_custom_id();
    assert!(!a.is_empty());
    assert_ne!(a, DockItemLayout::new_custom_id());
}

#[test]
fn move_ek_ogeyi_uygulamanin_otesine_tasir() {
    let items = [app("a.lnk"), app("b.lnk"), clock()];
    assert_eq!("a.lnk,Clock,b.lnk", describe(&DockItemRules::move_item(&items, 2, 1)));
}

#[test]
fn delay_to_next_minute_dakika_basina_hizalar() {
    let now = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_milli_opt(10, 15, 20, 500).unwrap();
    let delay = DockClockTick::delay_to_next_minute(now);
    assert_eq!(TimeSpan::from_millis(39500) + ClockTick::MARGIN, delay);
    assert_eq!(0, (now + delay.to_chrono()).second());
}

#[test]
fn delay_to_next_minute_hep_pozitif_ve_en_cok_bir_dakika_ve_pay() {
    for s in [0, 1, 30, 59] {
        let now = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap().and_hms_milli_opt(0, 0, s, 999).unwrap();
        let d = DockClockTick::delay_to_next_minute(now);
        assert!(d >= ClockTick::MINIMUM_DELAY && d <= TimeSpan::from_mins(1) + ClockTick::MARGIN, "s={s}: {d}");
    }
}

#[test]
fn saat_bicimi_kulturu_kullanir() {
    let now = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap().and_hms_opt(14, 5, 0).unwrap();
    assert_eq!("14:05", DockClockTick::format_time(now, &Culture::tr_tr()));
    assert!(DockClockTick::format_date(now, &Culture::en_us()).contains("12"));
}

// --- DockCustomCommandTests ---

#[test]
fn try_parse_gecerli_komutlari_ayirir() {
    // (text, target, args)
    let cases: [(&str, &str, Option<&str>); 6] = [
        (r"C:\Tools\a.exe", r"C:\Tools\a.exe", None),
        (r"C:\Program Files\App\a.exe", r"C:\Program Files\App\a.exe", None),
        ("\"C:\\Program Files\\a.exe\" --flag \"x y\"", r"C:\Program Files\a.exe", Some("--flag \"x y\"")),
        (r"\\server\share\run.cmd", r"\\server\share\run.cmd", None),
        ("https://example.com/a?b=1", "https://example.com/a?b=1", None),
        ("mailto:a@b.com", "mailto:a@b.com", None),
    ];
    for (text, target, args) in cases {
        let launch = DockCustomCommand::try_parse(Some(text)).unwrap_or_else(|| panic!("ayrıştırılamadı: {text}"));
        assert_eq!(target, launch.target, "{text}");
        assert_eq!(args.map(str::to_string), launch.arguments, "{text}");
    }
}

#[test]
fn try_parse_gecersiz_ya_da_riskli_komutlari_reddeder() {
    let cases: [Option<&str>; 19] = [
        None,
        Some(""),
        Some("   "),
        Some("notepad.exe"),
        Some(r"..\a.exe"),
        Some("shell:RecycleBinFolder"),
        Some("::{645FF040-5081-101B-9F08-00AA002F954E}"),
        Some("ms-msdt:/id PCWDiagnostic"),
        Some("javascript:alert(1)"),
        Some("file:///C:/a.exe"),
        Some(r"\\?\C:\a.exe"),
        Some(r"\\.\pipe\x"),
        Some("C:\\a.exe\r\ncalc"),
        Some("C:\\a.exe\0"),
        Some("\"C:\\a.exe"),
        Some("\"\" x"),
        Some("C:\\a|b.exe"),
        Some("\"https://example.com\" --x"),
        Some("https://exa mple.com"),
    ];
    for text in cases {
        assert!(DockCustomCommand::try_parse(text).is_none(), "{text:?}");
    }
}

#[test]
fn try_parse_cok_uzun_komutu_reddeder() {
    let text = format!(r"C:\{}", "a".repeat(DockItemRules::MAX_COMMAND_LENGTH as usize));
    assert!(DockCustomCommand::try_parse(Some(&text)).is_none());
}

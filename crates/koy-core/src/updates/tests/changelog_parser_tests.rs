//! Karşılığı: `koy/tests/Core.Tests/Updates/ChangelogParserTests.cs`.

use crate::updates::ChangelogParser;

const SAMPLE_CHANGELOG: &str = "\
# Değişiklik Günlüğü (Changelog)

Bu projedeki tüm önemli değişiklikler bu dosyada belgelenecektir.

---

## [0.2.0](https://github.com/koyhq/koy/compare/v0.1.1...v0.2.0) (2026-11-07)

Öne çıkanlar:
- **Dock klasörleri:** sabitli bir uygulamayı başka birinin üstüne sürükleyince klasör olur.
- **Tam genişlik dock:** 48 px Windows görev çubuğu ölçüsünde.
- **Küçük tepsi:** tepsi düğmesi dar bir ok olur.

### Eklenenler (Added)
- Dock klasörleri ve cam panel.
- Pil ve Bluetooth bildirimleri.

### Düzeltmeler (Fixed)
- Büyütülmüş pencereler yeniden boyutlanmıyor.
- Açık uygulamalar kaybolmuyor.

## [0.1.1](https://github.com/koyhq/koy/compare/v0.1.0...v0.1.1) (2026-11-06)

0.1.0'dan sonraki ilk büyük sürüm. Öne çıkanlar:
- Yeni çentik tasarımı: 7 sekme.
- Gözcü süreç ve görev çubuğu güvencesi.

### Düzeltmeler (Fixed)
- İkinci ekran çökmesi düzeltildi.";

const TR: &str = "## [0.3.0] (2026-11-09)\n\nÖne çıkanlar:\n- **dock:** ikon hizası\n";
const EN: &str = "## [0.3.0] (2026-11-09)\n\nHighlights:\n- **dock:** icon alignment\n";
const ONLY_OLD: &str = "## [0.2.0] (2026-11-07)\n\nHighlights:\n- old\n";

#[test]
fn extract_version_mevcut_surumu_dogru_ayiklar() {
    let notes = ChangelogParser::extract_version(Some(SAMPLE_CHANGELOG), "0.2.0", "en").unwrap();

    assert_eq!("0.2.0", notes.version);
    assert_eq!(Some("2026-11-07"), notes.date.as_deref());
    assert_eq!(Some("https://github.com/koyhq/koy/compare/v0.1.1...v0.2.0"), notes.compare_url.as_deref());
    assert_eq!(3, notes.highlights.len());
    assert!(notes.highlights[0].contains("Dock klasörleri"));
    assert_eq!(2, notes.sections.len());
    assert_eq!("Eklenenler (Added)", notes.sections[0].title);
    assert_eq!(2, notes.sections[0].items.len());
    assert_eq!("Düzeltmeler (Fixed)", notes.sections[1].title);
    assert_eq!(2, notes.sections[1].items.len());
}

#[test]
fn extract_version_v_harfli_ve_on_ekli_surumleri_esler() {
    let notes = ChangelogParser::extract_version(Some(SAMPLE_CHANGELOG), "v0.1.1+build123", "en").unwrap();

    assert_eq!("0.1.1", notes.version);
    assert_eq!(Some("2026-11-06"), notes.date.as_deref());
    assert_eq!(2, notes.highlights.len());
}

#[test]
fn extract_version_bulunmayan_surumde_null_dondurur() {
    assert!(ChangelogParser::extract_version(Some(SAMPLE_CHANGELOG), "9.9.9", "en").is_none());
}

#[test]
fn parse_latest_en_ustteki_surumu_verir() {
    let latest = ChangelogParser::parse_latest(Some(SAMPLE_CHANGELOG), "en").unwrap();

    assert_eq!("0.2.0", latest.version);
}

#[test]
fn get_all_versions_tum_surumleri_sirali_verir() {
    let versions = ChangelogParser::get_all_versions(Some(SAMPLE_CHANGELOG));

    assert_eq!(2, versions.len());
    assert_eq!("0.2.0", versions[0]);
    assert_eq!("0.1.1", versions[1]);
}

#[test]
fn read_embedded_changelog_gomulu_kaynagi_basariyla_okur() {
    let markdown = ChangelogParser::read_embedded_changelog("tr");
    let markdown = markdown.expect("gömülü günlük okunamadı");
    assert!(markdown.contains("Değişiklik Günlüğü"));

    let notes = ChangelogParser::parse_latest(Some(&markdown), "en").unwrap();
    assert!(!notes.version.is_empty());
}

#[test]
fn extract_version_surum_yoksa_son_surume_dusmez() {
    // Güncelleme penceresi yeni sürümün notunu ister; kurulu sürümün notları yerine null dönmeli.
    let md = "## [0.2.0] (2026-11-07)\r\n\r\nÖne çıkanlar:\r\n- **Eski:** not\r\n";
    assert!(ChangelogParser::extract_version(Some(md), "0.2.1", "en").is_none());
}

#[test]
fn extract_version_paket_notu_biciminu_okur() {
    // Yayın akışının --releaseNotes ile pakete koyduğu bölüm (CRLF, tek sürüm).
    let md = "## [0.2.1](https://github.com/koyhq/koy/compare/v0.2.0...v0.2.1) (2026-11-09)\r\n\r\nÖne çıkanlar:\r\n- **Yeni:** özellik\r\n\r\n### Düzeltilenler (Fixed)\r\n- bir şey\r\n";
    let notes = ChangelogParser::extract_version(Some(md), "0.2.1", "en").unwrap();
    assert!(!notes.highlights.is_empty());
}

#[test]
fn clean_item_duz_metne_cevirir() {
    let cases = [
        (
            "**dock:** sağ tık menüsüne atlama listesini ekle ([#32](https://github.com/koyhq/koy/issues/32)) ([9aa22ea](https://github.com/koyhq/koy/commit/9aa22ea9))",
            "Sağ tık menüsüne atlama listesini ekle",
        ),
        (
            "gizlilik ipucu metinleri ([#24](https://x/24), [#27](https://x/27), [#29](https://x/29)) ([890cf03](https://x/c))",
            "Gizlilik ipucu metinleri",
        ),
        ("**updater:** indirme ([#27](https://x/27))", "İndirme"),
        ("**Atlama listesi:** dock'ta sağ tık", "Atlama listesi: dock'ta sağ tık"),
        (
            "**Depo taşındı:** Koy artık [github.com/koyhq/koy](https://github.com/koyhq/koy) adresinde.",
            "Depo taşındı: Koy artık github.com/koyhq/koy adresinde.",
        ),
        ("`Ayarlar` sayfası", "Ayarlar sayfası"),
    ];
    for (input, expected) in cases {
        assert_eq!(Some(expected.to_string()), ChangelogParser::clean_item(input, "tr"), "girdi: {input}");
    }
}

#[test]
fn clean_item_site_kapsamini_gizler() {
    assert!(ChangelogParser::clean_item("**site:** hero çentiğini aç ([#33](https://x/33)) ([00708b3](https://x/c))", "en").is_none());
}

#[test]
fn extract_version_release_please_maddelerini_temizler() {
    let md = "## [0.2.1](https://github.com/koyhq/koy/compare/v0.2.0...v0.2.1) (2026-11-08)\r\n\r\nÖne çıkanlar:\r\n- **Hep havada:** kapsül havada kalır.\r\n\r\n\r\n### Eklenenler\r\n\r\n* **dock:** yüzen kapsüle Hep havada modunu ekle ([#31](https://x/31)) ([3c828bd](https://x/c))\r\n* **site:** hero çentiği ([#33](https://x/33))\r\n";
    let notes = ChangelogParser::extract_version(Some(md), "0.2.1", "en").unwrap();
    assert_eq!(vec!["Hep havada: kapsül havada kalır.".to_string()], notes.highlights);
    assert_eq!(1, notes.sections.len());
    assert_eq!(vec!["Yüzen kapsüle Hep havada modunu ekle".to_string()], notes.sections[0].items);
}

#[test]
fn read_embedded_changelog_returns_both_languages() {
    assert!(ChangelogParser::read_embedded_changelog("en").unwrap().contains("# Changelog"));
    assert!(ChangelogParser::read_embedded_changelog("tr").unwrap().contains("Değişiklik Günlüğü"));
}

#[test]
fn resolve_embedded_finds_shared_version_in_each_language() {
    let en = ChangelogParser::read_embedded_changelog("en");
    let version = ChangelogParser::get_all_versions(en.as_deref()).remove(0);
    assert_eq!(version, ChangelogParser::resolve_embedded(&version, "en").unwrap().version);
    assert_eq!(version, ChangelogParser::resolve_embedded(&version, "tr").unwrap().version);
}

#[test]
fn resolve_version_uses_selected_language() {
    for (language, expected) in [("tr", "İkon hizası"), ("en", "Icon alignment")] {
        let notes = ChangelogParser::resolve_version("0.3.0", language, |l| Some(if l == "tr" { TR } else { EN }.to_string()));
        assert_eq!(vec![expected.to_string()], notes.unwrap().highlights, "dil: {language}");
    }
}

#[test]
fn resolve_version_falls_back_to_english_when_turkish_is_missing() {
    for (language, expected) in [("tr", "Icon alignment")] {
        let notes = ChangelogParser::resolve_version("0.3.0", language, |l| Some(if l == "tr" { ONLY_OLD } else { EN }.to_string()));
        assert_eq!(vec![expected.to_string()], notes.unwrap().highlights, "dil: {language}");
    }
}

#[test]
fn resolve_version_falls_back_to_turkish_when_english_is_missing() {
    let notes = ChangelogParser::resolve_version("0.3.0", "en", |l| Some(if l == "tr" { TR } else { ONLY_OLD }.to_string()));
    assert_eq!(vec!["İkon hizası".to_string()], notes.unwrap().highlights);
}

#[test]
fn resolve_version_returns_null_when_no_language_has_the_version() {
    assert!(ChangelogParser::resolve_version("0.3.0", "en", |_| Some(ONLY_OLD.to_string())).is_none());
}

#[test]
fn clean_item_capitalizes_i_by_language() {
    assert_eq!(Some("Improve the dock".to_string()), ChangelogParser::clean_item("**dock:** improve the dock", "en"));
    assert_eq!(Some("İyileştir".to_string()), ChangelogParser::clean_item("**dock:** iyileştir", "tr"));
}

#[test]
fn select_feed_notes_picks_the_language_section() {
    for (language, expected_language, expected) in [("tr", "tr", "İkon hizası"), ("en", "en", "Icon alignment")] {
        let feed = format!("{EN}\n{}\n\n{TR}", ChangelogParser::TURKISH_NOTES_MARKER);

        let (markdown, actual_language) = ChangelogParser::select_feed_notes(Some(&feed), language);

        assert_eq!(expected_language, actual_language);
        assert_eq!(
            vec![expected.to_string()],
            ChangelogParser::extract_version(markdown.as_deref(), "0.3.0", actual_language).unwrap().highlights,
            "dil: {language}"
        );
    }
}

#[test]
fn select_feed_notes_without_marker_is_english() {
    let (markdown, language) = ChangelogParser::select_feed_notes(Some(EN), "tr");
    assert_eq!("en", language);
    assert_eq!(Some(EN), markdown.as_deref());
}

#[test]
fn feed_with_both_languages_stays_readable_for_older_versions() {
    let feed = format!("{EN}\n{}\n\n{TR}", ChangelogParser::TURKISH_NOTES_MARKER);
    assert_eq!(
        vec!["Icon alignment".to_string()],
        ChangelogParser::extract_version(Some(&feed), "0.3.0", "en").unwrap().highlights
    );
}

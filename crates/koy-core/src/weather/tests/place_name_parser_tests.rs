//! Karşılığı: `PlaceNameParserTests.cs`.

use crate::weather::PlaceNameParser;

#[test]
fn sehir_adi_once_gelir() {
    let json = r#"{"address":{"suburb":"Kadıköy","city":"İstanbul","state":"İstanbul","country":"Türkiye"}}"#;
    assert_eq!(Some("İstanbul".to_string()), PlaceNameParser::parse(Some(json)));
}

#[test]
fn sehir_yoksa_kasaba_sonra_ilce_sonra_il() {
    assert_eq!(Some("Urla".to_string()), PlaceNameParser::parse(Some(r#"{"address":{"town":"Urla","province":"İzmir"}}"#)));
    assert_eq!(
        Some("Seferihisar".to_string()),
        PlaceNameParser::parse(Some(r#"{"address":{"county":"Seferihisar","province":"İzmir"}}"#))
    );
    assert_eq!(
        Some("İzmir".to_string()),
        PlaceNameParser::parse(Some(r#"{"address":{"province":"İzmir","country":"Türkiye"}}"#))
    );
}

#[test]
fn sokak_ve_mahalle_kullanilmaz() {
    assert_eq!(
        None,
        PlaceNameParser::parse(Some(r#"{"address":{"road":"Bağdat Caddesi","suburb":"Kadıköy","country":"Türkiye"}}"#))
    );
}

#[test]
fn gecersiz_yanit_null_doner() {
    let cases: [Option<&str>; 7] = [
        None,
        Some(""),
        Some("bozuk"),
        Some(r#"{"error":"Unable to geocode"}"#),
        Some(r#"{"address":{"city":"  "}}"#),
        Some(r#"{"address":"metin"}"#),
        Some("[]"),
    ];
    for json in cases {
        assert_eq!(None, PlaceNameParser::parse(json), "{json:?}");
    }
}

#[test]
fn koordinat_iki_ondaliga_yuvarlanir() {
    assert_eq!((41.01, 28.98), PlaceNameParser::round(41.00823, 28.97836));
    assert_eq!((-33.87, 151.21), PlaceNameParser::round(-33.8688, 151.2093));
}

#[test]
fn adres_yuvarlanmis_koordinat_ve_dil_tasir() {
    let url = PlaceNameParser::build_url(41.00823, 28.97836, "tr");
    assert!(url.starts_with("https://nominatim.openstreetmap.org/reverse?"));
    assert!(url.contains("lat=41.01"));
    assert!(url.contains("lon=28.98"));
    assert!(url.contains("zoom=10"));
    assert!(url.contains("accept-language=tr"));
    assert!(!url.contains("41.008"));

    assert!(PlaceNameParser::build_url(1.0, 2.0, "EN").contains("accept-language=en"));
    assert!(PlaceNameParser::build_url(1.0, 2.0, "de").contains("accept-language=tr"));
}

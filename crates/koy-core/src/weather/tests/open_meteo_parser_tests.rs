//! Karşılığı: `WeatherLocationTests.cs` içindeki `OpenMeteoParserTests`.

use crate::weather::OpenMeteoParser;

#[test]
fn current_weather_is_parsed() {
    let json = r#"{"latitude":41.0,"current":{"time":"2026-11-06T09:00","temperature_2m":14.6,"weather_code":61}}"#;

    let result = OpenMeteoParser::parse_current(Some(json));

    let (temperature, weather_code) = result.expect("sonuç bekleniyordu");
    assert_eq!(14.6, temperature);
    assert_eq!(61, weather_code);
}

#[test]
fn bad_current_weather_responses_return_null() {
    let cases: [Option<&str>; 7] = [
        None,
        Some(""),
        Some("bozuk json"),
        Some("{}"),
        Some(r#"{"current":{"temperature_2m":"sıcak","weather_code":1}}"#),
        Some(r#"{"current":{"temperature_2m":10}}"#),
        Some(r#"{"error":true,"reason":"limit"}"#),
    ];
    for json in cases {
        assert_eq!(None, OpenMeteoParser::parse_current(json), "{json:?}");
    }
}

#[test]
fn geocoding_results_are_parsed_and_capped() {
    let json = r#"
    {"results":[
      {"name":"Ankara","latitude":39.91987,"longitude":32.85427,"admin1":"Ankara","country":"Türkiye"},
      {"name":"Ankara","latitude":-1.5,"longitude":10.5,"country":"Gabon"},
      {"name":"A3","latitude":1,"longitude":1},
      {"name":"A4","latitude":1,"longitude":1},
      {"name":"A5","latitude":1,"longitude":1},
      {"name":"A6","latitude":1,"longitude":1}
    ]}
    "#;

    let places = OpenMeteoParser::parse_geocoding(Some(json));

    assert_eq!(OpenMeteoParser::MAX_RESULTS, places.len());
    assert_eq!("Ankara, Türkiye", places[0].display_name());
    assert_eq!(39.91987, places[0].latitude);
    assert_eq!("Ankara, Gabon", places[1].display_name());
}

#[test]
fn geocoding_skips_entries_without_a_name_or_valid_coordinates() {
    let json = r#"
    {"results":[
      {"latitude":1,"longitude":1},
      {"name":"Uzak","latitude":200,"longitude":1},
      {"name":"Eksik","latitude":1},
      {"name":"İyi","latitude":10,"longitude":20}
    ]}
    "#;

    let places = OpenMeteoParser::parse_geocoding(Some(json));

    assert_eq!(1, places.len());
    assert_eq!("İyi", places[0].name);
}

#[test]
fn empty_or_bad_geocoding_responses_return_an_empty_list() {
    let cases: [Option<&str>; 6] = [
        None,
        Some(""),
        Some("{}"),
        Some(r#"{"generationtime_ms":0.5}"#),
        Some(r#"{"results":"yok"}"#),
        Some("[1,2"),
    ];
    for json in cases {
        assert!(OpenMeteoParser::parse_geocoding(json).is_empty(), "{json:?}");
    }
}

#[test]
fn wind_speed_is_parsed_and_rejects_bad_values() {
    // Rust'a özgü ek: `ParseWindSpeed` C# testlerinde doğrudan sınanmıyordu.
    assert_eq!(Some(12.5), OpenMeteoParser::parse_wind_speed(Some(r#"{"current":{"wind_speed_10m":12.5}}"#)));
    assert_eq!(None, OpenMeteoParser::parse_wind_speed(Some(r#"{"current":{"wind_speed_10m":-1}}"#)));
    assert_eq!(None, OpenMeteoParser::parse_wind_speed(Some(r#"{"current":{"wind_speed_10m":"x"}}"#)));
    assert_eq!(None, OpenMeteoParser::parse_wind_speed(None));
}

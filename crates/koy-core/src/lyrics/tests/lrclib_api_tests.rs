//! Karşılığı: `tests/Core.Tests/LrclibApiTests.cs`.

use crate::lyrics::{LrclibApi, LyricsQuery};
use crate::time::TimeSpan;

fn query(seconds: Option<f64>) -> LyricsQuery {
    LyricsQuery::new("Song", "Artist", seconds.map(TimeSpan::from_seconds_f64))
}

fn default_query() -> LyricsQuery {
    query(Some(200.0))
}

// JSON sayısı kültürden bağımsız yazılır (Rust `{}` biçimi de her zaman noktalıdır).
fn item(title: &str, duration: f64, synced: Option<&str>) -> String {
    let synced = match synced {
        None => "null".to_string(),
        Some(s) => format!("\"{s}\""),
    };
    format!("{{\"trackName\":\"{title}\",\"duration\":{duration},\"syncedLyrics\":{synced}}}")
}

#[test]
fn get_url_escapes_fields_and_rounds_duration() {
    let url = LrclibApi::get_url(&LyricsQuery::new("Şarkı & Söz", "A/B", Some(TimeSpan::from_seconds_f64(199.6))));

    assert_eq!(
        "https://lrclib.net/api/get?track_name=%C5%9Eark%C4%B1%20%26%20S%C3%B6z&artist_name=A%2FB&duration=200",
        url
    );
}

#[test]
fn get_url_omits_missing_or_out_of_range_duration() {
    for seconds in [None, Some(0.5), Some(4000.0)] {
        assert!(!LrclibApi::get_url(&query(seconds)).contains("duration"), "{seconds:?}");
    }
}

#[test]
fn search_url_uses_primary_artist() {
    let url = LrclibApi::search_url(&LyricsQuery::new("Song", "A, B", None));

    assert!(url.ends_with("artist_name=A"));
}

#[test]
fn reads_only_synced_lyrics() {
    for (json, expected) in [
        (r#"{"syncedLyrics":"[00:01.00]a"}"#, Some("[00:01.00]a")),
        (r#"{"syncedLyrics":null,"plainLyrics":"a"}"#, None),
        (r#"{"syncedLyrics":"  "}"#, None),
        ("[]", None),
        ("not json", None),
    ] {
        assert_eq!(expected.map(str::to_string), LrclibApi::parse_synced(json), "{json}");
    }
}

#[test]
fn picks_closest_duration_within_tolerance() {
    let json = format!(
        "[{},{},{}]",
        item("Song", 205.0, Some("far")),
        item("Song", 202.0, Some("near")),
        item("Song", 200.5, Some("nearest"))
    );

    assert_eq!(Some("nearest".to_string()), LrclibApi::pick_best(&json, &default_query()));
}

#[test]
fn rejects_versions_outside_tolerance() {
    assert_eq!(None, LrclibApi::pick_best(&format!("[{}]", item("Song", 260.0, Some("other version"))), &default_query()));
}

#[test]
fn skips_entries_without_synced_lyrics() {
    let json = format!("[{},{}]", item("Song", 200.0, None), item("Song", 201.0, Some("ok")));

    assert_eq!(Some("ok".to_string()), LrclibApi::pick_best(&json, &default_query()));
}

#[test]
fn rejects_a_different_song() {
    assert_eq!(
        None,
        LrclibApi::pick_best(&format!("[{}]", item("Completely Different", 200.0, Some("x"))), &default_query())
    );
}

#[test]
fn without_duration_takes_first_matching_title() {
    let json = format!(
        "[{},{},{}]",
        item("Other", 100.0, Some("no")),
        item("Song (Remix)", 100.0, Some("first")),
        item("Song", 200.0, Some("second"))
    );

    assert_eq!(Some("first".to_string()), LrclibApi::pick_best(&json, &query(None)));
}

#[test]
fn survives_unexpected_search_responses() {
    for json in ["{}", "broken"] {
        assert_eq!(None, LrclibApi::pick_best(json, &default_query()), "{json}");
    }
}

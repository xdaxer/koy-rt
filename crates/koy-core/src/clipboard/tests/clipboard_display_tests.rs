use crate::clipboard::*;
use crate::time::{add_span, DateTimeOffset, TimeSpan};
use chrono::{FixedOffset, TimeZone};

fn now() -> DateTimeOffset {
    FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 6, 14, 0, 0).unwrap()
}

fn add_seconds(t: DateTimeOffset, s: i64) -> DateTimeOffset {
    add_span(t, TimeSpan::from_secs(s))
}

#[test]
fn age_picks_the_largest_fitting_unit() {
    let cases = [
        (0, AgeUnit::Now, 0),
        (59, AgeUnit::Now, 0),
        (60, AgeUnit::Minutes, 1),
        (2 * 60 + 10, AgeUnit::Minutes, 2),
        (59 * 60, AgeUnit::Minutes, 59),
        (60 * 60, AgeUnit::Hours, 1),
        (5 * 3600 + 100, AgeUnit::Hours, 5),
        (24 * 3600, AgeUnit::Days, 1),
        (3 * 86400 + 5, AgeUnit::Days, 3),
    ];
    for (seconds, unit, value) in cases {
        assert_eq!((unit, value), ClipboardDisplay::age(add_seconds(now(), -seconds), now()), "seconds = {seconds}");
    }
}

#[test]
fn age_future_timestamp_is_now() {
    // Saat geri alınırsa negatif süre "şimdi" sayılır.
    assert_eq!((AgeUnit::Now, 0), ClipboardDisplay::age(add_seconds(now(), 5 * 60), now()));
}

#[test]
fn preview_takes_first_lines_and_skips_blanks() {
    let text = "  ilk\n\n   ikinci  \r\nüçüncü\ndördüncü";
    assert_eq!("ilk\nikinci\nüçüncü", ClipboardDisplay::preview(Some(text), 3, 90));
    assert_eq!("ilk", ClipboardDisplay::preview(Some(text), 1, 90));
}

#[test]
fn preview_cuts_long_lines() {
    let result = ClipboardDisplay::preview(Some(&"a".repeat(200)), 3, 10);
    assert_eq!(format!("{}…", "a".repeat(10)), result);
}

#[test]
fn preview_empty_input_is_empty() {
    for text in [None, Some(""), Some("  \n ")] {
        assert_eq!("", ClipboardDisplay::preview(text, 3, 90), "{text:?}");
    }
}

#[test]
fn try_parse_color_reads_known_forms() {
    let cases = [
        ("#3B82F6", 0x3B, 0x82, 0xF6, 255),
        ("#fff", 255, 255, 255, 255),
        ("#0af", 0x00, 0xAA, 0xFF, 255),
        ("#11223380", 0x11, 0x22, 0x33, 0x80),
        ("rgb(59, 130, 246)", 59, 130, 246, 255),
        ("RGB(0,0,0)", 0, 0, 0, 255),
        ("rgba(10, 20, 30, 0.5)", 10, 20, 30, 128),
        ("rgba(10, 20, 30, 50%)", 10, 20, 30, 128),
        ("rgba(10, 20, 30, 1)", 10, 20, 30, 255),
    ];
    for (text, r, g, b, a) in cases {
        let (rr, gg, bb, aa) = ClipboardDisplay::try_parse_color(Some(text)).unwrap_or_else(|| panic!("{text}"));
        assert_eq!((r, g, b, a), (rr as i32, gg as i32, bb as i32, aa as i32), "{text}");
    }
}

#[test]
fn try_parse_color_rejects_others() {
    for text in [None, Some(""), Some("#12"), Some("#12345"), Some("#GGGGGG"), Some("rgb(300, 0, 0)"), Some("rgb(1, 2)"), Some("mavi")] {
        assert!(ClipboardDisplay::try_parse_color(text).is_none(), "{text:?}");
    }
}

#[test]
fn classifier_colors_always_parse() {
    // Sınıflandırıcının "Renk" dediği her metin gösterimde de çözülebilmeli.
    for text in ["#abc", "#aabbcc", "#aabbccdd", "rgb(1,2,3)", "rgba(1, 2, 3, 0.4)", "rgba(1,2,3,40%)"] {
        assert_eq!(ClipboardKind::Color, ClipboardClassifier::classify(Some(text)));
        assert!(ClipboardDisplay::try_parse_color(Some(text)).is_some(), "{text}");
    }
}

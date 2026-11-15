//! Karşılığı: `tests/Core.Tests/LrcParserTests.cs`.

use crate::lyrics::LrcParser;
use crate::time::TimeSpan;

fn ms(value: i64) -> TimeSpan {
    TimeSpan::from_millis(value)
}

#[test]
fn reads_time_stamps() {
    for (lrc, expected) in [
        ("[01:02.5]a", 62500),
        ("[01:02.50]a", 62500),
        ("[01:02.500]a", 62500),
        ("[01:02:50]a", 62500),
        ("[01:02]a", 62000),
    ] {
        let lines = LrcParser::parse(Some(lrc));
        assert_eq!(lines.len(), 1, "{lrc}");
        let line = &lines[0];

        assert_eq!(ms(expected), line.time, "{lrc}");
        assert_eq!("a", line.text, "{lrc}");
    }
}

#[test]
fn sorts_by_time_and_expands_repeated_stamps() {
    let lines = LrcParser::parse(Some("[00:30.00]b\n[00:10.00][00:50.00]chorus\n"));

    assert_eq!(vec!["chorus", "b", "chorus"], lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>());
    assert_eq!(vec![ms(10_000), ms(30_000), ms(50_000)], lines.iter().map(|l| l.time).collect::<Vec<_>>());
}

#[test]
fn skips_metadata_and_handles_bom_and_crlf() {
    let lines = LrcParser::parse(Some("\u{FEFF}[ar:Someone]\r\n[ti:Song]\r\nplain text\r\n[00:01.00] hello \r\n"));

    assert_eq!(lines.len(), 1);
    assert_eq!("hello", lines[0].text);
}

#[test]
fn removes_word_level_tags() {
    let lines = LrcParser::parse(Some("[00:01.00]<00:01.00>one <00:01.50>two"));

    assert_eq!(lines.len(), 1);
    assert_eq!("one two", lines[0].text);
}

#[test]
fn positive_offset_moves_lines_earlier_and_never_below_zero() {
    let lines = LrcParser::parse(Some("[offset:+500]\n[00:00.20]a\n[00:02.00]b"));

    assert_eq!(TimeSpan::ZERO, lines[0].time);
    assert_eq!(ms(1500), lines[1].time);
}

#[test]
fn collapses_consecutive_blank_lines() {
    let lines = LrcParser::parse(Some("[00:01.00]a\n[00:02.00]\n[00:03.00]\n[00:04.00]b"));

    assert_eq!(vec!["a", "", "b"], lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>());
}

#[test]
fn returns_empty_when_there_is_nothing_to_show() {
    for lrc in [None, Some(""), Some("no stamps here"), Some("[00:01.00]\n[00:02.00]")] {
        assert!(LrcParser::parse(lrc).is_empty(), "{lrc:?}");
    }
}

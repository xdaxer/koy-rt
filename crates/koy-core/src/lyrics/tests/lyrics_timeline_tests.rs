//! Karşılığı: `tests/Core.Tests/LyricsTimelineTests.cs`.

use crate::lyrics::{LyricLine, LyricsTimeline};
use crate::time::TimeSpan;

fn s(seconds: f64) -> TimeSpan {
    TimeSpan::from_seconds_f64(seconds)
}

fn lines() -> Vec<LyricLine> {
    vec![LyricLine::new(s(10.0), "a"), LyricLine::new(s(20.0), "b"), LyricLine::new(s(30.0), "c")]
}

#[test]
fn finds_active_line_with_lead() {
    for (position, expected) in [(0.0, -1), (9.8, -1), (9.9, 0), (19.0, 0), (25.0, 1), (500.0, 2)] {
        assert_eq!(expected, LyricsTimeline::index_at(&lines(), s(position)), "{position}");
    }
}

#[test]
fn no_lines_means_no_active_line_or_timer() {
    assert_eq!(-1, LyricsTimeline::index_at(&[], s(5.0)));
    assert_eq!(None, LyricsTimeline::until_next_change(&[], s(5.0)));
}

#[test]
fn delay_reaches_next_line_minus_lead() {
    assert_eq!(Some(s(20.0) - LyricsTimeline::LEAD - s(15.0)), LyricsTimeline::until_next_change(&lines(), s(15.0)));
}

#[test]
fn no_timer_after_last_line() {
    assert_eq!(None, LyricsTimeline::until_next_change(&lines(), s(31.0)));
}

#[test]
fn lines_with_same_time_never_produce_a_zero_delay_loop() {
    let lines = vec![LyricLine::new(s(10.0), "a"), LyricLine::new(s(10.0), "b"), LyricLine::new(s(20.0), "c")];

    assert_eq!(1, LyricsTimeline::index_at(&lines, s(11.0)));
    assert!(LyricsTimeline::until_next_change(&lines, s(11.0)).unwrap() > TimeSpan::ZERO);
}

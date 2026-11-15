//! Karşılığı: `tests/Core.Tests/LyricsQueryTests.cs`.

use crate::lyrics::LyricsQuery;
use crate::time::TimeSpan;

const LENGTH: TimeSpan = TimeSpan::from_mins(3);

#[test]
fn removes_title_noise_only() {
    for (raw, expected) in [
        ("Song (Official Music Video)", "Song"),
        ("Song [Lyrics]", "Song"),
        ("Song (feat. Someone)", "Song"),
        ("Song (Resmi Video)", "Song"),
        ("Song - Remastered 2011", "Song"),
        ("Song - 2011 Remaster", "Song"),
        ("Song (Live at Wembley)", "Song (Live at Wembley)"),
    ] {
        assert_eq!(expected, LyricsQuery::create(Some(raw), Some("Artist"), LENGTH).unwrap().title, "{raw}");
    }
}

#[test]
fn removes_channel_suffixes() {
    for (raw, expected) in [("Queen - Topic", "Queen"), ("QueenVEVO", "Queen"), ("Queen", "Queen")] {
        assert_eq!(expected, LyricsQuery::create(Some("Song"), Some(raw), LENGTH).unwrap().artist, "{raw}");
    }
}

#[test]
fn splits_video_title_when_artist_is_missing() {
    let query = LyricsQuery::create(Some("Artist - Song (Official Video)"), Some(""), LENGTH).unwrap();

    assert_eq!("Artist", query.artist);
    assert_eq!("Song", query.title);
}

#[test]
fn strips_artist_prefix_when_it_repeats_the_artist_field() {
    assert_eq!("Song", LyricsQuery::create(Some("artist - Song"), Some("Artist"), LENGTH).unwrap().title);
}

#[test]
fn splits_video_title_when_channel_name_overlaps_the_artist() {
    for (title, channel, artist, track) in [
        ("Queen - Bohemian Rhapsody (Official Video)", "Queen Official", "Queen", "Bohemian Rhapsody"),
        ("Daft Punk - Around the World", "Punk", "Daft Punk", "Around the World"),
    ] {
        let query = LyricsQuery::create(Some(title), Some(channel), LENGTH).unwrap();

        assert_eq!(artist, query.artist, "{title}");
        assert_eq!(track, query.title, "{title}");
    }
}

#[test]
fn does_not_split_on_a_partial_word_match() {
    let query = LyricsQuery::create(Some("Anything - Else"), Some("A"), LENGTH).unwrap();

    assert_eq!("A", query.artist);
    assert_eq!("Anything - Else", query.title);
}

#[test]
fn keeps_dash_when_left_side_is_not_the_artist() {
    assert_eq!("Part 1 - Intro", LyricsQuery::create(Some("Part 1 - Intro"), Some("Artist"), LENGTH).unwrap().title);
}

#[test]
fn returns_null_without_a_title() {
    for title in [None, Some("   "), Some("(Official Video)")] {
        assert!(LyricsQuery::create(title, Some("Artist"), LENGTH).is_none(), "{title:?}");
    }
}

#[test]
fn rejects_absurdly_long_titles() {
    assert!(LyricsQuery::create(Some(&"x".repeat(201)), Some("Artist"), LENGTH).is_none());
}

#[test]
fn unknown_duration_is_null() {
    assert_eq!(None, LyricsQuery::create(Some("Song"), Some("Artist"), TimeSpan::ZERO).unwrap().duration);
}

#[test]
fn key_ignores_duration_and_case() {
    let first = LyricsQuery::create(Some("Song"), Some("Artist"), LENGTH).unwrap();
    let second = LyricsQuery::create(Some("SONG"), Some("ARTIST"), TimeSpan::ZERO).unwrap();

    assert_eq!(first.key(), second.key());
}

#[test]
fn primary_artist_is_the_first_name() {
    assert_eq!("A", LyricsQuery::create(Some("Song"), Some("A, B"), LENGTH).unwrap().primary_artist());
}

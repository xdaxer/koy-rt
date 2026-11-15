//! Karşılığı: `tests/Core.Tests/LyricsStorageTests.cs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, SystemTime};

use chrono::{Months, TimeDelta};

use crate::lyrics::{
    CancellationToken, ILyricsSource, LocalLyricsSource, LyricsCache, LyricsCacheStatus, LyricsLookup, LyricsLookupKind,
    LyricsQuery,
};
use crate::time::{DateTimeOffset, TimeProvider, TimeSpan, Timer, TimerCallback};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        static COUNTER: AtomicI32 = AtomicI32::new(0);
        let path = std::env::temp_dir()
            .join(format!("koy-lyrics-storage-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst)));
        fs::create_dir_all(&path).unwrap();
        TestDir { path }
    }

    fn write_lrc(&self, relative_path: &str, content: &str) {
        let path = self.path.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn file_count(&self) -> usize {
        fs::read_dir(&self.path).unwrap().filter(|e| e.as_ref().unwrap().file_type().unwrap().is_file()).count()
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn query() -> LyricsQuery {
    LyricsQuery::new("Song", "Artist", None)
}

/// `FixedTime : TimeProvider` (`GetUtcNow` sabit).
struct FixedTime(DateTimeOffset);

impl TimeProvider for FixedTime {
    fn utc_now(&self) -> DateTimeOffset {
        self.0
    }

    fn timestamp_frequency(&self) -> i64 {
        TimeSpan::TICKS_PER_SECOND
    }

    fn timestamp(&self) -> i64 {
        0
    }

    fn create_timer(&self, _callback: TimerCallback, _due_time: TimeSpan, _period: TimeSpan) -> Box<dyn Timer> {
        unimplemented!("test saati zamanlayıcı kurmaz")
    }
}

fn now() -> DateTimeOffset {
    chrono::Utc::now().fixed_offset()
}

fn find(source: &LocalLyricsSource, query: &LyricsQuery) -> LyricsLookup {
    source.find(query, &CancellationToken::none()).unwrap()
}

#[test]
fn local_finds_exact_file_name() {
    let dir = TestDir::new();
    dir.write_lrc("Artist - Song.lrc", "exact");

    let result = find(&LocalLyricsSource::new(&dir.path), &query());

    assert_eq!(LyricsLookup::found("exact"), result);
}

#[test]
fn local_replaces_characters_windows_forbids() {
    let dir = TestDir::new();
    dir.write_lrc("AC_DC - Song_.lrc", "safe");

    let result = find(&LocalLyricsSource::new(&dir.path), &LyricsQuery::new("Song?", "AC/DC", None));

    assert_eq!(Some("safe".to_string()), result.lrc);
}

#[test]
fn local_scans_subfolders_ignoring_case() {
    let dir = TestDir::new();
    dir.write_lrc(&Path::new("albums").join("01 ARTIST song.lrc").to_string_lossy(), "nested");

    let result = find(&LocalLyricsSource::new(&dir.path), &query());

    assert_eq!(Some("nested".to_string()), result.lrc);
}

#[test]
fn local_does_not_guess_without_an_artist() {
    let dir = TestDir::new();
    dir.write_lrc("Someone - Song.lrc", "[00:01.00]a");

    let result = find(&LocalLyricsSource::new(&dir.path), &LyricsQuery::new("Song", "", None));

    assert_eq!(LyricsLookupKind::NotFound, result.kind);
}

#[test]
fn local_ignores_oversized_files() {
    let dir = TestDir::new();
    dir.write_lrc("Artist - Song.lrc", &"x".repeat(1024 * 1024 + 1));

    let result = find(&LocalLyricsSource::new(&dir.path), &query());

    assert_eq!(LyricsLookupKind::NotFound, result.kind);
}

#[test]
fn local_is_not_found_when_folder_is_missing() {
    let dir = TestDir::new();

    let result = find(&LocalLyricsSource::new(dir.path.join("nope")), &query());

    assert_eq!(LyricsLookupKind::NotFound, result.kind);
}

#[test]
fn cache_returns_what_was_written() {
    let dir = TestDir::new();
    let cache = LyricsCache::new(&dir.path);

    assert_eq!(LyricsCacheStatus::Miss, cache.read(&query()).0);
    cache.write(&query(), "lrc");

    assert_eq!((LyricsCacheStatus::Hit, Some("lrc".to_string())), cache.read(&query()));
}

#[test]
fn cache_file_name_does_not_expose_the_track() {
    let dir = TestDir::new();
    LyricsCache::new(&dir.path).write(&query(), "lrc");

    let names: Vec<String> =
        fs::read_dir(&dir.path).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(1, names.len());
    assert!(!names[0].to_lowercase().contains("song"));
}

#[test]
fn cache_missing_marker_expires() {
    let dir = TestDir::new();
    let now = now();
    LyricsCache::new(&dir.path).write_missing(&query());

    let six_days = LyricsCache::with_options(&dir.path, Some(Arc::new(FixedTime(now + TimeDelta::days(6)))), 1000);
    let eight_days = LyricsCache::with_options(&dir.path, Some(Arc::new(FixedTime(now + TimeDelta::days(8)))), 1000);
    assert_eq!(LyricsCacheStatus::KnownMissing, six_days.read(&query()).0);
    assert_eq!(LyricsCacheStatus::Miss, eight_days.read(&query()).0);
}

#[test]
fn cache_found_lyrics_do_not_expire() {
    let dir = TestDir::new();
    let now = now();
    LyricsCache::new(&dir.path).write(&query(), "lrc");

    let five_years = now.checked_add_months(Months::new(60)).unwrap();
    let cache = LyricsCache::with_options(&dir.path, Some(Arc::new(FixedTime(five_years))), 1000);
    assert_eq!(LyricsCacheStatus::Hit, cache.read(&query()).0);
}

#[test]
fn cache_removes_oldest_files_beyond_the_limit() {
    let dir = TestDir::new();
    let cache = LyricsCache::with_options(&dir.path, None, 10);
    let start = SystemTime::now() - Duration::from_secs(24 * 3600);
    let after_start_hour = start + Duration::from_secs(3600);

    let numbered = |i: i32| LyricsQuery::new(format!("Song {i}"), "Artist", None);
    for i in 0..10 {
        cache.write(&numbered(i), "lrc");
        for entry in fs::read_dir(&dir.path).unwrap() {
            let file = entry.unwrap().path();
            let modified = fs::metadata(&file).unwrap().modified().unwrap();
            if modified > after_start_hour {
                let handle = fs::OpenOptions::new().write(true).open(&file).unwrap();
                handle.set_modified(start + Duration::from_secs(60 * i as u64)).unwrap();
            }
        }
    }

    cache.write(&numbered(10), "lrc");

    assert_eq!(8, dir.file_count());
    assert_eq!(LyricsCacheStatus::Miss, cache.read(&numbered(0)).0);
    assert_eq!(LyricsCacheStatus::Hit, cache.read(&numbered(10)).0);
}

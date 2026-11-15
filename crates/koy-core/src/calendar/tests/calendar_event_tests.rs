//! Karşılığı: `CalendarEventTests.cs` (EventAgendaTests + EventBookTests).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::calendar::*;
use crate::clock::DayOfWeek;

fn friday() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 6).unwrap()
}

fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, min, 0).unwrap()
}

fn time(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn ev(title: &str, start: NaiveDateTime, minutes: i32) -> CalendarEvent {
    CalendarEvent::new(crate::calendar::tests_support_guid(), title, start, minutes)
}

fn titles(events: &[CalendarEvent]) -> Vec<&str> {
    events.iter().map(|e| e.title.as_str()).collect()
}

// ---- EventAgendaTests ----

#[test]
fn for_day_returns_only_that_day_ordered_by_time() {
    let events = [ev("Öğle", at(2026, 11, 6, 12, 0), 60), ev("Sabah", at(2026, 11, 6, 8, 30), 60), ev("Yarın", at(2026, 11, 7, 9, 0), 60)];

    let day = EventAgenda::for_day(&events, friday());

    assert_eq!(vec!["Sabah", "Öğle"], titles(&day));
}

#[test]
fn week_start_is_the_monday_of_the_week() {
    let rows = [
        (2026, 11, 2, 2),  // Pazartesi kendisi
        (2026, 11, 6, 2),  // Cuma
        (2026, 11, 8, 2),  // Pazar önceki Pazartesi'ye aittir
        (2026, 11, 9, 9),  // sonraki Pazartesi
    ];
    for (y, m, d, expected_day) in rows {
        let start = EventAgenda::week_start(NaiveDate::from_ymd_opt(y, m, d).unwrap());
        assert_eq!(expected_day, chrono::Datelike::day(&start), "{y}-{m}-{d}");
        assert_eq!(DayOfWeek::Monday, DayOfWeek::of(start));
    }
}

#[test]
fn for_week_has_seven_days_marks_today_and_groups_events() {
    let events = [
        ev("Pzt", at(2026, 11, 2, 9, 0), 60),
        ev("Cuma 1", at(2026, 11, 6, 9, 0), 60),
        ev("Cuma 2", at(2026, 11, 6, 15, 0), 60),
        ev("Gelecek hafta", at(2026, 11, 9, 9, 0), 60),
    ];

    let week = EventAgenda::for_week(&events, friday(), friday());

    assert_eq!(7, week.len());
    assert_eq!(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap(), week[0].date);
    assert_eq!(NaiveDate::from_ymd_opt(2026, 11, 8).unwrap(), week[6].date);
    assert_eq!(1, week.iter().filter(|d| d.is_today).count());
    assert!(week[4].is_today);
    assert_eq!(1, week[0].events.len());
    assert_eq!(2, week[4].events.len());
    assert_eq!(0, week.iter().map(|d| d.events.iter().filter(|e| e.title == "Gelecek hafta").count()).sum::<usize>());
}

#[test]
fn next_today_skips_finished_events_and_other_days() {
    let events = [
        ev("Bitti", at(2026, 11, 6, 8, 0), 60),
        ev("Yarın", at(2026, 11, 7, 10, 0), 60),
        ev("Sıradaki", at(2026, 11, 6, 14, 0), 60),
        ev("Sonra", at(2026, 11, 6, 18, 0), 60),
    ];

    let next = EventAgenda::next_today(&events, at(2026, 11, 6, 10, 0));

    assert_eq!(Some("Sıradaki"), next.as_ref().map(|e| e.title.as_str()));
}

#[test]
fn next_today_includes_an_event_in_progress() {
    let events = [ev("Toplantı", at(2026, 11, 6, 9, 0), 120)];

    assert_eq!(Some("Toplantı".to_string()), EventAgenda::next_today(&events, at(2026, 11, 6, 10, 0)).map(|e| e.title));
    assert!(EventAgenda::next_today(&events, at(2026, 11, 6, 11, 0)).is_none());
}

#[test]
fn next_today_treats_zero_length_events_as_a_moment() {
    let events = [ev("Hatırlatma", at(2026, 11, 6, 9, 0), 0)];

    assert!(EventAgenda::next_today(&events, at(2026, 11, 6, 9, 0)).is_some());
    assert!(EventAgenda::next_today(&events, at(2026, 11, 6, 9, 1)).is_none());
}

#[test]
fn next_today_is_null_without_events() {
    assert!(EventAgenda::next_today(&[], at(2026, 11, 6, 9, 0)).is_none());
}

#[test]
fn titles_are_trimmed_and_blank_titles_rejected() {
    let rows = [(None, None), (Some(""), None), (Some("   "), None), (Some("  Toplantı "), Some("Toplantı"))];
    for (input, expected) in rows {
        assert_eq!(expected.map(String::from), EventAgenda::normalize_title(input), "{input:?}");
    }
}

#[test]
fn long_titles_are_cut() {
    let title = EventAgenda::normalize_title(Some(&"a".repeat(200)));

    assert_eq!(EventAgenda::MAX_TITLE_LENGTH, title.unwrap().chars().count());
}

#[test]
fn times_are_parsed_in_common_forms() {
    let rows = [
        ("14:30", 14, 30),
        ("9:05", 9, 5),
        ("09.05", 9, 5),
        ("1430", 14, 30),
        ("905", 9, 5),
        (" 00:00 ", 0, 0),
        ("23:59", 23, 59),
    ];
    for (text, hour, minute) in rows {
        assert_eq!(Some(time(hour, minute)), EventAgenda::try_parse_time(Some(text)), "{text:?}");
    }
}

#[test]
fn bad_times_are_rejected() {
    for text in [None, Some(""), Some("24:00"), Some("12:60"), Some("12:5"), Some("abc"), Some("12:30:00"), Some("-1:30")] {
        assert_eq!(None, EventAgenda::try_parse_time(text), "{text:?}");
    }
}

// ---- EventBookTests ----

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        TempDir(std::env::temp_dir().join(format!("koy-events-{}", crate::calendar::tests_support_guid())))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn add_keeps_events_sorted_and_raises_changed() {
    let mut book = EventBook::default();
    let raised = Arc::new(AtomicI32::new(0));
    let r = Arc::clone(&raised);
    book.changed().subscribe(move |_| {
        r.fetch_add(1, Ordering::SeqCst);
    });

    book.add(Some("Akşam"), friday(), time(19, 0));
    book.add(Some("Sabah"), friday(), time(8, 0));

    assert_eq!(vec!["Sabah", "Akşam"], titles(book.events()));
    assert_eq!(2, raised.load(Ordering::SeqCst));
}

#[test]
fn blank_title_is_not_added() {
    let mut book = EventBook::default();

    assert!(book.add(Some("  "), friday(), time(8, 0)).is_none());
    assert!(book.events().is_empty());
}

#[test]
fn duration_is_clamped() {
    let mut book = EventBook::default();

    assert_eq!(0, book.add_with_duration(Some("a"), friday(), time(8, 0), -5).unwrap().duration_minutes);
    assert_eq!(EventAgenda::MAX_DURATION_MINUTES, book.add_with_duration(Some("b"), friday(), time(9, 0), 99999).unwrap().duration_minutes);
}

#[test]
fn book_is_capped_at_the_maximum() {
    let mut book = EventBook::default();
    for i in 0..EventBook::MAX_EVENTS {
        assert!(book.add(Some(&format!("e{i}")), friday(), time(8, 0)).is_some());
    }

    assert!(book.add(Some("fazla"), friday(), time(8, 0)).is_none());
    assert_eq!(EventBook::MAX_EVENTS, book.events().len());
}

#[test]
fn remove_deletes_by_id_and_reports_result() {
    let mut book = EventBook::default();
    let added = book.add(Some("Sil"), friday(), time(8, 0)).unwrap();

    assert!(book.remove(&added.id));
    assert!(!book.remove(&added.id));
    assert!(book.events().is_empty());
}

#[test]
fn prune_before_removes_only_events_that_ended_before_the_cutoff() {
    let mut book = EventBook::default();
    book.add(Some("Eski"), NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(), time(8, 0));
    book.add(Some("Yeni"), NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(), time(8, 0));

    let removed = book.prune_before(NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());

    assert_eq!(1, removed);
    assert_eq!(vec!["Yeni"], titles(book.events()));
}

#[test]
fn store_round_trips_events() {
    let dir = TempDir::new();
    let store = EventStore::new(dir.0.join("events.json"));
    let mut book = EventBook::default();
    book.add_with_duration(Some("Doktor"), friday(), time(14, 30), 45);
    book.add_with_duration(Some("Spor"), friday() + chrono::Duration::days(1), time(7, 0), 0);

    store.save(book.events()).unwrap();
    let loaded = store.load();

    assert_eq!(2, loaded.len());
    assert_eq!("Doktor", loaded[0].title);
    assert_eq!(at(2026, 11, 6, 14, 30), loaded[0].start);
    assert_eq!(45, loaded[0].duration_minutes);
    let mut temp = store.path().to_path_buf().into_os_string();
    temp.push(".tmp");
    assert!(!PathBuf::from(temp).exists());
}

#[test]
fn store_writes_the_csharp_json_shape() {
    // C# dosyası: PascalCase, Source sayı, salt okunur özellikler (End, Date, IsReadOnly) de yazılır.
    let dir = TempDir::new();
    let store = EventStore::new(dir.0.join("events.json"));
    let start = at(2026, 11, 6, 14, 30);
    store.save(&[CalendarEvent::with_source("g1", "Doktor", start, 45, CalendarEventSource::Google)]).unwrap();

    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(store.path()).unwrap()).unwrap();
    let row = &value[0];
    assert_eq!("g1", row["Id"]);
    assert_eq!("2026-11-06T14:30:00", row["Start"]);
    assert_eq!(45, row["DurationMinutes"]);
    assert_eq!(1, row["Source"]);
    assert_eq!("2026-11-06T15:15:00", row["End"]);
    assert_eq!("2026-11-06", row["Date"]);
    assert_eq!(true, row["IsReadOnly"]);
}

#[test]
fn store_returns_an_empty_list_for_missing_or_corrupt_files() {
    let dir = TempDir::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    let missing = EventStore::new(dir.0.join("yok.json"));
    let corrupt = dir.0.join("bozuk.json");
    std::fs::write(&corrupt, "{ bu json değil").unwrap();

    assert!(missing.load().is_empty());
    assert!(EventStore::new(corrupt).load().is_empty());
}

#[test]
fn store_skips_entries_without_id_or_title() {
    let dir = TempDir::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    let path = dir.0.join("karisik.json");
    std::fs::write(
        &path,
        r#"[{"Id":"","Title":"x","Start":"2026-11-06T09:00:00","DurationMinutes":30},{"Id":"a","Title":"İyi","Start":"2026-11-06T09:00:00","DurationMinutes":30},{"Id":"b","Title":" ","Start":"2026-11-06T09:00:00","DurationMinutes":30}]"#,
    )
    .unwrap();

    let loaded = EventStore::new(path).load();

    assert_eq!(vec!["İyi"], titles(&loaded));
}

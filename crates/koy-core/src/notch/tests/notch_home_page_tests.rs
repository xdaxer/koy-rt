use chrono::{Duration, NaiveDate, NaiveDateTime};

use crate::calendar::{CalendarEvent, EventAgenda};
use crate::notch::{NotchHomePage, NotchHomePages};
use crate::time::TimeSpan;

#[test]
fn on_open_preserves_selected_page_when_music_is_available() {
    use NotchHomePage::{Home, Music};
    let rows = [(Music, true, true, Music), (Home, true, true, Home), (Music, false, true, Home), (Music, true, false, Home), (Home, false, false, Home)];
    for (current, has_track, music_enabled, expected) in rows {
        assert_eq!(expected, NotchHomePages::on_open(current, has_track, music_enabled), "{current:?} {has_track} {music_enabled}");
    }
}

// ---- EventAgendaUpcomingTests (aynı C# dosyasında) ----

fn dt(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
}

fn now() -> NaiveDateTime {
    dt(2026, 11, 13, 23, 47)
}

fn ev(title: &str, start: NaiveDateTime, minutes: i32) -> CalendarEvent {
    CalendarEvent::new(format!("{title}-id"), title, start, minutes)
}

fn titles(list: Vec<CalendarEvent>) -> Vec<String> {
    list.into_iter().map(|e| e.title).collect()
}

#[test]
fn upcoming_skips_finished_keeps_ongoing_and_orders_across_days() {
    let events = [
        ev("Bitti", dt(2026, 11, 13, 20, 0), 60),
        ev("Sürüyor", dt(2026, 11, 13, 23, 30), 60),
        ev("Yarın öğle", dt(2026, 11, 14, 14, 30), 60),
        ev("Yarın sabah", dt(2026, 11, 14, 10, 0), 60),
    ];

    let list = EventAgenda::upcoming_default(&events, now(), 3);

    assert_eq!(vec!["Sürüyor", "Yarın sabah", "Yarın öğle"], titles(list));
}

#[test]
fn upcoming_drops_a_past_moment_and_respects_count_and_horizon() {
    let events = [
        ev("Geçmiş an", dt(2026, 11, 13, 23, 0), 0),
        ev("Bir", dt(2026, 11, 14, 9, 0), 60),
        ev("İki", dt(2026, 11, 15, 9, 0), 60),
        ev("Çok ileride", dt(2026, 12, 30, 9, 0), 60),
    ];

    assert_eq!(vec!["Bir"], titles(EventAgenda::upcoming_default(&events, now(), 1)));
    assert_eq!(vec!["Bir", "İki"], titles(EventAgenda::upcoming_default(&events, now(), 5)));
    assert!(EventAgenda::upcoming_default(&events, now(), 0).is_empty());
}

#[test]
fn until_is_zero_once_started() {
    let soon = ev("Yakında", now() + Duration::minutes(73), 60);
    let started = ev("Başladı", now() - Duration::minutes(5), 60);

    assert_eq!(TimeSpan::from_mins(73), EventAgenda::until(&soon, now()));
    assert_eq!(TimeSpan::ZERO, EventAgenda::until(&started, now()));
}

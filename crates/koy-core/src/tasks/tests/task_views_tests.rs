//! Karşılığı: `TaskViewsTests.cs`.

use std::sync::Arc;

use chrono::{Datelike, Duration, FixedOffset, NaiveDate, NaiveTime, TimeZone, Weekday};

use crate::tasks::*;

fn thursday() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 5).unwrap() // Perşembe
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn hm(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn item(title: &str, done: bool, time: Option<NaiveTime>, day: Option<NaiveDate>) -> TaskItem {
    TaskItem::new(
        Guid::new_guid(),
        title,
        day.unwrap_or_else(thursday),
        time,
        done,
        chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset(),
    )
}

#[test]
fn range_day_is_today() {
    assert_eq!((thursday(), thursday()), TaskViews::range(TaskScope::Day, thursday()));
}

#[test]
fn range_week_starts_on_monday_and_ends_on_sunday() {
    let (from, to) = TaskViews::range(TaskScope::Week, thursday());
    assert_eq!(d(2026, 11, 2), from);
    assert_eq!(Weekday::Mon, from.weekday());
    assert_eq!(d(2026, 11, 8), to);
    assert_eq!(Weekday::Sun, to.weekday());
}

#[test]
fn range_week_on_sunday_belongs_to_the_week_that_ends_today() {
    let sunday = d(2026, 11, 8);
    let (from, to) = TaskViews::range(TaskScope::Week, sunday);
    assert_eq!(d(2026, 11, 2), from);
    assert_eq!(sunday, to);
}

#[test]
fn range_week_on_monday_starts_today() {
    let monday = d(2026, 11, 2);
    assert_eq!(monday, TaskViews::range(TaskScope::Week, monday).0);
}

#[test]
fn range_month_covers_first_to_last_day() {
    for (y, m, day, last_day) in [(2026, 11, 6, 30), (2026, 2, 10, 28), (2028, 2, 29, 29), (2026, 12, 31, 31)] {
        let (from, to) = TaskViews::range(TaskScope::Month, d(y, m, day));
        assert_eq!(d(y, m, 1), from, "{y}-{m}-{day}");
        assert_eq!(d(y, m, last_day), to, "{y}-{m}-{day}");
    }
}

#[test]
fn load_uses_get_today_for_day_and_get_range_for_others() {
    let dir = std::env::temp_dir().join(format!("koy-tv-{}", Guid::new_guid().to_string_n()));
    let now = FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 5, 10, 0, 0).unwrap();
    let tasks = TaskService::new(Some(dir.join("tasks.json")), Some(Arc::new(move || now)), None);
    tasks.add("bugün", None, None);
    tasks.add("yarın", None, Some(d(2026, 11, 6)));
    tasks.add("gelecek ay", None, Some(d(2026, 12, 2)));

    let titles = |scope| -> Vec<String> { TaskViews::load(&tasks, scope, thursday()).into_iter().map(|t| t.title).collect() };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_eq!(vec!["bugün"], titles(TaskScope::Day));
        assert_eq!(vec!["bugün", "yarın"], titles(TaskScope::Week));
        assert_eq!(vec!["bugün", "yarın"], titles(TaskScope::Month));
    }));
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
fn try_parse_time_accepts_common_forms() {
    let cases = [
        ("14:30", 14, 30),
        ("9:05", 9, 5),
        ("09.05", 9, 5),
        ("1430", 14, 30),
        ("930", 9, 30),
        (" 7:00 ", 7, 0),
    ];
    for (text, hour, minute) in cases {
        assert_eq!(Some(Some(hm(hour, minute))), TaskViews::try_parse_time(Some(text)), "{text:?}");
    }
}

#[test]
fn try_parse_time_empty_means_no_time() {
    for text in [None, Some(""), Some("   ")] {
        assert_eq!(Some(None), TaskViews::try_parse_time(text), "{text:?}");
    }
}

#[test]
fn try_parse_time_invalid_returns_false() {
    for text in ["25:00", "12:60", "abc", "12", "12:3x"] {
        assert_eq!(None, TaskViews::try_parse_time(Some(text)), "{text:?}");
    }
}

#[test]
fn next_id_is_first_undone() {
    let done = item("a", true, None, None);
    let next = item("b", false, None, None);
    let later = item("c", false, None, None);
    assert_eq!(Some(next.id), TaskViews::next_id(&[done.clone(), next, later]));
    assert_eq!(None, TaskViews::next_id(&[done]));
    assert_eq!(None, TaskViews::next_id(&[]));
}

#[test]
fn label_next_task_without_time_is_now() {
    let (is_now, text) = TaskViews::label(&item("a", false, None, None), true, hm(10, 0), thursday());
    assert!(is_now);
    assert_eq!("", text);
}

#[test]
fn label_next_task_in_the_future_shows_its_time() {
    let (is_now, text) = TaskViews::label(&item("a", false, Some(hm(16, 0)), None), true, hm(10, 0), thursday());
    assert!(!is_now);
    assert_eq!("16:00", text);
}

#[test]
fn label_next_task_whose_time_has_come_is_now() {
    let (is_now, _) = TaskViews::label(&item("a", false, Some(hm(9, 30)), None), true, hm(10, 0), thursday());
    assert!(is_now);
}

#[test]
fn label_leftover_from_earlier_day_is_now_when_next() {
    let old = item("a", false, Some(hm(23, 0)), Some(thursday() - Duration::days(1)));
    assert!(TaskViews::label(&old, true, hm(8, 0), thursday()).0);
}

#[test]
fn label_other_tasks_show_only_their_time() {
    assert_eq!((false, "16:00".to_string()), TaskViews::label(&item("a", false, Some(hm(16, 0)), None), false, hm(10, 0), thursday()));
    assert_eq!((false, String::new()), TaskViews::label(&item("a", false, None, None), false, hm(10, 0), thursday()));
    assert_eq!((false, "08:00".to_string()), TaskViews::label(&item("a", true, Some(hm(8, 0)), None), true, hm(10, 0), thursday()));
}

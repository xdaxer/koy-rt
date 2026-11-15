//! Karşılığı: `TaskServiceTests.cs`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{Duration, FixedOffset, Local, NaiveDate, NaiveTime, TimeZone};

use crate::tasks::*;
use crate::time::DateTimeOffset;

/// C#: `_dir` (Guid'li geçici klasör) + `Dispose` + `_now` + `New(...)`.
struct Fixture {
    dir: PathBuf,
    now: Arc<Mutex<DateTimeOffset>>,
}

impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("koy-tasks-{}", Guid::new_guid()));
        let now = FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 6, 9, 0, 0).unwrap();
        Fixture { dir, now: Arc::new(Mutex::new(now)) }
    }

    fn file_path(&self) -> PathBuf {
        self.dir.join("tasks.json")
    }

    fn new_service(&self, on_error: Option<TaskErrorCallback>) -> TaskService {
        let now = Arc::clone(&self.now);
        TaskService::new(Some(self.file_path()), Some(Arc::new(move || *now.lock().unwrap())), on_error)
    }

    fn today(&self) -> NaiveDate {
        self.now.lock().unwrap().with_timezone(&Local).date_naive()
    }

    fn add_days_to_now(&self, days: i64) {
        let mut n = self.now.lock().unwrap();
        *n += Duration::days(days);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn hm(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn titles(items: &[TaskItem]) -> Vec<String> {
    items.iter().map(|t| t.title.clone()).collect()
}

#[test]
fn add_trims_title_and_rejects_blank() {
    let f = Fixture::new();
    let s = f.new_service(None);
    assert!(s.add("   ", None, None).is_none());
    let t = s.add("  Rapor  ", Some(hm(14, 30)), None).unwrap();
    assert_eq!("Rapor", t.title);
    assert_eq!(Some(hm(14, 30)), t.time);
    assert_eq!(f.today(), t.day);
    assert!(!t.done);
}

#[test]
fn today_sorts_timed_by_time_then_untimed_then_done_last() {
    let f = Fixture::new();
    let s = f.new_service(None);
    s.add("saatsiz", None, None);
    s.add("öğleden sonra", Some(hm(15, 0)), None);
    s.add("sabah", Some(hm(9, 0)), None);
    let done = s.add("bitti", Some(hm(8, 0)), None).unwrap();
    s.set_done(done.id, true);

    assert_eq!(vec!["sabah", "öğleden sonra", "saatsiz", "bitti"], titles(&s.get_today()));
}

#[test]
fn new_day_hides_yesterdays_done_but_keeps_pending_and_does_not_delete() {
    let f = Fixture::new();
    let s = f.new_service(None);
    let finished = s.add("dünkü bitti", None, None).unwrap();
    s.add("dünkü yarım", None, None);
    s.set_done(finished.id, true);

    f.add_days_to_now(1);

    assert_eq!(vec!["dünkü yarım"], titles(&s.get_today()));
    let yesterday = f.today() - Duration::days(1);
    assert_eq!(2, s.get_range(yesterday, yesterday).len());
}

#[test]
fn update_done_remove() {
    let f = Fixture::new();
    let s = f.new_service(None);
    let t = s.add("a", None, None).unwrap();

    assert!(s.update(t.id, " b ", Some(hm(10, 0)), None));
    assert!(!s.update(t.id, "", None, None));
    assert!(!s.update(Guid::new_guid(), "x", None, None));
    assert!(s.set_done(t.id, true));
    assert!(!s.set_done(t.id, true));
    let today = s.get_today();
    assert_eq!(1, today.len());
    let now = &today[0];
    assert_eq!("b", now.title);
    assert!(now.done);

    assert!(s.remove(t.id));
    assert!(!s.remove(t.id));
    assert!(s.get_today().is_empty());
}

#[test]
fn day_and_range_queries() {
    let f = Fixture::new();
    let s = f.new_service(None);
    let today = f.today();
    s.add("bugün", None, Some(today));
    s.add("yarın", None, Some(today + Duration::days(1)));
    s.add("gelecek hafta", None, Some(today + Duration::days(8)));

    assert_eq!(vec!["yarın"], titles(&s.get_day(today + Duration::days(1))));
    assert_eq!(vec!["bugün", "yarın"], titles(&s.get_range(today, today + Duration::days(6))));
}

#[test]
fn persists_across_instances() {
    let f = Fixture::new();
    let s = f.new_service(None);
    s.add("kalıcı", Some(hm(7, 5)), None);

    let today = f.new_service(None).get_today();
    assert_eq!(1, today.len());
    assert_eq!("kalıcı", today[0].title);
    assert_eq!(Some(hm(7, 5)), today[0].time);
}

#[test]
fn keeps_at_most_200_pruning_old_done_first() {
    let f = Fixture::new();
    let s = f.new_service(None);
    let today = f.today();
    let old = s.add("eski bitmiş", None, Some(today - Duration::days(30))).unwrap();
    s.set_done(old.id, true);
    for i in 0..MAX_TASKS {
        s.add(&format!("g{i}"), None, None);
    }

    let all = s.get_range(today - Duration::days(60), today + Duration::days(1));
    assert_eq!(MAX_TASKS, all.len());
    assert!(!all.iter().any(|t| t.title == "eski bitmiş"));
}

#[test]
fn corrupt_file_starts_empty_and_reports_error() {
    let f = Fixture::new();
    std::fs::create_dir_all(&f.dir).unwrap();
    std::fs::write(f.file_path(), "[ bozuk").unwrap();
    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&errors);
    let on_error: TaskErrorCallback = Arc::new(move |src, _| sink.lock().unwrap().push(src.to_string()));

    let s = f.new_service(Some(on_error));

    assert!(s.get_today().is_empty());
    assert_eq!(1, errors.lock().unwrap().len());
    assert!(s.add("yeniden", None, None).is_some());
}

#[test]
fn changed_fires_on_mutations() {
    let f = Fixture::new();
    let s = f.new_service(None);
    let count = Arc::new(AtomicI32::new(0));
    let c = Arc::clone(&count);
    s.changed().subscribe(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });
    let t = s.add("a", None, None).unwrap();
    s.set_done(t.id, true);
    s.remove(t.id);
    assert_eq!(3, count.load(Ordering::SeqCst));
}

#[test]
fn json_format_matches_the_csharp_file() {
    // Rust'a özgü: dosya biçimi (PascalCase, Guid "D", DateOnly, TimeOnly) C# ile uyumlu okunur.
    let json = r#"[{"Id":"0f8fad5b-d9cb-469f-a165-70867728950e","Title":"Rapor","Day":"2026-11-06","Time":"14:30:00","Done":false,"CreatedAt":"2026-11-06T09:00:00+00:00"},
        {"Id":"1f8fad5b-d9cb-469f-a165-70867728950e","Title":"Saatsiz","Day":"2026-11-07","Time":null,"Done":true,"CreatedAt":"2026-11-06T09:00:00.1234567+03:00"}]"#;
    let items: Vec<TaskItem> = serde_json::from_str(json).unwrap();
    assert_eq!(2, items.len());
    assert_eq!("0f8fad5b-d9cb-469f-a165-70867728950e", items[0].id.to_string());
    assert_eq!(Some(hm(14, 30)), items[0].time);
    assert_eq!(None, items[1].time);
    assert!(items[1].done);
}

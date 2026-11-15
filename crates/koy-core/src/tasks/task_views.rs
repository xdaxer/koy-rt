//! Karşılığı: `Tasks/TaskViews.cs`.

use chrono::{Datelike, Months, NaiveDate, NaiveTime, Timelike};

use super::guid::Guid;
use super::i_task_service::ITaskService;
use super::task_item::TaskItem;
use crate::text;

/// Odak sayfasındaki görev listesinin kapsamı (Gün · Hafta · Ay).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskScope {
    #[default]
    Day,
    Week,
    Month,
}

/// Görev listesinin saf yardımcıları: kapsam aralıkları, saat girişi, sıradaki görev. WPF yok.
pub struct TaskViews;

impl TaskViews {
    /// Kapsamın günleri (uçlar dahil). Hafta pazartesi başlar (Türkiye). Gün kapsamı tek gündür; sorgu yerine
    /// `ITaskService::get_today` kullanılır (önceki günden kalan tamamlanmamışlar da görünür).
    pub fn range(scope: TaskScope, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        match scope {
            TaskScope::Week => {
                let offset = (today.weekday().num_days_from_sunday() as i64 + 6) % 7; // Pazartesi = 0
                let monday = today - chrono::Duration::days(offset);
                (monday, monday + chrono::Duration::days(6))
            }
            TaskScope::Month => {
                let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
                (first, first.checked_add_months(Months::new(1)).unwrap() - chrono::Duration::days(1))
            }
            TaskScope::Day => (today, today),
        }
    }

    /// Gün kapsamı için `ITaskService::get_today`, diğerleri için `ITaskService::get_range`.
    pub fn load(tasks: &dyn ITaskService, scope: TaskScope, today: NaiveDate) -> Vec<TaskItem> {
        if scope == TaskScope::Day {
            return tasks.get_today();
        }
        let (from, to) = Self::range(scope, today);
        tasks.get_range(from, to)
    }

    /// "14:30", "9:05", "14.30" ya da "1430" okunur. Boş giriş "saat yok" demektir (`Some(None)`);
    /// geçersiz (25:00, "abc") için `None`. (C#: `bool TryParseTime(string?, out TimeOnly?)`; dönüş
    /// `Some(saat)` = true + saat, `None` = false + boş saat.)
    pub fn try_parse_time(input: Option<&str>) -> Option<Option<NaiveTime>> {
        let t = input.map(text::trim).unwrap_or("");
        if t.is_empty() {
            return Some(None);
        }

        let mut t = t.replace('.', ":");
        let len = t.encode_utf16().count();
        if !t.contains(':') && (len == 3 || len == 4) && t.chars().all(|c| c.is_ascii_digit()) {
            t.insert(t.len() - 2, ':');
        }

        // TryParseExact(["H:mm", "HH:mm"]): bir ya da iki haneli saat, tam iki haneli dakika.
        let (hour, minute) = t.split_once(':')?;
        let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if !(hour.len() == 1 || hour.len() == 2) || minute.len() != 2 || !all_digits(hour) || !all_digits(minute) {
            return None;
        }
        let parsed = NaiveTime::from_hms_opt(hour.parse().ok()?, minute.parse().ok()?, 0)?;
        Some(Some(parsed))
    }

    /// "14:30" biçimi (kültürden bağımsız).
    pub fn format_time(time: NaiveTime) -> String {
        format!("{:02}:{:02}", time.hour(), time.minute())
    }

    /// Sıradaki görev: listedeki ilk tamamlanmamış (liste zaten saate göre sıralı).
    pub fn next_id(items: &[TaskItem]) -> Option<Guid> {
        items.iter().find(|t| !t.done).map(|t| t.id)
    }

    /// Sıradaki görevin sağ etiketi: saati geldiyse ya da saati yoksa "şimdi" (`true`), yoksa saatin kendisi.
    /// Dönüş (şimdiMi, saatMetni); saati olmayan ve sıradaki olmayan görev için saat metni boş.
    pub fn label(task: &TaskItem, is_next: bool, now: NaiveTime, today: NaiveDate) -> (bool, String) {
        let time_text = task.time.map(Self::format_time).unwrap_or_default();
        if !is_next || task.done {
            return (false, time_text);
        }
        let due = task.day < today || (task.day == today && (task.time.is_none() || task.time.unwrap() <= now));
        (due, if due { String::new() } else { time_text })
    }
}

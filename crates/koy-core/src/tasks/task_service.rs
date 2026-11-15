//! Karşılığı: `Tasks/TaskService.cs`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::{Local, NaiveDate, NaiveTime};

use super::guid::Guid;
use super::i_task_service::{ITaskService, MAX_TASKS};
use super::task_item::TaskItem;
use crate::event::Event;
use crate::settings::JsonListFile;
use crate::time::DateTimeOffset;

/// C#: `Func<DateTimeOffset>` (saat kaynağı).
pub type NowFn = Arc<dyn Fn() -> DateTimeOffset + Send + Sync>;

/// C#: `Action<string, Exception>` (dosya okuma/yazma hata bildirimi).
pub type TaskErrorCallback = Arc<dyn Fn(&str, &dyn std::error::Error) + Send + Sync>;

/// `ITaskService`'in dosya destekli uygulaması. Dosya yoksa/bozuksa boş liste ile devam eder.
pub struct TaskService {
    /// C#: `_gate` + `_items` (kilit listeyi korur; dosya yazımı da kilit altında yapılır).
    items: Mutex<Vec<TaskItem>>,
    file: JsonListFile<TaskItem>,
    now: NowFn,
    changed: Event<()>,
}

impl TaskService {
    /// C#: `new TaskService(path = null, now = null, onError = null)`. `path` boşsa ayar klasöründeki `tasks.json`,
    /// `now` boşsa `DateTimeOffset.Now`.
    pub fn new(path: Option<PathBuf>, now: Option<NowFn>, on_error: Option<TaskErrorCallback>) -> Self {
        let now: NowFn = now.unwrap_or_else(|| Arc::new(|| chrono::Local::now().fixed_offset()));
        let file = JsonListFile::<TaskItem>::new(path.unwrap_or_else(|| crate::settings::in_settings_folder("tasks.json")), on_error);
        let mut items: Vec<TaskItem> = file.load().into_iter().filter(|t| !crate::text::is_null_or_white_space(Some(&t.title))).collect();
        if Self::prune(&mut items) {
            file.save(&items);
        }

        TaskService { items: Mutex::new(items), file, now, changed: Event::new() }
    }

    fn today(&self) -> NaiveDate {
        // C#: `DateOnly.FromDateTime(_now().LocalDateTime)`.
        (self.now)().with_timezone(&Local).date_naive()
    }

    fn raise_changed(&self) {
        self.changed.invoke(&());
    }

    /// 200'ü aşanı budar: önce tamamlananların en eskisi, tamamlanan kalmadıysa en eski kayıt.
    /// Bugünün görevleri tamamlanmamışken en son budanır.
    fn prune(items: &mut Vec<TaskItem>) -> bool {
        let mut removed = false;
        while items.len() > MAX_TASKS {
            // OrderByDescending(Done).ThenBy(Day).ThenBy(CreatedAt).First(): eşitlikte ilk kayıt (kararlı).
            let victim = items
                .iter()
                .enumerate()
                .min_by_key(|(_, t)| (!t.done, t.day, t.created_at))
                .map(|(i, _)| i)
                .unwrap();
            items.remove(victim);
            removed = true;
        }

        removed
    }
}

impl ITaskService for TaskService {
    fn changed(&self) -> &Event<()> {
        &self.changed
    }

    fn get_today(&self) -> Vec<TaskItem> {
        let today = self.today();
        let items = self.items.lock().unwrap();
        let mut list: Vec<TaskItem> = items.iter().filter(|t| t.day == today || (t.day < today && !t.done)).cloned().collect();
        list.sort_by_key(|t| (t.done, t.day, t.time.is_none(), t.time, t.created_at));
        list
    }

    fn get_day(&self, day: NaiveDate) -> Vec<TaskItem> {
        self.get_range(day, day)
    }

    fn get_range(&self, from: NaiveDate, to: NaiveDate) -> Vec<TaskItem> {
        let items = self.items.lock().unwrap();
        let mut list: Vec<TaskItem> = items.iter().filter(|t| t.day >= from && t.day <= to).cloned().collect();
        list.sort_by_key(|t| (t.day, t.done, t.time.is_none(), t.time, t.created_at));
        list
    }

    fn add(&self, title: &str, time: Option<NaiveTime>, day: Option<NaiveDate>) -> Option<TaskItem> {
        let title = crate::text::trim(title);
        if title.is_empty() {
            return None;
        }

        let item = {
            let mut items = self.items.lock().unwrap();
            let item = TaskItem::new(Guid::new_guid(), title, day.unwrap_or_else(|| self.today()), time, false, (self.now)());
            items.push(item.clone());
            Self::prune(&mut items);
            self.file.save(&items);
            item
        };

        self.raise_changed();
        Some(item)
    }

    fn update(&self, id: Guid, title: &str, time: Option<NaiveTime>, day: Option<NaiveDate>) -> bool {
        let title = crate::text::trim(title);
        if title.is_empty() {
            return false;
        }

        {
            let mut items = self.items.lock().unwrap();
            let Some(index) = items.iter().position(|t| t.id == id) else {
                return false;
            };
            let old_day = items[index].day;
            items[index] = TaskItem { title: title.to_string(), time, day: day.unwrap_or(old_day), ..items[index].clone() };
            self.file.save(&items);
        }

        self.raise_changed();
        true
    }

    fn set_done(&self, id: Guid, done: bool) -> bool {
        {
            let mut items = self.items.lock().unwrap();
            let Some(index) = items.iter().position(|t| t.id == id) else {
                return false;
            };
            if items[index].done == done {
                return false;
            }
            items[index].done = done;
            self.file.save(&items);
        }

        self.raise_changed();
        true
    }

    fn remove(&self, id: Guid) -> bool {
        {
            let mut items = self.items.lock().unwrap();
            let before = items.len();
            items.retain(|t| t.id != id);
            if items.len() == before {
                return false;
            }
            self.file.save(&items);
        }

        self.raise_changed();
        true
    }
}

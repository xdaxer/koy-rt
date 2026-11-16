//! Odak sayfasındaki görev listesi (Bugün kartı): ekle (başlık + isteğe bağlı saat), tamamla, sil ve Gün · Hafta · Ay
//! kapsamı. Mantık `ITaskService` ve `TaskViews` içinde; burası yalnız bağlar. Servise yalnız sayfa görünürken abone
//! olunur; hiçbir zamanlayıcı yoktur. Karşılığı: `Features/Focus/TasksViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use chrono::Local;
use koy_core::event::Subscription;
use koy_core::tasks::{ITaskService, TaskScope, TaskViews};

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::{self, Strings};

/// Görev kimliği (core `TaskItem::id`).
pub type TaskId = koy_core::tasks::Guid;

/// Görev listesinin bir satırı (değişmez görünüm; tamamlama ve silme komutları üst modelde).
#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: TaskId,
    pub title: String,
    pub done: bool,
    /// Listedeki ilk tamamlanmamış görev: vurgulu çizilir.
    pub is_next: bool,
    /// Sıradaki görevin saati geldi ya da saati yok: sağda "şimdi" yazar.
    pub is_now: bool,
    /// "16:00" ya da boş.
    pub time_text: String,
    /// Hafta / Ay kapsamında "Pzt 2"; Gün kapsamında boş.
    pub day_text: String,
}

#[derive(Debug, Clone, Default)]
pub struct TasksState {
    pub rows: Vec<TaskRow>,
    pub scope: TaskScope,
    /// Kartın başlığı: Bugün / Bu hafta / Bu ay.
    pub title: String,
    pub is_empty: bool,
    pub new_title: String,
    pub new_time: String,
    /// Saat alanındaki yazı geçersiz (alan kırmızı yazılır, görev eklenmez).
    pub new_time_invalid: bool,
}

pub struct TasksViewModel {
    tasks: Arc<dyn ITaskService>,
    state: RefCell<TasksState>,
    pub changed: Notifier,
    active: Cell<Option<Subscription>>,
    disposed: Cell<bool>,
}

impl TasksViewModel {
    pub fn new(tasks: Arc<dyn ITaskService>) -> Rc<Self> {
        Rc::new(TasksViewModel {
            tasks,
            state: RefCell::new(TasksState { title: Strings::get("TaskToday"), is_empty: true, ..Default::default() }),
            changed: Notifier::default(),
            active: Cell::new(None),
            disposed: Cell::new(false),
        })
    }

    pub fn state(&self) -> std::cell::Ref<'_, TasksState> {
        self.state.borrow()
    }

    /// Sayfa görünürken true: servis olayına abone olur ve listeyi tazeler (gün değişmiş olabilir).
    pub fn set_active(self: &Rc<Self>, active: bool) {
        let is_active = {
            let a = self.active.take();
            let r = a.is_some();
            self.active.set(a);
            r
        };
        if self.disposed.get() || active == is_active {
            return;
        }
        if active {
            let me = UiRef::new(self);
            let sub = self.tasks.changed().subscribe(move |_| {
                // Olay arka plandan da gelebilir: her durumda arayüz iş parçacığında yeniden yüklenir.
                dispatch::post(move || {
                    if let Some(vm) = me.upgrade()
                        && !vm.disposed.get()
                        && vm.is_active()
                    {
                        vm.reload();
                        vm.changed.notify();
                    }
                });
            });
            self.active.set(Some(sub));
            self.reload();
            self.changed.notify();
        } else if let Some(sub) = self.active.take() {
            self.tasks.changed().unsubscribe(sub);
        }
    }

    fn is_active(&self) -> bool {
        let a = self.active.take();
        let r = a.is_some();
        self.active.set(a);
        r
    }

    fn reload(&self) {
        let now = Local::now().naive_local();
        let (today, time) = (now.date(), now.time());
        let scope = self.state.borrow().scope;
        let items = TaskViews::load(&*self.tasks, scope, today);
        let next = TaskViews::next_id(&items);
        let culture = strings::culture();
        let rows: Vec<TaskRow> = items
            .iter()
            .map(|item| {
                let is_next = Some(item.id) == next;
                let day_text = if scope == TaskScope::Day {
                    String::new()
                } else {
                    koy_core::clock::format_date_time(item.day.and_time(chrono::NaiveTime::MIN), None, "ddd d", &culture).unwrap_or_default()
                };
                let (is_now, time_text) = TaskViews::label(item, is_next, time, today);
                TaskRow {
                    id: item.id,
                    title: item.title.clone(),
                    done: item.done,
                    is_next: is_next && !item.done,
                    is_now,
                    time_text,
                    day_text,
                }
            })
            .collect();
        let mut st = self.state.borrow_mut();
        st.is_empty = rows.is_empty();
        st.rows = rows;
    }

    pub fn set_scope(&self, scope: TaskScope) {
        {
            let mut st = self.state.borrow_mut();
            if st.scope == scope {
                return;
            }
            st.scope = scope;
            st.title = Strings::get(match scope {
                TaskScope::Week => "TaskThisWeek",
                TaskScope::Month => "TaskThisMonth",
                _ => "TaskToday",
            });
        }
        self.reload();
        self.changed.notify();
    }

    pub fn set_new_title(&self, value: &str) {
        self.state.borrow_mut().new_title = value.to_string();
    }

    pub fn set_new_time(&self, value: &str) {
        let mut st = self.state.borrow_mut();
        st.new_time = value.to_string();
        st.new_time_invalid = false;
    }

    /// Başlık boşsa ya da saat geçersizse eklenmez; saat boşsa görev saatsizdir. Hep bugüne eklenir.
    pub fn add(&self) {
        let (title, time_text) = {
            let st = self.state.borrow();
            (st.new_title.clone(), st.new_time.clone())
        };
        if title.trim().is_empty() {
            return;
        }
        let Some(time) = TaskViews::try_parse_time(Some(&time_text)) else {
            self.state.borrow_mut().new_time_invalid = true;
            self.changed.notify();
            return;
        };
        if self.tasks.add(&title, time, None).is_none() {
            return;
        }
        {
            let mut st = self.state.borrow_mut();
            st.new_title.clear();
            st.new_time.clear();
        }
        self.changed.notify();
    }

    pub fn toggle(&self, id: TaskId) {
        let done = self.state.borrow().rows.iter().find(|r| r.id == id).map(|r| r.done);
        if let Some(done) = done {
            self.tasks.set_done(id, !done);
        }
    }

    pub fn remove(&self, id: TaskId) {
        self.tasks.remove(id);
    }

    pub fn dispose(&self) {
        if self.disposed.get() {
            return;
        }
        if let Some(sub) = self.active.take() {
            self.tasks.changed().unsubscribe(sub);
        }
        self.disposed.set(true);
    }
}

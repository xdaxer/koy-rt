//! Takvim sayfası: ay / hafta / gün görünümü ve yerel etkinlikler. Mantık core'da (MonthGrid, EventAgenda, EventBook);
//! bu tür yalnız bağlar. Zamanlayıcı yoktur: etkinlik defterine yalnız sayfa görünürken abone olunur.
//! Karşılığı: `Features/Calendar/CalendarViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use chrono::{Datelike, Local, Months, NaiveDate, NaiveTime};
use koy_core::calendar::{CalendarEventSource, CalendarSettingsStore, CalendarViewMode, DayAgenda, EventAgenda, MonthGrid};
use koy_core::clock::{ClockFormatPolicy, Culture, DayOfWeek, format_date_time};
use koy_core::settings::AppSettings;

use super::shared_events::SharedEvents;
use crate::infra::observable::Notifier;
use crate::resources::strings::{self, Strings};

/// Ay ızgarasının bir hücresi.
#[derive(Debug, Clone)]
pub struct CalendarCell {
    pub date: NaiveDate,
    pub is_current_month: bool,
    pub is_today: bool,
    pub is_selected: bool,
    pub has_events: bool,
}

/// Hafta görünümünün bir satırı.
#[derive(Debug, Clone)]
pub struct WeekDayRow {
    pub date: NaiveDate,
    pub label: String,
    pub is_today: bool,
    pub is_selected: bool,
    pub summary: String,
    pub has_events: bool,
}

/// Seçili günün bir etkinlik satırı.
#[derive(Debug, Clone)]
pub struct EventRow {
    pub id: String,
    pub time_text: String,
    pub title: String,
    pub source: CalendarEventSource,
}

impl EventRow {
    pub fn can_delete(&self) -> bool {
        self.source == CalendarEventSource::Local
    }
    pub fn source_badge(&self) -> &'static str {
        match self.source {
            CalendarEventSource::Google => "G",
            CalendarEventSource::Outlook => "O",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CalendarState {
    pub view_mode: CalendarViewMode,
    /// Seçili gün: ay/hafta/gün görünümünün ve sağdaki gündemin dayanağı.
    pub anchor: Option<NaiveDate>,
    pub header_title: String,
    /// Sağ kartın başlığı ("6 Kasım Perşembe").
    pub day_title: String,
    pub cells: Vec<CalendarCell>,
    pub week_days: Vec<WeekDayRow>,
    pub day_events: Vec<EventRow>,
    pub new_event_title: String,
    pub new_event_time: String,
    /// Girilen saat geçersiz (alan çerçevesi kırmızı).
    pub new_event_time_invalid: bool,
}

pub struct CalendarViewModel {
    events: Rc<SharedEvents>,
    culture: Culture,
    time_pattern: String,
    first_day: DayOfWeek,
    show_week_number: bool,
    pub weekday_headers: Vec<String>,
    state: RefCell<CalendarState>,
    pub changed: Notifier,
    sub: Cell<Option<u64>>,
    synced_this_session: Cell<bool>,
    disposed: Cell<bool>,
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

impl CalendarViewModel {
    pub fn new(settings: &AppSettings, events: Rc<SharedEvents>) -> Rc<Self> {
        let culture = strings::culture_for(&settings.general.language);
        // Saat ve hafta ayarları; ayar değişince çentik yeniden kurulur, burada bir kez okunur.
        let time_pattern = ClockFormatPolicy::resolve_time_pattern(settings.notch.clock_hour_format, false, &culture);
        let first_day = ClockFormatPolicy::resolve_first_day_of_week(settings.notch.clock_first_day_of_week, culture.first_day_of_week);
        // Pazar = 0 sırasıyla iki harfli başlıklar; ızgaranın ilk gününe göre döndürülür.
        let by_day: [&str; 7] = if culture.name.to_ascii_lowercase().starts_with("tr") {
            ["Pz", "Pt", "Sa", "Ça", "Pe", "Cu", "Ct"]
        } else {
            ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
        };
        let weekday_headers = MonthGrid::weekday_order(first_day).iter().map(|d| by_day[*d as usize].to_string()).collect();
        let vm = Rc::new(CalendarViewModel {
            events,
            culture,
            time_pattern,
            first_day,
            show_week_number: settings.notch.clock_show_week_number,
            weekday_headers,
            state: RefCell::new(CalendarState { anchor: Some(today()), new_event_time: "09:00".into(), ..Default::default() }),
            changed: Notifier::default(),
            sub: Cell::new(None),
            synced_this_session: Cell::new(false),
            disposed: Cell::new(false),
        });
        // Defter burada okunmaz (disk işi sayfa görününce yapılır); yalnız başlık kurulur.
        vm.update_titles();
        vm
    }

    pub fn events(&self) -> &Rc<SharedEvents> {
        &self.events
    }

    pub fn state(&self) -> std::cell::Ref<'_, CalendarState> {
        self.state.borrow()
    }

    fn anchor(&self) -> NaiveDate {
        self.state.borrow().anchor.unwrap_or_else(today)
    }

    fn is_active(&self) -> bool {
        let s = self.sub.get();
        s.is_some()
    }

    /// Sayfa görünürken true: defter olayına abone olur, bugünü yeniler; false: abonelik kesilir.
    pub fn set_active(self: &Rc<Self>, active: bool) {
        if self.disposed.get() || active == self.is_active() {
            return;
        }
        if active {
            let weak = Rc::downgrade(self);
            let id = self.events.changed.subscribe(move || {
                // Defter yalnız arayüz iş parçacığında değişir; yine de C#'taki gibi bir sonraki tura bırakılır.
                let weak = weak.clone();
                dispatch_local(move || {
                    if let Some(vm) = weak.upgrade()
                        && !vm.disposed.get()
                        && vm.is_active()
                    {
                        vm.refresh();
                        vm.changed.notify();
                    }
                });
            });
            self.sub.set(Some(id));
            // Ayarlardan bağlanan ya da kesilen hesap önbelleği değiştirir: harici etkinlikler önbellekten yeniden okunur.
            self.events.reload_external_from_cache();
            // Uygulama gece boyunca açık kalmış olabilir.
            self.state.borrow_mut().anchor = Some(today());
            self.refresh();
            self.changed.notify();
            if !self.synced_this_session.replace(true) {
                let cal = CalendarSettingsStore::new(None).load();
                if cal.google_enabled || cal.outlook_enabled {
                    self.events.sync_external(cal);
                }
            }
        } else if let Some(id) = self.sub.take() {
            self.events.changed.unsubscribe(id);
        }
    }

    fn fmt(&self, date: NaiveDate, pattern: &str) -> String {
        format_date_time(date.and_time(NaiveTime::MIN), None, pattern, &self.culture).unwrap_or_default()
    }

    fn update_titles(&self) {
        let anchor = self.anchor();
        let mut day_title = self.fmt(anchor, "d MMMM dddd");
        if self.show_week_number {
            day_title += " · ";
            day_title += &Strings::format(
                "CalendarWeekNumber",
                &[&ClockFormatPolicy::week_of_year(anchor.and_time(NaiveTime::MIN), self.first_day).to_string()],
            );
        }
        let mode = self.state.borrow().view_mode;
        let header = match mode {
            CalendarViewMode::Week => {
                let first = EventAgenda::week_start_with_first_day(anchor, self.first_day);
                let last = first + chrono::Duration::days(6);
                if first.month() == last.month() {
                    format!("{} – {}", first.day(), self.fmt(last, "d MMM"))
                } else {
                    format!("{} – {}", self.fmt(first, "d MMM"), self.fmt(last, "d MMM"))
                }
            }
            _ => self.fmt(anchor, "MMMM yyyy"),
        };
        let mut st = self.state.borrow_mut();
        st.day_title = day_title;
        st.header_title = header;
    }

    fn refresh(&self) {
        self.update_titles();
        let today = today();
        let anchor = self.anchor();
        let all = if self.is_active() { self.events.events() } else { Vec::new() };
        let with_events: HashSet<NaiveDate> = all.iter().map(|e| e.start.date()).collect();
        let mode = self.state.borrow().view_mode;
        let fmt_time = |t: chrono::NaiveDateTime| format_date_time(t, None, &self.time_pattern, &self.culture).unwrap_or_default();
        let mut st = self.state.borrow_mut();
        match mode {
            CalendarViewMode::Month => {
                st.cells = MonthGrid::build_with_first_day(anchor.year(), anchor.month(), Some(today), self.first_day)
                    .into_iter()
                    .map(|d| CalendarCell {
                        date: d.date,
                        is_current_month: d.is_current_month,
                        is_today: d.is_today,
                        is_selected: d.date == anchor,
                        has_events: with_events.contains(&d.date),
                    })
                    .collect();
            }
            CalendarViewMode::Week => {
                st.week_days = EventAgenda::for_week_with_first_day(&all, anchor, today, self.first_day)
                    .iter()
                    .map(|d| self.build_week_row(d, anchor, &fmt_time))
                    .collect();
            }
            _ => {}
        }
        st.day_events = EventAgenda::for_day(&all, anchor)
            .into_iter()
            .map(|e| EventRow { id: e.id.clone(), time_text: fmt_time(e.start), title: e.title.clone(), source: e.source })
            .collect();
    }

    fn build_week_row(&self, day: &DayAgenda, anchor: NaiveDate, fmt_time: &dyn Fn(chrono::NaiveDateTime) -> String) -> WeekDayRow {
        let label = self.fmt(day.date, "ddd d");
        let mut summary = String::new();
        if let Some(first) = day.events.first() {
            summary = format!("{} {}", fmt_time(first.start), first.title);
            if day.events.len() > 1 {
                summary += &format!("  +{}", day.events.len() - 1);
            }
        }
        WeekDayRow {
            date: day.date,
            label,
            is_today: day.is_today,
            is_selected: day.date == anchor,
            summary,
            has_events: !day.events.is_empty(),
        }
    }

    // ---- Komutlar ----

    pub fn set_view(&self, mode: CalendarViewMode) {
        if self.state.borrow().view_mode == mode {
            return;
        }
        self.state.borrow_mut().view_mode = mode;
        self.refresh();
        self.changed.notify();
    }

    pub fn previous(&self) {
        self.move_by(-1);
    }

    pub fn next(&self) {
        self.move_by(1);
    }

    fn move_by(&self, direction: i32) {
        let anchor = self.anchor();
        let mode = self.state.borrow().view_mode;
        let new = match mode {
            CalendarViewMode::Month => {
                // C# DateOnly.AddMonths: ayın son günü taşarsa yeni ayın son gününe kırpılır.
                if direction >= 0 { anchor.checked_add_months(Months::new(1)) } else { anchor.checked_sub_months(Months::new(1)) }
            }
            CalendarViewMode::Week => Some(anchor + chrono::Duration::days(7 * direction as i64)),
            _ => Some(anchor + chrono::Duration::days(direction as i64)),
        };
        self.state.borrow_mut().anchor = Some(new.unwrap_or(anchor));
        self.refresh();
        self.changed.notify();
    }

    pub fn go_today(&self) {
        self.state.borrow_mut().anchor = Some(today());
        self.refresh();
        self.changed.notify();
    }

    /// Ay hücresine ya da hafta satırına tıklanınca o gün seçilir; sağdaki gündem onu gösterir.
    pub fn select_day(&self, date: NaiveDate) {
        self.state.borrow_mut().anchor = Some(date);
        self.refresh();
        self.changed.notify();
    }

    pub fn set_new_event_title(&self, v: &str) {
        self.state.borrow_mut().new_event_title = v.to_string();
    }

    pub fn set_new_event_time(&self, v: &str) {
        let mut st = self.state.borrow_mut();
        st.new_event_time = v.to_string();
        st.new_event_time_invalid = false;
    }

    pub fn add_event(&self) {
        let (title, time_text) = {
            let st = self.state.borrow();
            (st.new_event_title.clone(), st.new_event_time.clone())
        };
        let time = EventAgenda::try_parse_time(Some(&time_text));
        self.state.borrow_mut().new_event_time_invalid = time.is_none();
        let Some(time) = time else {
            self.changed.notify();
            return;
        };
        if self.events.add(&title, self.anchor(), time).is_none() {
            return;
        }
        self.state.borrow_mut().new_event_title.clear();
        self.changed.notify();
    }

    pub fn delete_event(&self, id: &str) {
        self.events.remove(id);
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.disposed.get() {
            return;
        }
        self.set_active(false);
        self.disposed.set(true);
    }
}

fn dispatch_local(f: impl FnOnce() + 'static) {
    crate::infra::dispatch::post_local(f);
}

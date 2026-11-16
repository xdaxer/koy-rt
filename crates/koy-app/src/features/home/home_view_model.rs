//! Ana sekmesinin veri kaynağı. İki yüzü var: tam sayfa müzik ve Ana (gün, hava, sıradaki etkinlikler, Odak satırı,
//! sistem, hızlı başlat). Yalnız mevcut görünüm modellerini birleştirir.
//! Karşılığı: `Features/Home/HomeViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use chrono::{Local, NaiveDate, NaiveDateTime, Timelike};
use koy_core::calendar::{CalendarEvent, EventAgenda};
use koy_core::clock::format_date_time;
use koy_core::notch::{HomeCard, HomeCardLayout, NotchHomePage};
use koy_core::shell::IShellActionService;

use super::quick_launch_view_model::QuickLaunchViewModel;
use crate::features::calendar::shared_events::SharedEvents;
use crate::features::control_center::bluetooth_devices_view_model::BluetoothDevicesViewModel;
use crate::features::focus::focus_view_model::FocusViewModel;
use crate::features::music::music_view_model::MusicViewModel;
use crate::features::status::status_view_model::StatusViewModel;
use crate::features::system_info::system_stats_view_model::SystemStatsViewModel;
use crate::infra::observable::Notifier;
use crate::resources::strings::{self, Strings};

/// Ana'daki bir etkinlik satırı: gün ve saat ("Yarın 10:00"), başlık ve kalan süre ("10 sa 13 dk sonra").
#[derive(Debug, Clone)]
pub struct UpcomingEventRow {
    pub id: String,
    pub date: NaiveDate,
    pub when: String,
    pub title: String,
    pub until: String,
}

impl UpcomingEventRow {
    pub fn from(e: &CalendarEvent, now: NaiveDateTime) -> Self {
        let date = e.start.date();
        let day = match (date - now.date()).num_days() {
            0 => Strings::get("HomeToday"),
            1 => Strings::get("HomeTomorrow"),
            _ => format_date_time(e.start, None, "ddd d", &strings::culture()).unwrap_or_default(),
        };
        let midnight = e.start.time().num_seconds_from_midnight() == 0 && e.start.time().nanosecond() == 0;
        let time = if e.duration_minutes == 0 && midnight { String::new() } else { format!(" {}", e.start.format("%H:%M")) };
        UpcomingEventRow {
            id: e.id.clone(),
            date,
            when: format!("{day}{time}"),
            title: e.title.clone(),
            until: format_until(EventAgenda::until(e, now).total_seconds()),
        }
    }
}

fn format_until(left_seconds: f64) -> String {
    if left_seconds <= 0.0 {
        return Strings::get("HomeEventNow");
    }
    let minutes = (left_seconds / 60.0).ceil() as i64;
    if minutes < 60 {
        return Strings::format("HomeEventInMinutes", &[&minutes.to_string()]);
    }
    if minutes < 24 * 60 {
        return Strings::format("HomeEventInHours", &[&(minutes / 60).to_string(), &(minutes % 60).to_string()]);
    }
    // C# Math.Round: bankacı yuvarlaması.
    let days = koy_core::text::round(left_seconds / 86400.0);
    Strings::format("HomeEventInDays", &[&days.to_string()])
}

pub struct HomeViewModel {
    pub music: Rc<MusicViewModel>,
    pub status: Rc<StatusViewModel>,
    pub focus: Rc<FocusViewModel>,
    pub stats: Rc<SystemStatsViewModel>,
    pub quick_launch: Rc<QuickLaunchViewModel>,
    pub bluetooth: Option<Rc<BluetoothDevicesViewModel>>,
    /// Aktif kartlar ve sıraları.
    pub cards: Vec<HomeCard>,
    shell: Option<Arc<dyn IShellActionService>>,
    open_focus: Option<Box<dyn Fn()>>,
    /// Sistem kartına tıklanınca Sistem sayfası açılır (NotchViewModel verir).
    pub open_system_page: RefCell<Option<Box<dyn Fn()>>>,
    /// Konum seçilmemişse Koy penceresinin Çentik sayfası açılır (App verir).
    pub choose_weather_location: RefCell<Option<Box<dyn Fn()>>>,
    page: Cell<NotchHomePage>,
    events: RefCell<Option<Rc<SharedEvents>>>,
    open_day: RefCell<Option<Box<dyn Fn(NaiveDate)>>>,
    events_sub: Cell<Option<u64>>,
    active: Cell<bool>,
    last_minute: RefCell<String>,
    next_event: RefCell<Option<UpcomingEventRow>>,
    later_events: RefCell<Vec<UpcomingEventRow>>,
    status_sub: Cell<Option<u64>>,
    disposed: Cell<bool>,
    pub changed: Notifier,
}

pub struct HomeParts {
    pub music: Rc<MusicViewModel>,
    pub status: Rc<StatusViewModel>,
    pub focus: Rc<FocusViewModel>,
    pub stats: Rc<SystemStatsViewModel>,
    pub quick_launch: Rc<QuickLaunchViewModel>,
    pub shell: Option<Arc<dyn IShellActionService>>,
    pub open_focus: Option<Box<dyn Fn()>>,
    pub bluetooth: Option<Rc<BluetoothDevicesViewModel>>,
    pub home_cards: Option<Vec<HomeCard>>,
}

impl HomeViewModel {
    pub fn new(p: HomeParts) -> Rc<Self> {
        let cards = HomeCardLayout::normalize(p.home_cards.as_deref()).iter().copied().collect();
        let vm = Rc::new(HomeViewModel {
            music: p.music,
            status: p.status,
            focus: p.focus,
            stats: p.stats,
            quick_launch: p.quick_launch,
            bluetooth: p.bluetooth,
            cards,
            shell: p.shell,
            open_focus: p.open_focus,
            open_system_page: RefCell::new(None),
            choose_weather_location: RefCell::new(None),
            page: Cell::new(NotchHomePage::Home),
            events: RefCell::new(None),
            open_day: RefCell::new(None),
            events_sub: Cell::new(None),
            active: Cell::new(false),
            last_minute: RefCell::new(String::new()),
            next_event: RefCell::new(None),
            later_events: RefCell::new(Vec::new()),
            status_sub: Cell::new(None),
            disposed: Cell::new(false),
            changed: Notifier::default(),
        });
        // Saat dakikada bir değişir: "… sonra" metni ve bitmiş etkinliklerin düşmesi için yeter (ayrı zamanlayıcı yok).
        let weak = Rc::downgrade(&vm);
        let id = vm.status.changed.subscribe(move || {
            if let Some(vm) = weak.upgrade() {
                let time = vm.status.state().current_time.clone();
                if vm.active.get() && *vm.last_minute.borrow() != time {
                    *vm.last_minute.borrow_mut() = time;
                    vm.refresh_upcoming();
                }
                vm.changed.notify();
            }
        });
        vm.status_sub.set(Some(id));
        vm
    }

    pub fn page(&self) -> NotchHomePage {
        self.page.get()
    }

    pub fn set_page(&self, page: NotchHomePage) {
        if self.page.replace(page) != page {
            self.changed.notify();
        }
    }

    pub fn is_music_page(&self) -> bool {
        self.page.get() == NotchHomePage::Music
    }

    pub fn is_home_page(&self) -> bool {
        self.page.get() == NotchHomePage::Home
    }

    pub fn next_event(&self) -> Option<UpcomingEventRow> {
        self.next_event.borrow().clone()
    }

    pub fn later_events(&self) -> Vec<UpcomingEventRow> {
        self.later_events.borrow().clone()
    }

    fn has(&self, c: HomeCard) -> bool {
        self.cards.contains(&c)
    }

    /// Odak satırı (Zamanlayıcılar kartı açık).
    pub fn show_focus_row(&self) -> bool {
        self.has(HomeCard::Timers)
    }

    /// Kulaklık/ses cihazı kartı (cihaz var ve Kulaklık kartı açık).
    pub fn show_headphone_card(&self) -> bool {
        self.bluetooth.as_ref().is_some_and(|b| b.has_primary_audio()) && self.has(HomeCard::HeadphoneBattery)
    }

    pub fn show_system_card(&self) -> bool {
        self.has(HomeCard::SystemNetwork)
    }

    pub fn show_quick_launch_card(&self) -> bool {
        self.has(HomeCard::QuickLaunch)
    }

    pub fn show_weather_card(&self) -> bool {
        self.has(HomeCard::Weather)
    }

    /// Hava durumu kartı konum seçilmişse hava gösterir, seçilmemişse konum seçmeye yönlendirir.
    pub fn show_weather(&self) -> bool {
        self.status.state().has_weather_location && self.show_weather_card()
    }

    pub fn show_no_location(&self) -> bool {
        !self.status.state().has_weather_location && self.show_weather_card()
    }

    /// Takvim defterini bağlar; `open_day` etkinliğe tıklanınca Takvim'i o günde açar.
    pub fn attach_calendar(&self, events: Rc<SharedEvents>, open_day: Box<dyn Fn(NaiveDate)>) {
        *self.events.borrow_mut() = Some(events);
        *self.open_day.borrow_mut() = Some(open_day);
    }

    /// Ana yüzü görünürken sıradaki listesi kurulur ve dakikada bir tazelenir; değilken abonelik yok.
    pub fn set_active(self: &Rc<Self>, active: bool) {
        if self.active.replace(active) == active {
            return;
        }
        let Some(events) = self.events.borrow().clone() else { return };
        if active {
            let weak = Rc::downgrade(self);
            let id = events.changed.subscribe(move || {
                if let Some(vm) = weak.upgrade() {
                    vm.refresh_upcoming();
                    vm.changed.notify();
                }
            });
            self.events_sub.set(Some(id));
            // Ayarlardan bağlanan ya da kesilen takvim hesabı "Bugün" kartına da yansısın.
            events.reload_external_from_cache();
            self.refresh_upcoming();
            self.changed.notify();
        } else if let Some(id) = self.events_sub.take() {
            events.changed.unsubscribe(id);
        }
    }

    fn refresh_upcoming(&self) {
        let Some(events) = self.events.borrow().clone() else { return };
        let now = Local::now().naive_local();
        let list = EventAgenda::upcoming_default(&events.events(), now, 3);
        *self.next_event.borrow_mut() = list.first().map(|e| UpcomingEventRow::from(e, now));
        *self.later_events.borrow_mut() = list.iter().skip(1).map(|e| UpcomingEventRow::from(e, now)).collect();
    }

    pub fn open_event(&self, date: NaiveDate) {
        if let Some(f) = self.open_day.borrow().as_ref() {
            f(date);
        }
    }

    /// Win+N ile Windows Bildirim Merkezi'ni açar.
    pub fn open_notification_center(&self) {
        if let Some(s) = &self.shell {
            s.open_notification_center();
        }
    }

    /// "Odak · 18 dk kaldı" satırına tıklanınca Odak sayfası açılır.
    pub fn open_focus(&self) {
        if let Some(f) = &self.open_focus {
            f();
        }
    }

    pub fn open_system(&self) {
        if let Some(f) = self.open_system_page.borrow().as_ref() {
            f();
        }
    }

    pub fn choose_weather_location(&self) {
        if let Some(f) = self.choose_weather_location.borrow().as_ref() {
            f();
        }
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.disposed.replace(true) {
            return;
        }
        self.set_active(false);
        if let Some(id) = self.status_sub.take() {
            self.status.changed.unsubscribe(id);
        }
        self.quick_launch.dispose();
        self.stats.dispose();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn until_text() {
        Strings::set_ui_culture("en");
        assert_eq!(format_until(0.0), Strings::get("HomeEventNow"));
        assert_eq!(format_until(61.0), Strings::format("HomeEventInMinutes", &["2"]));
        assert_eq!(format_until(3600.0 * 10.0 + 60.0 * 13.0), Strings::format("HomeEventInHours", &["10", "13"]));
    }
}

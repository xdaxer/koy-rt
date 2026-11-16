//! Odak sayfası ve çalışan zamanlayıcının özetleri (kapalı çentikteki kalan süre, Ana'daki "Odak · 18 dk kaldı").
//! Süre ve durum `FocusTimers` içindedir (görünümden bağımsız); bu tür yalnız ekran metnini üretir. Boşta CPU:
//! tazeleme zamanlayıcısı yalnız bir zamanlayıcı çalışırken ve çentik görünürken, saniye başına bir kez kurulur.
//! Karşılığı: `Features/Focus/FocusViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, Timelike};
use koy_core::event::Subscription;
use koy_core::settings::AppSettings;
use koy_core::time::TimeSpan;
use koy_core::timers::{CountdownPresets, CountdownState, DurationFormat, FocusKind, FocusReading, FocusTimers, PomodoroMode};

use super::tasks_view_model::TasksViewModel;
use crate::infra::dispatch;
use crate::infra::observable::{LocalEvent, Notifier};
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;

/// Odak sayfasının üstündeki alt sekmelerden biri (Odak, Mola, Sayaç, Kronometre).
#[derive(Debug, Clone)]
pub struct FocusTab {
    pub kind: FocusKind,
    pub label: String,
    pub is_selected: bool,
}

/// Sayacın hazır süre düğmesi ("5 dk").
#[derive(Debug, Clone)]
pub struct FocusPreset {
    pub minutes: i32,
    pub label: String,
}

/// Kronometre tur satırı.
#[derive(Debug, Clone)]
pub struct FocusLapRow {
    pub label: String,
    pub total: String,
    pub split: String,
}

#[derive(Debug, Clone)]
pub struct FocusState {
    pub tabs: Vec<FocusTab>,
    pub selected: FocusKind,
    /// Halkanın ortasındaki büyük süre.
    pub time_text: String,
    /// Süre metninin altındaki küçük satır ("2 / 4 tur", "Geçen süre"…).
    pub sub_text: String,
    /// Halka doluluğu, 0-1.
    pub progress: f64,
    pub is_running: bool,
    /// Sayaç süresi yalnız çalışmıyorken değiştirilir.
    pub can_edit_countdown: bool,
    pub sessions_text: String,
    /// Sayacın ayarlı süresi ("05:00").
    pub countdown_duration_text: String,
    pub laps: Vec<FocusLapRow>,
    /// Herhangi bir zamanlayıcı çalışıyor.
    pub has_running: bool,
    /// Kapalı çentikte pil ve bağlantı simgelerinin yerine yazılan süre ("18:24").
    pub running_text: String,
    /// Ana'daki satır ("Odak · 18 dk kaldı").
    pub home_line: String,
}

impl FocusState {
    pub fn is_pomodoro_tab(&self) -> bool {
        matches!(self.selected, FocusKind::Focus | FocusKind::Break)
    }
    pub fn is_countdown_tab(&self) -> bool {
        self.selected == FocusKind::Countdown
    }
    pub fn is_stopwatch_tab(&self) -> bool {
        self.selected == FocusKind::Stopwatch
    }
    /// "Sonraki aşamaya geç" yalnız Pomodoro sekmelerinde anlamlı.
    pub fn can_skip(&self) -> bool {
        self.is_pomodoro_tab()
    }
    pub fn has_laps(&self) -> bool {
        !self.laps.is_empty()
    }
}

pub struct FocusViewModel {
    timers: Arc<FocusTimers>,
    pub tasks: Option<Rc<TasksViewModel>>,
    pub presets: Vec<FocusPreset>,
    pub durations_text: String,
    state: RefCell<FocusState>,
    pub changed: Notifier,
    /// Pomodoro oturumu ya da sayaç bitti: çentik kısa süre Peek'e geçer ve titrer.
    pub timer_finished: LocalEvent<()>,
    display_timer: RefCell<Option<slint::Timer>>,
    panel_visible: Cell<bool>,
    summary_visible: Cell<bool>,
    disposed: Cell<bool>,
    subs: RefCell<Vec<Subscription>>,
}

impl FocusViewModel {
    pub fn new(timers: Arc<FocusTimers>, settings: &AppSettings, tasks: Option<Rc<TasksViewModel>>) -> Rc<Self> {
        let durations_text = Strings::format(
            "FocusDurations",
            &[
                &settings.focus.pomodoro_work_minutes.to_string(),
                &settings.focus.pomodoro_short_break_minutes.to_string(),
                &settings.focus.pomodoro_long_break_minutes.to_string(),
            ],
        );
        let tabs = vec![
            FocusTab { kind: FocusKind::Focus, label: Strings::get("FocusTabFocus"), is_selected: true },
            FocusTab { kind: FocusKind::Break, label: Strings::get("FocusTabBreak"), is_selected: false },
            FocusTab { kind: FocusKind::Countdown, label: Strings::get("FocusTabTimer"), is_selected: false },
            FocusTab { kind: FocusKind::Stopwatch, label: Strings::get("FocusTabStopwatch"), is_selected: false },
        ];
        let presets = CountdownPresets::MINUTES
            .iter()
            .map(|m| FocusPreset { minutes: *m, label: Strings::format("TimerPresetMinutes", &[&m.to_string()]) })
            .collect();
        let vm = Rc::new(FocusViewModel {
            timers,
            tasks,
            presets,
            durations_text,
            state: RefCell::new(FocusState {
                tabs,
                selected: FocusKind::Focus,
                time_text: String::new(),
                sub_text: String::new(),
                progress: 0.0,
                is_running: false,
                can_edit_countdown: true,
                sessions_text: "0".into(),
                countdown_duration_text: String::new(),
                laps: Vec::new(),
                has_running: false,
                running_text: String::new(),
                home_line: String::new(),
            }),
            changed: Notifier::default(),
            timer_finished: LocalEvent::default(),
            display_timer: RefCell::new(None),
            panel_visible: Cell::new(false),
            summary_visible: Cell::new(false),
            disposed: Cell::new(false),
            subs: RefCell::new(Vec::new()),
        });
        let me = UiRef::new(&vm);
        let s1 = vm.timers.changed().subscribe(move |_| {
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && !vm.disposed.get()
                {
                    vm.refresh(true);
                    vm.restart_display_timer();
                    vm.changed.notify();
                }
            });
        });
        let s2 = vm.timers.finished().subscribe(move |_| {
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && !vm.disposed.get()
                {
                    vm.timer_finished.invoke(&());
                }
            });
        });
        vm.subs.borrow_mut().extend([s1, s2]);
        vm.refresh(true);
        vm
    }

    pub fn state(&self) -> std::cell::Ref<'_, FocusState> {
        self.state.borrow()
    }

    /// `panel_visible`: Odak sayfası açık çentikte görünüyor. `summary_visible`: çentik görünür.
    pub fn set_viewing(self: &Rc<Self>, panel_visible: bool, summary_visible: bool) {
        self.panel_visible.set(panel_visible);
        self.summary_visible.set(summary_visible);
        if panel_visible || summary_visible {
            self.refresh(true);
            self.changed.notify();
        }
        self.restart_display_timer();
    }

    // Çalışan bir şey yoksa ya da hiçbir şey görünmüyorsa tik yok; çalışırken bir sonraki saniye başına tek atımlık.
    fn restart_display_timer(self: &Rc<Self>) {
        *self.display_timer.borrow_mut() = None;
        if self.disposed.get() || !(self.panel_visible.get() || self.summary_visible.get()) || self.timers.running().is_none() {
            return;
        }
        let ms = 1000 - (Local::now().nanosecond() / 1_000_000).min(999) as u64 + 20;
        let weak = Rc::downgrade(self);
        let timer = dispatch::delay(Duration::from_millis(ms), move || {
            if let Some(vm) = weak.upgrade() {
                vm.refresh(false);
                vm.changed.notify();
                vm.restart_display_timer();
            }
        });
        *self.display_timer.borrow_mut() = Some(timer);
    }

    /// `structural`: durum değişti (başlat, duraklat, kip, tur); yalnız saat ilerlediyse false.
    fn refresh(&self, structural: bool) {
        let t = &self.timers;
        let mut st = self.state.borrow_mut();
        // Oturum bitip mola başlayınca ya da Pomodoro dışarıdan kip değiştirince Odak / Mola sekmesi onu izler.
        if matches!(st.selected, FocusKind::Focus | FocusKind::Break) {
            let follow = if t.pomodoro.mode() == PomodoroMode::Work { FocusKind::Focus } else { FocusKind::Break };
            st.selected = follow;
        }
        let selected = st.selected;
        for tab in st.tabs.iter_mut() {
            tab.is_selected = tab.kind == selected;
        }
        let reading = t.read(selected);
        st.time_text = format_reading(&reading);
        st.is_running = t.is_running(selected);
        st.can_edit_countdown = t.countdown.state() != CountdownState::Running;
        st.sub_text = self.build_sub_text(selected);
        st.progress = self.build_progress(selected, &reading);
        st.sessions_text = t.pomodoro.completed_work_sessions().to_string();
        st.countdown_duration_text = DurationFormat::countdown(t.countdown.duration());
        let running = t.running();
        st.has_running = running.is_some();
        st.running_text = running.as_ref().map(format_reading).unwrap_or_default();
        st.home_line = running.as_ref().map(build_home_line).unwrap_or_default();
        if structural {
            st.laps = t
                .stopwatch
                .laps()
                .into_iter()
                .take(20)
                .map(|lap| FocusLapRow {
                    label: Strings::format("TimerLapRow", &[&lap.number.to_string()]),
                    total: DurationFormat::stopwatch(lap.total),
                    split: format!("+{}", DurationFormat::stopwatch(lap.split)),
                })
                .collect();
        }
    }

    fn build_sub_text(&self, selected: FocusKind) -> String {
        let p = &self.timers.pomodoro;
        match selected {
            FocusKind::Focus => Strings::format("FocusRound", &[&p.current_round().to_string(), &p.config().sessions_before_long_break.to_string()]),
            FocusKind::Break => Strings::get(if p.mode() == PomodoroMode::LongBreak { "LongBreak" } else { "ShortBreak" }),
            FocusKind::Countdown => {
                Strings::get(if self.timers.countdown.state() == CountdownState::Finished { "FocusTimeUp" } else { "FocusCountdown" })
            }
            _ => Strings::get("FocusElapsed"),
        }
    }

    fn build_progress(&self, selected: FocusKind, reading: &FocusReading) -> f64 {
        match selected {
            FocusKind::Focus | FocusKind::Break => {
                let total = self.timers.pomodoro.get_duration_for_mode(self.timers.pomodoro.mode()).total_seconds();
                if total > 0.0 { (1.0 - reading.value.total_seconds() / total).clamp(0.0, 1.0) } else { 0.0 }
            }
            FocusKind::Countdown => {
                let total = self.timers.countdown.duration().total_seconds();
                if self.timers.countdown.state() == CountdownState::Finished {
                    1.0
                } else if total > 0.0 {
                    (1.0 - reading.value.total_seconds() / total).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
            // Kronometre: halka dakikayı süpürür.
            _ => reading.value.total_seconds() % 60.0 / 60.0,
        }
    }

    // ---- Komutlar ----

    pub fn select_tab(self: &Rc<Self>, kind: FocusKind) {
        self.timers.select_pomodoro_kind(kind);
        self.state.borrow_mut().selected = kind;
        self.refresh(true);
        self.restart_display_timer();
        self.changed.notify();
    }

    pub fn toggle(&self) {
        let selected = self.state.borrow().selected;
        self.timers.toggle(selected);
    }

    pub fn reset(&self) {
        let selected = self.state.borrow().selected;
        self.timers.reset(selected);
    }

    /// Ana'daki Odak satırı: çalışan zamanlayıcı varsa onu duraklatır, yoksa seçili sekmeninkini başlatır / sürdürür.
    pub fn toggle_running(&self) {
        let kind = self.timers.running().map_or(self.state.borrow().selected, |r| r.kind);
        self.timers.toggle(kind);
    }

    pub fn skip(&self) {
        self.timers.pomodoro.skip();
    }

    pub fn lap(&self) {
        self.timers.stopwatch.lap();
    }

    pub fn increase_countdown(&self) {
        self.timers.countdown.set_duration(CountdownPresets::step(self.timers.countdown.duration(), true));
    }

    pub fn decrease_countdown(&self) {
        self.timers.countdown.set_duration(CountdownPresets::step(self.timers.countdown.duration(), false));
    }

    pub fn set_countdown_minutes(&self, minutes: i32) {
        self.timers.countdown.set_duration(TimeSpan::from_mins(minutes as i64));
    }

    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        *self.display_timer.borrow_mut() = None;
        let subs: Vec<Subscription> = self.subs.borrow_mut().drain(..).collect();
        if let [a, b] = subs[..] {
            self.timers.changed().unsubscribe(a);
            self.timers.finished().unsubscribe(b);
        }
        if let Some(t) = &self.tasks {
            t.dispose();
        }
    }
}

fn format_reading(reading: &FocusReading) -> String {
    if reading.kind == FocusKind::Stopwatch {
        // Kronometre metninin son iki karakteri (salise) atılır.
        let s = DurationFormat::stopwatch(reading.value);
        let n = s.chars().count();
        s.chars().take(n.saturating_sub(2)).collect()
    } else {
        DurationFormat::countdown(reading.value)
    }
}

fn build_home_line(running: &FocusReading) -> String {
    let label = Strings::get(match running.kind {
        FocusKind::Focus => "FocusTabFocus",
        FocusKind::Break => "FocusTabBreak",
        FocusKind::Countdown => "FocusTabTimer",
        _ => "FocusTabStopwatch",
    });
    let minutes = running.whole_minutes();
    let amount = if minutes > 0 {
        Strings::format("HomeMinutes", &[&minutes.to_string()])
    } else if running.counts_down {
        Strings::get("HomeUnderMinute")
    } else {
        Strings::format("HomeMinutes", &["0"])
    };
    Strings::format(if running.counts_down { "HomeLeft" } else { "HomeElapsed" }, &[&label, &amount])
}

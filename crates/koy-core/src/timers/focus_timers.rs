use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::countdown_presets::CountdownPresets;
use super::countdown_timer::{CountdownState, CountdownTimer};
use super::lap_stopwatch::{LapStopwatch, StopwatchState};
use super::pomodoro::{Pomodoro, PomodoroConfig, PomodoroMode, PomodoroState};
use crate::event::Event;
use crate::time::{TimeProvider, TimeSpan};

/// Odak sekmesindeki dört alt sekme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusKind {
    Focus,
    Break,
    Countdown,
    Stopwatch,
}

/// Bir zamanlayıcının o anki okuması. `counts_down` yanlışsa değer geçen süredir (kronometre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusReading {
    pub kind: FocusKind,
    pub value: TimeSpan,
    pub counts_down: bool,
}

impl FocusReading {
    pub fn new(kind: FocusKind, value: TimeSpan, counts_down: bool) -> Self {
        FocusReading { kind, value, counts_down }
    }

    /// Tam dakika (alta yuvarlanır): 18:24 için 18. Ana'daki "18 dk kaldı" satırı için.
    pub fn whole_minutes(&self) -> i32 {
        if self.value <= TimeSpan::ZERO { 0 } else { self.value.total_minutes() as i32 }
    }
}

/// Pomodoro, serbest zamanlayıcı ve kronometrenin tek sahibi. Görünümden bağımsızdır: çentik sekme değiştirse,
/// gizlense ya da birden çok monitörde pencere olsa da süre aynı yerden akar (NotchFeature tek örnek kurar ve
/// bütün pencerelere verir). Aynı anda en çok bir zamanlayıcı çalışır: birini başlatmak çalışanı duraklatır.
/// Kapalı çentikteki kalan süre ve Ana'daki "Odak · 18 dk kaldı" satırı [`FocusTimers::running`] okumasından beslenir.
/// Olaylar zamanlayıcının iş parçacığından gelir; arayüz kendi iş parçacığına aktarmalıdır.
pub struct FocusTimers {
    disposed: AtomicBool,

    pub pomodoro: Pomodoro,
    pub countdown: CountdownTimer,
    pub stopwatch: LapStopwatch,

    changed: Arc<Event<()>>,
    finished: Arc<Event<FocusKind>>,
}

impl FocusTimers {
    pub fn new(
        config: Option<PomodoroConfig>,
        countdown_duration: Option<TimeSpan>,
        time_provider: Option<Arc<dyn TimeProvider>>,
    ) -> Self {
        let pomodoro = Pomodoro::new(config, time_provider.clone());
        let countdown = CountdownTimer::new(
            countdown_duration.unwrap_or_else(|| TimeSpan::from_mins(i64::from(CountdownPresets::DEFAULT_MINUTES))),
            time_provider.clone(),
        );
        let stopwatch = LapStopwatch::new(time_provider);

        let changed: Arc<Event<()>> = Arc::new(Event::new());
        let finished: Arc<Event<FocusKind>> = Arc::new(Event::new());

        let raise = |changed: &Arc<Event<()>>| {
            let changed = Arc::clone(changed);
            move || changed.invoke(&())
        };

        let r = raise(&changed);
        pomodoro.state_changed().subscribe(move |_| r());
        let r = raise(&changed);
        pomodoro.mode_changed().subscribe(move |_| r());
        let f = Arc::clone(&finished);
        pomodoro.session_completed().subscribe(move |mode| {
            // OnPomodoroSessionCompleted
            f.invoke(&if *mode == PomodoroMode::Work { FocusKind::Focus } else { FocusKind::Break });
        });
        let r = raise(&changed);
        countdown.state_changed().subscribe(move |_| r());
        let f = Arc::clone(&finished);
        countdown.finished().subscribe(move |_| f.invoke(&FocusKind::Countdown));
        let r = raise(&changed);
        stopwatch.state_changed().subscribe(move |_| r());
        let r = raise(&changed);
        stopwatch.laps_changed().subscribe(move |_| r());

        FocusTimers { disposed: AtomicBool::new(false), pomodoro, countdown, stopwatch, changed, finished }
    }

    /// Bir zamanlayıcının durumu değişti (başlat, duraklat, sıfırla, kip, süre, tur). Saniyelik tik DEĞİLDİR:
    /// süre metnini arayüz kendi tazeleme zamanlayıcısıyla, yalnız görünürken okur.
    pub fn changed(&self) -> &Event<()> {
        &self.changed
    }

    /// Bir Pomodoro oturumu ya da serbest zamanlayıcı bitti.
    pub fn finished(&self) -> &Event<FocusKind> {
        &self.finished
    }

    pub fn is_running(&self, kind: FocusKind) -> bool {
        match kind {
            FocusKind::Focus | FocusKind::Break => {
                self.pomodoro.state() == PomodoroState::Running && (self.pomodoro.mode() == PomodoroMode::Work) == (kind == FocusKind::Focus)
            }
            FocusKind::Countdown => self.countdown.state() == CountdownState::Running,
            _ => self.stopwatch.state() == StopwatchState::Running,
        }
    }

    /// Çalışan zamanlayıcının okuması; hiçbiri çalışmıyorsa None.
    pub fn running(&self) -> Option<FocusReading> {
        if self.pomodoro.state() == PomodoroState::Running {
            return Some(self.read(if self.pomodoro.mode() == PomodoroMode::Work { FocusKind::Focus } else { FocusKind::Break }));
        }
        if self.countdown.state() == CountdownState::Running {
            return Some(self.read(FocusKind::Countdown));
        }
        if self.stopwatch.state() == StopwatchState::Running {
            return Some(self.read(FocusKind::Stopwatch));
        }
        None
    }

    pub fn read(&self, kind: FocusKind) -> FocusReading {
        match kind {
            FocusKind::Focus | FocusKind::Break => FocusReading::new(kind, self.pomodoro.remaining(), true),
            FocusKind::Countdown => FocusReading::new(kind, self.countdown.remaining(), true),
            _ => FocusReading::new(kind, self.stopwatch.elapsed(), false),
        }
    }

    /// Odak / Mola alt sekmesi: Pomodoro'yu çalışma ya da mola kipine geçirir (çalışıyorsa duraklatır).
    /// Uzun mola sırasında "Mola" seçmek bir şey yapmaz. Sayaç ve kronometre için işlem yoktur.
    pub fn select_pomodoro_kind(&self, kind: FocusKind) {
        if kind == FocusKind::Focus {
            self.pomodoro.set_mode(PomodoroMode::Work);
        } else if kind == FocusKind::Break && self.pomodoro.mode() == PomodoroMode::Work {
            self.pomodoro.set_mode(PomodoroMode::ShortBreak);
        }
    }

    /// Başlatır ya da duraklatır; başlatırken çalışan diğer zamanlayıcıyı duraklatır.
    pub fn toggle(&self, kind: FocusKind) {
        if self.is_running(kind) {
            self.pause(kind);
            return;
        }

        if !matches!(kind, FocusKind::Focus | FocusKind::Break) {
            self.pomodoro.pause();
        }
        if kind != FocusKind::Countdown {
            self.countdown.pause();
        }
        if kind != FocusKind::Stopwatch {
            self.stopwatch.pause();
        }

        match kind {
            FocusKind::Focus | FocusKind::Break => self.pomodoro.start(),
            FocusKind::Countdown => self.countdown.start(),
            _ => self.stopwatch.start(),
        }
    }

    pub fn reset(&self, kind: FocusKind) {
        match kind {
            FocusKind::Focus | FocusKind::Break => self.pomodoro.reset(),
            FocusKind::Countdown => self.countdown.reset(),
            _ => self.stopwatch.reset(),
        }
    }

    fn pause(&self, kind: FocusKind) {
        match kind {
            FocusKind::Focus | FocusKind::Break => self.pomodoro.pause(),
            FocusKind::Countdown => self.countdown.pause(),
            _ => self.stopwatch.pause(),
        }
    }

    pub fn dispose(&self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }

        self.pomodoro.dispose();
        self.countdown.dispose();
    }
}

impl Drop for FocusTimers {
    fn drop(&mut self) {
        self.dispose();
    }
}

use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::event::Event;
use crate::text;
use crate::time::{SystemTimeProvider, TimeProvider, TimeSpan, Timer, TimerCallback};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PomodoroMode {
    #[default]
    Work,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PomodoroState {
    #[default]
    Stopped,
    Running,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PomodoroConfig {
    pub work_duration: TimeSpan,
    pub short_break_duration: TimeSpan,
    pub long_break_duration: TimeSpan,
    pub sessions_before_long_break: i32,
}

impl PomodoroConfig {
    pub const MIN_MINUTES: i32 = 1;
    pub const MAX_MINUTES: i32 = 180;

    /// C#'taki `new PomodoroConfig(work, short, long)`: `SessionsBeforeLongBreak` varsayılanı 4.
    pub fn new(work_duration: TimeSpan, short_break_duration: TimeSpan, long_break_duration: TimeSpan) -> Self {
        Self::with_sessions(work_duration, short_break_duration, long_break_duration, 4)
    }

    /// C#'taki dört bağımsız değişkenli `new PomodoroConfig(...)`.
    pub fn with_sessions(
        work_duration: TimeSpan,
        short_break_duration: TimeSpan,
        long_break_duration: TimeSpan,
        sessions_before_long_break: i32,
    ) -> Self {
        PomodoroConfig { work_duration, short_break_duration, long_break_duration, sessions_before_long_break }
    }

    /// Ayarlardaki dakika değerlerinden yapılandırma; saçma değerler 1-180 dakikaya sıkıştırılır.
    pub fn from_minutes(work: i32, short_break: i32, long_break: i32) -> Self {
        Self::with_sessions(
            TimeSpan::from_mins(i64::from(text::clamp(work, Self::MIN_MINUTES, Self::MAX_MINUTES))),
            TimeSpan::from_mins(i64::from(text::clamp(short_break, Self::MIN_MINUTES, Self::MAX_MINUTES))),
            TimeSpan::from_mins(i64::from(text::clamp(long_break, Self::MIN_MINUTES, Self::MAX_MINUTES))),
            4,
        )
    }
}

impl Default for PomodoroConfig {
    /// `PomodoroConfig.Default`: klasik 25 / 5 / 15, 4 turda bir uzun mola.
    fn default() -> Self {
        Self::with_sessions(TimeSpan::from_mins(25), TimeSpan::from_mins(5), TimeSpan::from_mins(15), 4)
    }
}

struct Inner {
    timer: Option<Box<dyn Timer>>,
    disposed: bool,
    config: PomodoroConfig,
    mode: PomodoroMode,
    state: PomodoroState,
    remaining: TimeSpan,
    completed_work_sessions: i32,
}

struct Shared {
    time_provider: Arc<dyn TimeProvider>,
    inner: Mutex<Inner>,
    ticked: Event<TimeSpan>,
    state_changed: Event<PomodoroState>,
    mode_changed: Event<PomodoroMode>,
    session_completed: Event<PomodoroMode>,
}

/// Pomodoro durum makinesi ve zamanlayıcısı.
/// Saf mantık: `TimeProvider` enjekte edilir, tüm durum geçişleri deterministik olarak test edilebilir.
///
/// C#'ta kilit yoktu (tek iş parçacığından kullanılır, zamanlayıcı geri çağrısı hariç); burada zamanlayıcı geri
/// çağrısı `Send` olmak zorunda olduğu için durum bir `Mutex` arkasındadır. Olaylar kilit bırakıldıktan sonra tetiklenir
/// (işleyici Pomodoro'ya geri dönüp durumu okuyabilsin), sıra C#'taki gibidir.
pub struct Pomodoro {
    shared: Arc<Shared>,
}

impl Pomodoro {
    pub fn new(config: Option<PomodoroConfig>, time_provider: Option<Arc<dyn TimeProvider>>) -> Self {
        let config = config.unwrap_or_default();
        Pomodoro {
            shared: Arc::new(Shared {
                time_provider: time_provider.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
                inner: Mutex::new(Inner {
                    timer: None,
                    disposed: false,
                    config,
                    mode: PomodoroMode::Work,
                    state: PomodoroState::Stopped,
                    remaining: config.work_duration,
                    completed_work_sessions: 0,
                }),
                ticked: Event::new(),
                state_changed: Event::new(),
                mode_changed: Event::new(),
                session_completed: Event::new(),
            }),
        }
    }

    pub fn config(&self) -> PomodoroConfig {
        self.shared.lock().config
    }

    pub fn mode(&self) -> PomodoroMode {
        self.shared.lock().mode
    }

    pub fn state(&self) -> PomodoroState {
        self.shared.lock().state
    }

    pub fn remaining(&self) -> TimeSpan {
        self.shared.lock().remaining
    }

    pub fn completed_work_sessions(&self) -> i32 {
        self.shared.lock().completed_work_sessions
    }

    pub fn ticked(&self) -> &Event<TimeSpan> {
        &self.shared.ticked
    }

    pub fn state_changed(&self) -> &Event<PomodoroState> {
        &self.shared.state_changed
    }

    pub fn mode_changed(&self) -> &Event<PomodoroMode> {
        &self.shared.mode_changed
    }

    pub fn session_completed(&self) -> &Event<PomodoroMode> {
        &self.shared.session_completed
    }

    pub fn start(&self) {
        let state = {
            let mut g = self.shared.lock();
            if g.state == PomodoroState::Running {
                return;
            }

            g.state = PomodoroState::Running;
            g.state
        };
        self.shared.state_changed.invoke(&state);

        // Duraklat / Sıfırla zamanlayıcıyı sonsuza çeker; yeniden başlatırken tekrar kurulmazsa süre bir daha akmaz.
        let mut g = self.shared.lock();
        match g.timer.as_mut() {
            None => {
                let weak = Arc::downgrade(&self.shared);
                let callback: TimerCallback = Box::new(move || {
                    if let Some(shared) = Weak::upgrade(&weak) {
                        shared.on_timer_tick();
                    }
                });
                g.timer = Some(self.shared.time_provider.create_timer(callback, TimeSpan::from_secs(1), TimeSpan::from_secs(1)));
            }
            Some(timer) => {
                timer.change(TimeSpan::from_secs(1), TimeSpan::from_secs(1));
            }
        }
    }

    /// Çalışma / mola arasında elle geçiş (Odak sekmesindeki "Odak" ve "Mola"). Çalışıyorsa önce duraklatır;
    /// süre yeni modun tam süresine döner. Aynı moda geçiş bir şey yapmaz.
    pub fn set_mode(&self, mode: PomodoroMode) {
        if self.shared.lock().mode == mode {
            return;
        }

        self.pause();
        let remaining = {
            let mut g = self.shared.lock();
            g.mode = mode;
            g.remaining = g.config_duration_for(mode);
            g.remaining
        };
        self.shared.mode_changed.invoke(&mode);
        self.shared.ticked.invoke(&remaining);
    }

    /// Döngüdeki tur (1 tabanlı, `PomodoroConfig::sessions_before_long_break` üzerinden): çalışırken
    /// şu anki tur, molada az önce biten tur. Tasarımdaki "2 / 4 tur".
    pub fn current_round(&self) -> i32 {
        let g = self.shared.lock();
        let n = g.config.sessions_before_long_break.max(1);
        if g.mode == PomodoroMode::Work {
            g.completed_work_sessions % n + 1
        } else {
            ((g.completed_work_sessions - 1) % n + 1).max(1)
        }
    }

    pub fn pause(&self) {
        let state = {
            let mut g = self.shared.lock();
            if g.state != PomodoroState::Running {
                return;
            }

            g.state = PomodoroState::Paused;
            if let Some(timer) = g.timer.as_mut() {
                timer.change(TimeSpan::INFINITE, TimeSpan::INFINITE);
            }
            g.state
        };
        self.shared.state_changed.invoke(&state);
    }

    pub fn reset(&self) {
        let (state, remaining) = {
            let mut g = self.shared.lock();
            g.state = PomodoroState::Stopped;
            if let Some(timer) = g.timer.as_mut() {
                timer.change(TimeSpan::INFINITE, TimeSpan::INFINITE);
            }
            g.remaining = g.config_duration_for(g.mode);
            (g.state, g.remaining)
        };
        self.shared.state_changed.invoke(&state);
        self.shared.ticked.invoke(&remaining);
    }

    pub fn skip(&self) {
        self.shared.advance_to_next_mode();
    }

    /// Saniyelik zamanlayıcı adımı; zamanlayıcı geri çağrısı bunu çağırır (testler doğrudan da çağırır).
    pub fn on_timer_tick(&self) {
        self.shared.on_timer_tick();
    }

    pub fn get_duration_for_mode(&self, mode: PomodoroMode) -> TimeSpan {
        self.shared.lock().config_duration_for(mode)
    }

    pub fn dispose(&self) {
        let mut g = self.shared.lock();
        if g.disposed {
            return;
        }

        g.disposed = true;
        if let Some(mut timer) = g.timer.take() {
            timer.dispose();
        }
    }
}

impl Drop for Pomodoro {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl Inner {
    fn config_duration_for(&self, mode: PomodoroMode) -> TimeSpan {
        match mode {
            PomodoroMode::Work => self.config.work_duration,
            PomodoroMode::ShortBreak => self.config.short_break_duration,
            PomodoroMode::LongBreak => self.config.long_break_duration,
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    fn on_timer_tick(&self) {
        let (remaining, completed) = {
            let mut g = self.lock();
            if g.state != PomodoroState::Running {
                return;
            }

            if g.remaining > TimeSpan::from_secs(1) {
                g.remaining -= TimeSpan::from_secs(1);
                (g.remaining, false)
            } else {
                g.remaining = TimeSpan::ZERO;
                (g.remaining, true)
            }
        };

        self.ticked.invoke(&remaining);
        if completed {
            self.complete_current_session();
        }
    }

    fn complete_current_session(&self) {
        let finished_mode = {
            let mut g = self.lock();
            let finished_mode = g.mode;
            if finished_mode == PomodoroMode::Work {
                g.completed_work_sessions += 1;
            }
            finished_mode
        };

        self.session_completed.invoke(&finished_mode);
        self.advance_to_next_mode();
    }

    fn advance_to_next_mode(&self) {
        let mode = {
            let mut g = self.lock();
            if g.mode == PomodoroMode::Work {
                if g.completed_work_sessions > 0 && g.completed_work_sessions % g.config.sessions_before_long_break == 0 {
                    g.mode = PomodoroMode::LongBreak;
                } else {
                    g.mode = PomodoroMode::ShortBreak;
                }
            } else {
                g.mode = PomodoroMode::Work;
            }

            g.remaining = g.config_duration_for(g.mode);
            g.mode
        };

        self.mode_changed.invoke(&mode);
        let remaining = self.lock().remaining;
        self.ticked.invoke(&remaining);
    }
}

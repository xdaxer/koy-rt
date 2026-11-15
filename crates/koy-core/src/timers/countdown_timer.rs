use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::event::Event;
use crate::time::{SystemTimeProvider, TimeProvider, TimeSpan, Timer, TimerCallback};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CountdownState {
    /// Hiç başlatılmadı ya da sıfırlandı; kalan süre = seçilen süre.
    #[default]
    Idle,
    Running,
    Paused,
    /// Süre doldu; sıfırlanana ya da yeniden başlatılana kadar bu durumda kalır.
    Finished,
}

struct Inner {
    timer: Option<Box<dyn Timer>>,
    end_timestamp: i64,
    remaining_when_stopped: TimeSpan,
    disposed: bool,
    duration: TimeSpan,
    state: CountdownState,
}

struct Shared {
    time_provider: Arc<dyn TimeProvider>,
    gate: Mutex<Inner>,
    finished: Event<()>,
    state_changed: Event<CountdownState>,
}

/// Serbest geri sayım zamanlayıcısı durum makinesi. Saniyelik tik yoktur: çalışırken bitiş anı bir kez hesaplanır
/// ve tek atımlık bir zamanlayıcı yalnız bitişte tetiklenir; kalan süre sorulduğunda saatten hesaplanır
/// ([`CountdownTimer::remaining`]). Böylece arka planda ve arayüz görünmezken hiçbir şey çalışmaz. Zaman
/// `TimeProvider` ile verilir; testler sahte saatle deterministik çalışır.
pub struct CountdownTimer {
    shared: Arc<Shared>,
}

impl CountdownTimer {
    pub const MIN_DURATION: TimeSpan = TimeSpan::from_secs(1);
    pub const MAX_DURATION: TimeSpan = TimeSpan::from_hours(99);

    pub fn new(duration: TimeSpan, time_provider: Option<Arc<dyn TimeProvider>>) -> Self {
        let duration = clamp(duration);
        CountdownTimer {
            shared: Arc::new(Shared {
                time_provider: time_provider.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
                gate: Mutex::new(Inner {
                    timer: None,
                    end_timestamp: 0,
                    remaining_when_stopped: duration,
                    disposed: false,
                    duration,
                    state: CountdownState::Idle,
                }),
                finished: Event::new(),
                state_changed: Event::new(),
            }),
        }
    }

    pub fn duration(&self) -> TimeSpan {
        self.shared.lock().duration
    }

    pub fn state(&self) -> CountdownState {
        self.shared.lock().state
    }

    /// Süre doldu. Zamanlayıcı iş parçacığından gelir; arayüz kendi iş parçacığına geçirmelidir.
    pub fn finished(&self) -> &Event<()> {
        &self.shared.finished
    }

    /// Durum değişti (başlat, duraklat, sıfırla, bitti, süre değişti).
    pub fn state_changed(&self) -> &Event<CountdownState> {
        &self.shared.state_changed
    }

    /// Kalan süre; çalışırken her sorguda saatten hesaplanır.
    pub fn remaining(&self) -> TimeSpan {
        let g = self.shared.lock();
        match g.state {
            CountdownState::Running => self.shared.remaining_from_clock(&g),
            CountdownState::Finished => TimeSpan::ZERO,
            _ => g.remaining_when_stopped,
        }
    }

    /// Süreyi değiştirir; yalnız çalışmıyorken geçerlidir (çalışan zamanlayıcının süresi oynatılmaz).
    /// Yeni süre kalan süre olur ve zamanlayıcı başlangıç durumuna döner. Geçerli değer 1 sn - 99 saat.
    pub fn set_duration(&self, duration: TimeSpan) -> bool {
        {
            let mut g = self.shared.lock();
            if g.state == CountdownState::Running {
                return false;
            }

            g.duration = clamp(duration);
            g.remaining_when_stopped = g.duration;
            g.state = CountdownState::Idle;
        }

        self.shared.state_changed.invoke(&CountdownState::Idle);
        true
    }

    pub fn start(&self) {
        {
            let mut g = self.shared.lock();
            if g.disposed || g.state == CountdownState::Running {
                return;
            }

            if g.state == CountdownState::Finished || g.remaining_when_stopped <= TimeSpan::ZERO {
                g.remaining_when_stopped = g.duration;
            }

            g.end_timestamp = self.shared.time_provider.timestamp() + self.shared.to_ticks(g.remaining_when_stopped);
            g.state = CountdownState::Running;

            if let Some(mut old) = g.timer.take() {
                old.dispose();
            }
            let due = g.remaining_when_stopped;
            g.timer = Some(self.shared.create_due_timer(Arc::downgrade(&self.shared), due));
        }

        self.shared.state_changed.invoke(&CountdownState::Running);
    }

    pub fn pause(&self) {
        {
            let mut g = self.shared.lock();
            if g.state != CountdownState::Running {
                return;
            }

            g.remaining_when_stopped = self.shared.remaining_from_clock(&g);
            g.state = CountdownState::Paused;
            Shared::stop_timer(&mut g);
        }

        self.shared.state_changed.invoke(&CountdownState::Paused);
    }

    pub fn reset(&self) {
        {
            let mut g = self.shared.lock();
            Shared::stop_timer(&mut g);
            g.remaining_when_stopped = g.duration;
            g.state = CountdownState::Idle;
        }

        self.shared.state_changed.invoke(&CountdownState::Idle);
    }

    pub fn dispose(&self) {
        let mut g = self.shared.lock();
        g.disposed = true;
        Shared::stop_timer(&mut g);
    }
}

impl Drop for CountdownTimer {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.gate.lock().unwrap()
    }

    fn create_due_timer(&self, weak: Weak<Shared>, due: TimeSpan) -> Box<dyn Timer> {
        let callback: TimerCallback = Box::new(move || {
            if let Some(shared) = weak.upgrade() {
                shared.on_due(&shared);
            }
        });
        self.time_provider.create_timer(callback, due, TimeSpan::INFINITE)
    }

    fn on_due(&self, this: &Arc<Shared>) {
        {
            let mut g = self.lock();
            if g.state != CountdownState::Running {
                return;
            }

            // Sistem zamanlayıcısı bitişten milisaniyeler önce tetiklenebilir: kalan süre varsa kalanı bekle.
            let left = self.remaining_from_clock(&g);
            if left > TimeSpan::ZERO {
                if let Some(mut old) = g.timer.take() {
                    old.dispose();
                }
                g.timer = Some(self.create_due_timer(Arc::downgrade(this), left + TimeSpan::from_millis(5)));
                return;
            }

            g.state = CountdownState::Finished;
            g.remaining_when_stopped = TimeSpan::ZERO;
            Self::stop_timer(&mut g);
        }

        self.state_changed.invoke(&CountdownState::Finished);
        self.finished.invoke(&());
    }

    fn remaining_from_clock(&self, g: &Inner) -> TimeSpan {
        let left = g.end_timestamp - self.time_provider.timestamp();
        if left <= 0 { TimeSpan::ZERO } else { self.time_provider.elapsed_time(0, left) }
    }

    fn to_ticks(&self, span: TimeSpan) -> i64 {
        (span.total_seconds() * self.time_provider.timestamp_frequency() as f64) as i64
    }

    fn stop_timer(g: &mut Inner) {
        if let Some(mut timer) = g.timer.take() {
            timer.dispose();
        }
    }
}

fn clamp(value: TimeSpan) -> TimeSpan {
    if value < CountdownTimer::MIN_DURATION {
        CountdownTimer::MIN_DURATION
    } else if value > CountdownTimer::MAX_DURATION {
        CountdownTimer::MAX_DURATION
    } else {
        value
    }
}

use std::sync::{Arc, Mutex, MutexGuard};

use crate::event::Event;
use crate::time::{SystemTimeProvider, TimeProvider, TimeSpan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StopwatchState {
    #[default]
    Idle,
    Running,
    Paused,
}

/// Bir tur: sıra numarası, başlangıçtan toplam süre ve bir önceki turdan beri geçen süre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopwatchLap {
    pub number: i32,
    pub total: TimeSpan,
    pub split: TimeSpan,
}

struct Inner {
    laps: Vec<StopwatchLap>,
    start_timestamp: i64,
    accumulated: TimeSpan,
    last_lap_total: TimeSpan,
    lap_counter: i32,
    state: StopwatchState,
}

/// Kronometre durum makinesi. Zamanlayıcı ya da tik yoktur: geçen süre sorulduğunda saatten hesaplanır, bu yüzden
/// arayüz görünmezken hiçbir iş yapılmaz. Başlat / duraklat / tur / sıfırla. Zaman `TimeProvider` ile verilir.
pub struct LapStopwatch {
    time_provider: Arc<dyn TimeProvider>,
    gate: Mutex<Inner>,
    state_changed: Event<StopwatchState>,
    laps_changed: Event<()>,
}

impl LapStopwatch {
    /// Saklanan tur sayısı üst sınırı (listeyi sınırsız büyütmemek için); en eski tur düşer.
    pub const MAX_LAPS: usize = 99;

    pub fn new(time_provider: Option<Arc<dyn TimeProvider>>) -> Self {
        LapStopwatch {
            time_provider: time_provider.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
            gate: Mutex::new(Inner {
                laps: Vec::new(),
                start_timestamp: 0,
                accumulated: TimeSpan::ZERO,
                last_lap_total: TimeSpan::ZERO,
                lap_counter: 0,
                state: StopwatchState::Idle,
            }),
            state_changed: Event::new(),
            laps_changed: Event::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.gate.lock().unwrap()
    }

    pub fn state(&self) -> StopwatchState {
        self.lock().state
    }

    pub fn state_changed(&self) -> &Event<StopwatchState> {
        &self.state_changed
    }

    pub fn laps_changed(&self) -> &Event<()> {
        &self.laps_changed
    }

    pub fn elapsed(&self) -> TimeSpan {
        let g = self.lock();
        self.elapsed_locked(&g)
    }

    /// Turlar, en yeni en başta.
    pub fn laps(&self) -> Vec<StopwatchLap> {
        self.lock().laps.iter().rev().copied().collect()
    }

    pub fn start(&self) {
        {
            let mut g = self.lock();
            if g.state == StopwatchState::Running {
                return;
            }

            g.start_timestamp = self.time_provider.timestamp();
            g.state = StopwatchState::Running;
        }

        self.state_changed.invoke(&StopwatchState::Running);
    }

    pub fn pause(&self) {
        {
            let mut g = self.lock();
            if g.state != StopwatchState::Running {
                return;
            }

            g.accumulated = self.elapsed_locked(&g);
            g.state = StopwatchState::Paused;
        }

        self.state_changed.invoke(&StopwatchState::Paused);
    }

    /// Tur ekler; yalnız çalışırken geçerlidir. Eklenen turu döner (çalışmıyorsa None).
    pub fn lap(&self) -> Option<StopwatchLap> {
        let lap = {
            let mut g = self.lock();
            if g.state != StopwatchState::Running {
                return None;
            }

            let total = self.elapsed_locked(&g);
            g.lap_counter += 1;
            let lap = StopwatchLap { number: g.lap_counter, total, split: total - g.last_lap_total };
            g.last_lap_total = total;
            g.laps.push(lap);
            if g.laps.len() > Self::MAX_LAPS {
                g.laps.remove(0);
            }
            lap
        };

        self.laps_changed.invoke(&());
        Some(lap)
    }

    pub fn reset(&self) {
        {
            let mut g = self.lock();
            g.state = StopwatchState::Idle;
            g.accumulated = TimeSpan::ZERO;
            g.last_lap_total = TimeSpan::ZERO;
            g.lap_counter = 0;
            g.laps.clear();
        }

        self.state_changed.invoke(&StopwatchState::Idle);
        self.laps_changed.invoke(&());
    }

    fn elapsed_locked(&self, g: &Inner) -> TimeSpan {
        if g.state == StopwatchState::Running {
            g.accumulated + self.time_provider.elapsed_since(g.start_timestamp)
        } else {
            g.accumulated
        }
    }
}

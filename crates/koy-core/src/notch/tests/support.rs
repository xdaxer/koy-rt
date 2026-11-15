//! Testlerin ortak yardımcıları.

use std::sync::{Arc, Mutex};

use crate::settings::{AppSettings, SettingsStore};
use crate::time::{DateTimeOffset, TimeProvider, TimeSpan, Timer, TimerCallback};

/// `&["a", "b"]` → `Vec<String>` (C# string dizisi değişmezleri için).
pub fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|x| x.to_string()).collect()
}

/// `tests/Core.Tests/Settings/SettingsFile.cs`: ham JSON'u gerçek `SettingsStore` yolundan (göç dahil) okur.
pub fn settings_file_load(json: &str) -> AppSettings {
    let dir = std::env::temp_dir().join(format!("koy-sf-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(&path, json).unwrap();
    let loaded = SettingsStore::new(path).load();
    let _ = std::fs::remove_dir_all(&dir);
    loaded
}

struct ManualTimerState {
    active: bool,
    callback: Option<TimerCallback>,
}

struct ManualTimer {
    state: Arc<Mutex<ManualTimerState>>,
}

impl Timer for ManualTimer {
    fn change(&mut self, _due_time: TimeSpan, _period: TimeSpan) -> bool {
        true
    }

    fn dispose(&mut self) {
        self.state.lock().unwrap().active = false;
    }
}

impl Drop for ManualTimer {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// `RefreshDebouncerTests.ManualTimeProvider`: zamanlayıcıları kaydeder, yalnız `fire_last` ile tetiklenir.
#[derive(Default)]
pub struct ManualTimeProvider {
    timers: Mutex<Vec<Arc<Mutex<ManualTimerState>>>>,
}

impl ManualTimeProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Timers.Count` (`Created`).
    pub fn created(&self) -> usize {
        self.timers.lock().unwrap().len()
    }

    /// `Timers[^1].Fire()`: zamanlayıcı hâlâ etkinse bir kez çalıştırır.
    pub fn fire_last(&self) {
        let last = Arc::clone(self.timers.lock().unwrap().last().expect("zamanlayıcı yok"));
        let cb = {
            let mut st = last.lock().unwrap();
            if st.active {
                st.active = false;
                st.callback.take()
            } else {
                None
            }
        };
        if let Some(mut cb) = cb {
            cb();
        }
    }
}

impl TimeProvider for ManualTimeProvider {
    fn utc_now(&self) -> DateTimeOffset {
        chrono::Utc::now().fixed_offset()
    }

    fn timestamp_frequency(&self) -> i64 {
        TimeSpan::TICKS_PER_SECOND
    }

    fn timestamp(&self) -> i64 {
        0
    }

    fn create_timer(&self, callback: TimerCallback, _due_time: TimeSpan, _period: TimeSpan) -> Box<dyn Timer> {
        let state = Arc::new(Mutex::new(ManualTimerState { active: true, callback: Some(callback) }));
        self.timers.lock().unwrap().push(Arc::clone(&state));
        Box::new(ManualTimer { state })
    }
}

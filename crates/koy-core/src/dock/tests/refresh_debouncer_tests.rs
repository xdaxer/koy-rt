use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::dock::{PostedWork, RefreshDebouncer};
use crate::time::{DateTimeOffset, TimeProvider, TimeSpan, Timer, TimerCallback};

struct ManualTimerState {
    active: AtomicBool,
    callback: Mutex<TimerCallback>,
}

struct ManualTimer(Arc<ManualTimerState>);

impl Timer for ManualTimer {
    fn change(&mut self, _due_time: TimeSpan, _period: TimeSpan) -> bool {
        true
    }

    fn dispose(&mut self) {
        self.0.active.store(false, Ordering::SeqCst);
    }
}

impl Drop for ManualTimer {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// `RefreshDebouncerTests.ManualTimeProvider`: zamanlayıcıyı elle tetikler (`fire_last`).
#[derive(Default)]
pub(super) struct ManualTimeProvider {
    timers: Mutex<Vec<Arc<ManualTimerState>>>,
}

impl ManualTimeProvider {
    pub fn new() -> Arc<Self> {
        Arc::new(ManualTimeProvider::default())
    }

    pub fn created(&self) -> usize {
        self.timers.lock().unwrap().len()
    }

    pub fn fire_last(&self) {
        let last = Arc::clone(self.timers.lock().unwrap().last().expect("zamanlayıcı yok"));
        if last.active.swap(false, Ordering::SeqCst) {
            (last.callback.lock().unwrap())();
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
        let state = Arc::new(ManualTimerState { active: AtomicBool::new(true), callback: Mutex::new(callback) });
        self.timers.lock().unwrap().push(Arc::clone(&state));
        Box::new(ManualTimer(state))
    }
}

fn counter() -> (Arc<AtomicUsize>, Arc<dyn Fn() + Send + Sync>) {
    let count = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&count);
    (count, Arc::new(move || {
        c.fetch_add(1, Ordering::SeqCst);
    }))
}

#[test]
fn birden_cok_istek_tek_eyleme_iner() {
    let time = ManualTimeProvider::new();
    let (count, action) = counter();
    let debouncer = RefreshDebouncer::new(action, TimeSpan::from_millis(100), None, Some(time.clone()));

    debouncer.request();
    debouncer.request();
    debouncer.request();

    assert_eq!(1, time.created());
    assert_eq!(0, count.load(Ordering::SeqCst));

    time.fire_last();

    assert_eq!(1, count.load(Ordering::SeqCst));
}

#[test]
fn eylem_sonrasi_yeni_istek_yeni_zamanlayici_kurar() {
    let time = ManualTimeProvider::new();
    let (count, action) = counter();
    let debouncer = RefreshDebouncer::new(action, TimeSpan::from_millis(100), None, Some(time.clone()));

    debouncer.request();
    time.fire_last();
    debouncer.request();
    time.fire_last();

    assert_eq!(2, count.load(Ordering::SeqCst));
    assert_eq!(2, time.created());
}

#[test]
fn cancel_bekleyen_istegi_iptal_eder() {
    let time = ManualTimeProvider::new();
    let (count, action) = counter();
    let debouncer = RefreshDebouncer::new(action, TimeSpan::from_millis(100), None, Some(time.clone()));

    debouncer.request();
    debouncer.cancel();
    time.fire_last();

    assert_eq!(0, count.load(Ordering::SeqCst));
    assert!(!debouncer.is_pending());
}

#[test]
fn dispose_sonrasi_eylem_calismaz() {
    let time = ManualTimeProvider::new();
    let (count, action) = counter();
    let debouncer = RefreshDebouncer::new(action, TimeSpan::from_millis(100), None, Some(time.clone()));

    debouncer.request();
    debouncer.dispose();
    time.fire_last();
    debouncer.request();

    assert_eq!(0, count.load(Ordering::SeqCst));
    assert_eq!(1, time.created());
}

#[test]
fn post_verilirse_eylem_onun_uzerinden_calisir() {
    let time = ManualTimeProvider::new();
    let (count, action) = counter();
    let posted: Arc<Mutex<Option<PostedWork>>> = Arc::new(Mutex::new(None));
    let p = Arc::clone(&posted);
    let post: Arc<dyn Fn(PostedWork) + Send + Sync> = Arc::new(move |work| {
        *p.lock().unwrap() = Some(work);
    });
    let debouncer = RefreshDebouncer::new(action, TimeSpan::from_millis(100), Some(post), Some(time.clone()));

    debouncer.request();
    time.fire_last();

    assert_eq!(0, count.load(Ordering::SeqCst));
    let work = posted.lock().unwrap().take();
    assert!(work.is_some());

    (work.unwrap())();
    assert_eq!(1, count.load(Ordering::SeqCst));
}

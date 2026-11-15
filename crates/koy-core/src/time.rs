//! .NET `TimeSpan`, `TimeProvider` ve `ITimer` karşılıkları.
//!
//! Core'daki zaman mantığı (Pomodoro, sayaç, çentik gecikmeleri, debounce) `TimeProvider` üzerinden
//! yürür ki testler gerçek beklemeden çalışsın. Anlamı birebir korumak için `TimeSpan` 100 ns'lik
//! tick sayısı olarak tutulur (C#'taki gibi işaretli `i64`); `Timeout.InfiniteTimeSpan` karşılığı
//! [`TimeSpan::INFINITE`] (-1 ms).

use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, FixedOffset, Local, Offset, TimeZone, Utc};

/// C# `DateTimeOffset` karşılığı.
pub type DateTimeOffset = DateTime<FixedOffset>;

/// C# `TimeSpan`: 100 ns'lik tick sayısı.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TimeSpan(pub i64);

impl TimeSpan {
    pub const TICKS_PER_MILLISECOND: i64 = 10_000;
    pub const TICKS_PER_SECOND: i64 = 10_000_000;
    pub const TICKS_PER_MINUTE: i64 = 600_000_000;
    pub const TICKS_PER_HOUR: i64 = 36_000_000_000;
    pub const TICKS_PER_DAY: i64 = 864_000_000_000;

    pub const ZERO: TimeSpan = TimeSpan(0);
    pub const MAX: TimeSpan = TimeSpan(i64::MAX);
    pub const MIN: TimeSpan = TimeSpan(i64::MIN);
    /// `Timeout.InfiniteTimeSpan`: zamanlayıcıyı kapatır.
    pub const INFINITE: TimeSpan = TimeSpan(-Self::TICKS_PER_MILLISECOND);

    pub const fn from_ticks(ticks: i64) -> Self {
        TimeSpan(ticks)
    }

    // Tamsayı aşırı yüklemeleri (.NET 9+: `TimeSpan.FromSeconds(5)` tamsayı sürümüne bağlanır, kesin).
    pub const fn from_millis(ms: i64) -> Self {
        TimeSpan(ms * Self::TICKS_PER_MILLISECOND)
    }
    pub const fn from_secs(s: i64) -> Self {
        TimeSpan(s * Self::TICKS_PER_SECOND)
    }
    pub const fn from_mins(m: i64) -> Self {
        TimeSpan(m * Self::TICKS_PER_MINUTE)
    }
    pub const fn from_hours(h: i64) -> Self {
        TimeSpan(h * Self::TICKS_PER_HOUR)
    }
    pub const fn from_days(d: i64) -> Self {
        TimeSpan(d * Self::TICKS_PER_DAY)
    }
    /// `new TimeSpan(hours, minutes, seconds)`.
    pub const fn new(hours: i64, minutes: i64, seconds: i64) -> Self {
        TimeSpan(hours * Self::TICKS_PER_HOUR + minutes * Self::TICKS_PER_MINUTE + seconds * Self::TICKS_PER_SECOND)
    }
    /// `new TimeSpan(days, hours, minutes, seconds, milliseconds)`.
    pub const fn new_full(days: i64, hours: i64, minutes: i64, seconds: i64, millis: i64) -> Self {
        TimeSpan(
            days * Self::TICKS_PER_DAY
                + hours * Self::TICKS_PER_HOUR
                + minutes * Self::TICKS_PER_MINUTE
                + seconds * Self::TICKS_PER_SECOND
                + millis * Self::TICKS_PER_MILLISECOND,
        )
    }

    // Kesirli aşırı yüklemeler: .NET gibi tick'e çevirip sıfıra doğru keser.
    fn from_double_ticks(ticks: f64) -> Self {
        if ticks.is_nan() {
            panic!("TimeSpan does not accept floating point Not-a-Number values.");
        }
        if ticks >= i64::MAX as f64 {
            return Self::MAX;
        }
        if ticks <= i64::MIN as f64 {
            return Self::MIN;
        }
        TimeSpan(ticks as i64)
    }
    pub fn from_milliseconds_f64(v: f64) -> Self {
        Self::from_double_ticks(v * Self::TICKS_PER_MILLISECOND as f64)
    }
    pub fn from_seconds_f64(v: f64) -> Self {
        Self::from_double_ticks(v * Self::TICKS_PER_SECOND as f64)
    }
    pub fn from_minutes_f64(v: f64) -> Self {
        Self::from_double_ticks(v * Self::TICKS_PER_MINUTE as f64)
    }
    pub fn from_hours_f64(v: f64) -> Self {
        Self::from_double_ticks(v * Self::TICKS_PER_HOUR as f64)
    }
    pub fn from_days_f64(v: f64) -> Self {
        Self::from_double_ticks(v * Self::TICKS_PER_DAY as f64)
    }

    pub const fn ticks(self) -> i64 {
        self.0
    }
    pub fn total_milliseconds(self) -> f64 {
        self.0 as f64 / Self::TICKS_PER_MILLISECOND as f64
    }
    pub fn total_seconds(self) -> f64 {
        self.0 as f64 / Self::TICKS_PER_SECOND as f64
    }
    pub fn total_minutes(self) -> f64 {
        self.0 as f64 / Self::TICKS_PER_MINUTE as f64
    }
    pub fn total_hours(self) -> f64 {
        self.0 as f64 / Self::TICKS_PER_HOUR as f64
    }
    pub fn total_days(self) -> f64 {
        self.0 as f64 / Self::TICKS_PER_DAY as f64
    }

    // Bileşenler: C#'taki gibi işaretli ve sıfıra doğru kesik.
    pub const fn days(self) -> i32 {
        (self.0 / Self::TICKS_PER_DAY) as i32
    }
    pub const fn hours(self) -> i32 {
        ((self.0 / Self::TICKS_PER_HOUR) % 24) as i32
    }
    pub const fn minutes(self) -> i32 {
        ((self.0 / Self::TICKS_PER_MINUTE) % 60) as i32
    }
    pub const fn seconds(self) -> i32 {
        ((self.0 / Self::TICKS_PER_SECOND) % 60) as i32
    }
    pub const fn milliseconds(self) -> i32 {
        ((self.0 / Self::TICKS_PER_MILLISECOND) % 1000) as i32
    }

    pub const fn duration(self) -> Self {
        TimeSpan(self.0.abs())
    }
    pub const fn is_infinite(self) -> bool {
        self.0 == Self::INFINITE.0
    }

    /// Negatif değer `Duration::ZERO` olur.
    pub fn to_std(self) -> Duration {
        if self.0 <= 0 {
            Duration::ZERO
        } else {
            Duration::from_nanos(self.0 as u64 * 100)
        }
    }
    pub fn from_std(d: Duration) -> Self {
        TimeSpan((d.as_nanos() / 100).min(i64::MAX as u128) as i64)
    }
    pub fn to_chrono(self) -> chrono::TimeDelta {
        chrono::TimeDelta::microseconds(self.0 / 10) + chrono::TimeDelta::nanoseconds((self.0 % 10) * 100)
    }
    pub fn from_chrono(d: chrono::TimeDelta) -> Self {
        // Mikro saniyeye sığmayan aralıklarda tick'e düş.
        match d.num_nanoseconds() {
            Some(ns) => TimeSpan(ns / 100),
            None => TimeSpan(d.num_microseconds().map_or(if d < chrono::TimeDelta::zero() { i64::MIN } else { i64::MAX }, |us| us.saturating_mul(10))),
        }
    }
}

impl fmt::Debug for TimeSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// C# `TimeSpan.ToString()` ("c" biçimi): `[-][d.]hh:mm:ss[.fffffff]`.
impl fmt::Display for TimeSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.0 < 0;
        let t = self.0.unsigned_abs();
        let tps = Self::TICKS_PER_SECOND as u64;
        let days = t / Self::TICKS_PER_DAY as u64;
        let hours = (t / Self::TICKS_PER_HOUR as u64) % 24;
        let minutes = (t / Self::TICKS_PER_MINUTE as u64) % 60;
        let seconds = (t / tps) % 60;
        let frac = t % tps;
        if neg {
            f.write_str("-")?;
        }
        if days > 0 {
            write!(f, "{days}.")?;
        }
        write!(f, "{hours:02}:{minutes:02}:{seconds:02}")?;
        if frac > 0 {
            write!(f, ".{frac:07}")?;
        }
        Ok(())
    }
}

impl Add for TimeSpan {
    type Output = TimeSpan;
    fn add(self, rhs: TimeSpan) -> TimeSpan {
        TimeSpan(self.0.checked_add(rhs.0).expect("TimeSpan overflowed because the duration is too long."))
    }
}
impl Sub for TimeSpan {
    type Output = TimeSpan;
    fn sub(self, rhs: TimeSpan) -> TimeSpan {
        TimeSpan(self.0.checked_sub(rhs.0).expect("TimeSpan overflowed because the duration is too long."))
    }
}
impl AddAssign for TimeSpan {
    fn add_assign(&mut self, rhs: TimeSpan) {
        *self = *self + rhs;
    }
}
impl SubAssign for TimeSpan {
    fn sub_assign(&mut self, rhs: TimeSpan) {
        *self = *self - rhs;
    }
}
impl Neg for TimeSpan {
    type Output = TimeSpan;
    fn neg(self) -> TimeSpan {
        TimeSpan(-self.0)
    }
}
/// `TimeSpan * double` (.NET: tick'ler çarpılır, sonuç yuvarlanır).
impl Mul<f64> for TimeSpan {
    type Output = TimeSpan;
    fn mul(self, rhs: f64) -> TimeSpan {
        TimeSpan::from_double_ticks((self.0 as f64 * rhs).round())
    }
}
/// `TimeSpan / double`.
impl Div<f64> for TimeSpan {
    type Output = TimeSpan;
    fn div(self, rhs: f64) -> TimeSpan {
        TimeSpan::from_double_ticks((self.0 as f64 / rhs).round())
    }
}
/// `TimeSpan / TimeSpan` → double.
impl Div for TimeSpan {
    type Output = f64;
    fn div(self, rhs: TimeSpan) -> f64 {
        self.0 as f64 / rhs.0 as f64
    }
}

/// `DateTimeOffset + TimeSpan` kolaylığı.
pub fn add_span(at: DateTimeOffset, span: TimeSpan) -> DateTimeOffset {
    at + span.to_chrono()
}

/// `DateTimeOffset - DateTimeOffset` → `TimeSpan`.
pub fn span_between(later: DateTimeOffset, earlier: DateTimeOffset) -> TimeSpan {
    TimeSpan::from_chrono(later - earlier)
}

/// `TimerCallback` karşılığı. Zamanlayıcı iş parçacığından çağrılabilir.
pub type TimerCallback = Box<dyn FnMut() + Send + 'static>;

/// `ITimer`: `change` ile yeniden kurulur, bırakılınca (`Drop`) ya da `dispose` ile durur.
pub trait Timer: Send {
    /// `dueTime` ya da `period` için [`TimeSpan::INFINITE`] o kısmı kapatır (C# ile aynı).
    fn change(&mut self, due_time: TimeSpan, period: TimeSpan) -> bool;
    fn dispose(&mut self);
}

/// .NET `TimeProvider` karşılığı.
pub trait TimeProvider: Send + Sync {
    fn utc_now(&self) -> DateTimeOffset;

    /// Yerel saat dilimi farkı (`LocalTimeZone`). Varsayılan: sistemin yerel saati.
    fn local_offset(&self, utc: DateTime<Utc>) -> FixedOffset {
        Local.offset_from_utc_datetime(&utc.naive_utc()).fix()
    }

    fn local_now(&self) -> DateTimeOffset {
        let utc = self.utc_now().with_timezone(&Utc);
        utc.with_timezone(&self.local_offset(utc))
    }

    fn timestamp_frequency(&self) -> i64;
    fn timestamp(&self) -> i64;

    /// `GetElapsedTime(start, end)`.
    fn elapsed_time(&self, start: i64, end: i64) -> TimeSpan {
        let freq = self.timestamp_frequency();
        let ticks = ((end - start) as f64 * (TimeSpan::TICKS_PER_SECOND as f64 / freq as f64)) as i64;
        TimeSpan(ticks)
    }

    /// `GetElapsedTime(start)`.
    fn elapsed_since(&self, start: i64) -> TimeSpan {
        self.elapsed_time(start, self.timestamp())
    }

    fn create_timer(&self, callback: TimerCallback, due_time: TimeSpan, period: TimeSpan) -> Box<dyn Timer>;
}

/// `TimeProvider.System`: gerçek saat; her zamanlayıcı yalnız kuruluyken bekleyen tek bir iş parçacığı kullanır.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTimeProvider;

static SYSTEM: SystemTimeProvider = SystemTimeProvider;

impl SystemTimeProvider {
    pub fn shared() -> &'static SystemTimeProvider {
        &SYSTEM
    }
}

fn process_epoch() -> Instant {
    use std::sync::OnceLock;
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

impl TimeProvider for SystemTimeProvider {
    fn utc_now(&self) -> DateTimeOffset {
        Utc::now().fixed_offset()
    }

    fn timestamp_frequency(&self) -> i64 {
        TimeSpan::TICKS_PER_SECOND
    }

    fn timestamp(&self) -> i64 {
        TimeSpan::from_std(process_epoch().elapsed()).0
    }

    fn create_timer(&self, callback: TimerCallback, due_time: TimeSpan, period: TimeSpan) -> Box<dyn Timer> {
        let mut t = SystemTimer::new(callback);
        t.change(due_time, period);
        Box::new(t)
    }
}

struct SystemTimerState {
    next: Option<Instant>,
    period: Option<Duration>,
    generation: u64,
    disposed: bool,
    worker_running: bool,
}

struct SystemTimerShared {
    state: Mutex<SystemTimerState>,
    cv: Condvar,
    callback: Mutex<TimerCallback>,
}

struct SystemTimer {
    shared: Arc<SystemTimerShared>,
}

impl SystemTimer {
    fn new(callback: TimerCallback) -> Self {
        SystemTimer {
            shared: Arc::new(SystemTimerShared {
                state: Mutex::new(SystemTimerState { next: None, period: None, generation: 0, disposed: false, worker_running: false }),
                cv: Condvar::new(),
                callback: Mutex::new(callback),
            }),
        }
    }

    fn worker(shared: Arc<SystemTimerShared>) {
        loop {
            let mut st = shared.state.lock().unwrap();
            loop {
                if st.disposed {
                    st.worker_running = false;
                    return;
                }
                match st.next {
                    // Kurulu değilse iş parçacığı biter: boşta bekleyen iş parçacığı kalmasın (AGENTS #5).
                    None => {
                        st.worker_running = false;
                        return;
                    }
                    Some(at) => {
                        let now = Instant::now();
                        if at <= now {
                            break;
                        }
                        st = shared.cv.wait_timeout(st, at - now).unwrap().0;
                    }
                }
            }
            let fired_at = st.next.unwrap();
            st.next = st.period.map(|p| fired_at + p);
            drop(st);
            // Geri çağrı kilitsiz çalışır ki kendi zamanlayıcısını `change` edebilsin.
            (shared.callback.lock().unwrap())();
        }
    }
}

impl Timer for SystemTimer {
    fn change(&mut self, due_time: TimeSpan, period: TimeSpan) -> bool {
        let mut st = self.shared.state.lock().unwrap();
        if st.disposed {
            return false;
        }
        st.generation += 1;
        st.next = if due_time.is_infinite() { None } else { Some(Instant::now() + due_time.to_std()) };
        st.period = if period.is_infinite() || period == TimeSpan::ZERO { None } else { Some(period.to_std()) };
        if st.next.is_some() && !st.worker_running {
            st.worker_running = true;
            let shared = Arc::clone(&self.shared);
            std::thread::Builder::new()
                .name("koy-timer".into())
                .spawn(move || SystemTimer::worker(shared))
                .expect("timer thread");
        }
        self.shared.cv.notify_all();
        true
    }

    fn dispose(&mut self) {
        let mut st = self.shared.state.lock().unwrap();
        st.disposed = true;
        st.next = None;
        self.shared.cv.notify_all();
    }
}

impl Drop for SystemTimer {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// Test saatleri: `tests/Core.Tests/FakeClock.cs` ve `StepTimeProvider.cs` karşılıkları.
/// Testlerin yanında sonraki katmanlar (Platform/App testleri) da kullanabilsin diye herkese açık.
pub mod testing {
    use super::*;
    use chrono::NaiveDate;

    struct FakeTimerSlot {
        // Tick cinsinden mutlak vade; None = kurulu değil.
        next: Option<i64>,
        period: i64,
        callback: Arc<Mutex<TimerCallback>>,
    }

    struct ClockInner {
        now: i64,
        timers: Vec<FakeTimerSlot>,
    }

    /// Ortak çekirdek: zaman yalnız `advance` ile ilerler, vadesi gelen zamanlayıcılar sırayla tetiklenir.
    struct ManualClock {
        inner: Arc<Mutex<ClockInner>>,
        frequency: i64,
        utc_base: DateTimeOffset,
        // FakeClock: tetiklenen zamanlayıcının "şimdi"si vadesinden geriye gitmez; periyot desteklenmez.
        periodic: bool,
    }

    struct ManualTimer {
        inner: Arc<Mutex<ClockInner>>,
        index: usize,
    }

    impl Timer for ManualTimer {
        fn change(&mut self, due_time: TimeSpan, period: TimeSpan) -> bool {
            let mut g = self.inner.lock().unwrap();
            let now = g.now;
            let slot = &mut g.timers[self.index];
            slot.next = if due_time.is_infinite() { None } else { Some(now + due_time.0) };
            slot.period = if period.is_infinite() { 0 } else { period.0 };
            true
        }
        fn dispose(&mut self) {
            self.inner.lock().unwrap().timers[self.index].next = None;
        }
    }

    impl Drop for ManualTimer {
        fn drop(&mut self) {
            self.dispose();
        }
    }

    impl ManualClock {
        fn new(frequency: i64, utc_base: DateTimeOffset, periodic: bool) -> Self {
            ManualClock { inner: Arc::new(Mutex::new(ClockInner { now: 0, timers: Vec::new() })), frequency, utc_base, periodic }
        }

        fn now_ticks(&self) -> i64 {
            self.inner.lock().unwrap().now
        }

        fn create(&self, callback: TimerCallback, due: TimeSpan, period: TimeSpan) -> Box<dyn Timer> {
            let mut g = self.inner.lock().unwrap();
            let now = g.now;
            g.timers.push(FakeTimerSlot {
                next: if due.is_infinite() { None } else { Some(now + due.0) },
                period: if period.is_infinite() || !self.periodic { 0 } else { period.0 },
                callback: Arc::new(Mutex::new(callback)),
            });
            let index = g.timers.len() - 1;
            Box::new(ManualTimer { inner: Arc::clone(&self.inner), index })
        }

        fn active(&self) -> usize {
            self.inner.lock().unwrap().timers.iter().filter(|t| t.next.is_some()).count()
        }

        fn advance(&self, by: TimeSpan) {
            let target = self.now_ticks() + by.0;
            loop {
                let cb = {
                    let mut g = self.inner.lock().unwrap();
                    // En erken vadeli; eşitlikte önce kurulan (OrderBy kararlıdır).
                    let next = g
                        .timers
                        .iter()
                        .enumerate()
                        .filter_map(|(i, t)| t.next.filter(|&n| n <= target).map(|n| (n, i)))
                        .min();
                    let Some((due, i)) = next else { break };
                    if due > g.now || self.periodic {
                        g.now = due;
                    }
                    let slot = &mut g.timers[i];
                    slot.next = if slot.period > 0 { Some(due + slot.period) } else { None };
                    Arc::clone(&slot.callback)
                };
                (cb.lock().unwrap())();
            }
            self.inner.lock().unwrap().now = target;
        }
    }

    /// `FakeClock`: tick frekansı 1000 (ms), UTC başlangıcı 2026-11-06 09:00:00Z, tek atımlık zamanlayıcılar.
    pub struct FakeClock(ManualClock);

    impl Default for FakeClock {
        fn default() -> Self {
            Self::new()
        }
    }

    impl FakeClock {
        pub fn new() -> Self {
            let base = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap().and_hms_opt(9, 0, 0).unwrap().and_utc().fixed_offset();
            FakeClock(ManualClock::new(1000, base, false))
        }
        pub fn advance(&self, by: TimeSpan) {
            self.0.advance(by)
        }
        pub fn active_timers(&self) -> usize {
            self.0.active()
        }
    }

    impl TimeProvider for FakeClock {
        fn utc_now(&self) -> DateTimeOffset {
            add_span(self.0.utc_base, TimeSpan(self.0.now_ticks()))
        }
        fn timestamp_frequency(&self) -> i64 {
            self.0.frequency
        }
        fn timestamp(&self) -> i64 {
            // GetTimestamp => (long)_now.TotalMilliseconds
            self.0.now_ticks() / TimeSpan::TICKS_PER_MILLISECOND
        }
        fn create_timer(&self, callback: TimerCallback, due_time: TimeSpan, period: TimeSpan) -> Box<dyn Timer> {
            self.0.create(callback, due_time, period)
        }
    }

    /// `StepTimeProvider`: tick frekansı `TicksPerSecond`, periyotlu zamanlayıcılar desteklenir.
    /// `GetUtcNow` C#'ta geçersiz kılınmadığı için gerçek saattir; burada da öyle.
    pub struct StepTimeProvider(ManualClock);

    impl Default for StepTimeProvider {
        fn default() -> Self {
            Self::new()
        }
    }

    impl StepTimeProvider {
        pub fn new() -> Self {
            StepTimeProvider(ManualClock::new(TimeSpan::TICKS_PER_SECOND, Utc::now().fixed_offset(), true))
        }
        pub fn advance(&self, by: TimeSpan) {
            self.0.advance(by)
        }
        pub fn active_timers(&self) -> usize {
            self.0.active()
        }
    }

    impl TimeProvider for StepTimeProvider {
        fn utc_now(&self) -> DateTimeOffset {
            Utc::now().fixed_offset()
        }
        fn timestamp_frequency(&self) -> i64 {
            self.0.frequency
        }
        fn timestamp(&self) -> i64 {
            self.0.now_ticks()
        }
        fn create_timer(&self, callback: TimerCallback, due_time: TimeSpan, period: TimeSpan) -> Box<dyn Timer> {
            self.0.create(callback, due_time, period)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn timespan_components_and_display() {
        let t = TimeSpan::new_full(1, 2, 3, 4, 5);
        assert_eq!((t.days(), t.hours(), t.minutes(), t.seconds(), t.milliseconds()), (1, 2, 3, 4, 5));
        assert_eq!(TimeSpan::new(0, 25, 0).to_string(), "00:25:00");
        assert_eq!(TimeSpan::from_millis(1500).to_string(), "00:00:01.5000000");
        assert_eq!((-TimeSpan::from_days(2)).to_string(), "-2.00:00:00");
        assert_eq!(TimeSpan::from_milliseconds_f64(1.99999).ticks(), 19999);
    }

    #[test]
    fn step_provider_fires_periodic_timers() {
        let clock = StepTimeProvider::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let h = Arc::clone(&hits);
        let _t = clock.create_timer(Box::new(move || { h.fetch_add(1, Ordering::SeqCst); }), TimeSpan::from_secs(1), TimeSpan::from_secs(1));
        clock.advance(TimeSpan::from_millis(3500));
        assert_eq!(hits.load(Ordering::SeqCst), 3);
        assert_eq!(clock.elapsed_since(0), TimeSpan::from_millis(3500));
    }

    #[test]
    fn fake_clock_one_shot_and_drop_stops() {
        let clock = FakeClock::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let h = Arc::clone(&hits);
        let t = clock.create_timer(Box::new(move || { h.fetch_add(1, Ordering::SeqCst); }), TimeSpan::from_secs(2), TimeSpan::INFINITE);
        assert_eq!(clock.active_timers(), 1);
        clock.advance(TimeSpan::from_secs(5));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(clock.timestamp(), 5000);
        drop(t);
        assert_eq!(clock.active_timers(), 0);
    }

    #[test]
    fn system_timer_fires() {
        let hits = Arc::new(AtomicUsize::new(0));
        let h = Arc::clone(&hits);
        let _t = SystemTimeProvider.create_timer(Box::new(move || { h.fetch_add(1, Ordering::SeqCst); }), TimeSpan::from_millis(10), TimeSpan::INFINITE);
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}

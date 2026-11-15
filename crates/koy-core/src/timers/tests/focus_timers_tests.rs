//! Karşılığı: `FocusTimersTests.cs`.

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::time::testing::StepTimeProvider;
use crate::time::TimeSpan;
use crate::timers::*;

fn short_config() -> PomodoroConfig {
    PomodoroConfig::with_sessions(TimeSpan::from_secs(10), TimeSpan::from_secs(4), TimeSpan::from_secs(8), 2)
}

fn create(time: &Arc<StepTimeProvider>) -> FocusTimers {
    FocusTimers::new(Some(short_config()), Some(TimeSpan::from_mins(1)), Some(time.clone()))
}

#[test]
fn pomodoro_pause_then_start_keeps_counting() {
    // Eski hata: Duraklat zamanlayıcıyı sonsuza çekiyordu, Başlat yeniden kurmuyordu; süre bir daha akmıyordu.
    let time = Arc::new(StepTimeProvider::new());
    let pomodoro = Pomodoro::new(Some(short_config()), Some(time.clone()));

    pomodoro.start();
    time.advance(TimeSpan::from_secs(3));
    pomodoro.pause();
    time.advance(TimeSpan::from_secs(30));
    assert_eq!(TimeSpan::from_secs(7), pomodoro.remaining());

    pomodoro.start();
    time.advance(TimeSpan::from_secs(2));

    assert_eq!(TimeSpan::from_secs(5), pomodoro.remaining());
}

#[test]
fn pomodoro_reset_then_start_counts_again() {
    let time = Arc::new(StepTimeProvider::new());
    let pomodoro = Pomodoro::new(Some(short_config()), Some(time.clone()));

    pomodoro.start();
    time.advance(TimeSpan::from_secs(4));
    pomodoro.reset();
    pomodoro.start();
    time.advance(TimeSpan::from_secs(2));

    assert_eq!(TimeSpan::from_secs(8), pomodoro.remaining());
}

#[test]
fn pomodoro_set_mode_switches_and_pauses() {
    let time = Arc::new(StepTimeProvider::new());
    let pomodoro = Pomodoro::new(Some(short_config()), Some(time.clone()));
    pomodoro.start();

    pomodoro.set_mode(PomodoroMode::ShortBreak);

    assert_eq!(PomodoroMode::ShortBreak, pomodoro.mode());
    assert_eq!(PomodoroState::Paused, pomodoro.state());
    assert_eq!(TimeSpan::from_secs(4), pomodoro.remaining());
}

#[test]
fn pomodoro_current_round_follows_cycle() {
    let time = Arc::new(StepTimeProvider::new());
    let pomodoro = Pomodoro::new(Some(short_config()), Some(time.clone())); // 2 turluk döngü
    pomodoro.start();

    assert_eq!((PomodoroMode::Work, 1), (pomodoro.mode(), pomodoro.current_round()));

    time.advance(pomodoro.remaining()); // 1. çalışma bitti -> kısa mola
    assert_eq!((PomodoroMode::ShortBreak, 1), (pomodoro.mode(), pomodoro.current_round()));

    time.advance(pomodoro.remaining()); // mola bitti -> 2. tur
    assert_eq!((PomodoroMode::Work, 2), (pomodoro.mode(), pomodoro.current_round()));

    time.advance(pomodoro.remaining()); // 2. çalışma bitti -> uzun mola
    assert_eq!((PomodoroMode::LongBreak, 2), (pomodoro.mode(), pomodoro.current_round()));

    time.advance(pomodoro.remaining()); // uzun mola bitti -> yeni döngü
    assert_eq!((PomodoroMode::Work, 1), (pomodoro.mode(), pomodoro.current_round()));
}

#[test]
fn running_keeps_counting_regardless_of_any_view() {
    // Sekme değiştirmek, çentiği gizlemek ya da başka bir alt sekmeye bakmak görünüm işidir; süre FocusTimers'ta akar.
    let time = Arc::new(StepTimeProvider::new());
    let timers = create(&time);
    timers.toggle(FocusKind::Focus);

    time.advance(TimeSpan::from_secs(3));
    let _ = timers.read(FocusKind::Stopwatch);
    let _ = timers.read(FocusKind::Countdown);
    time.advance(TimeSpan::from_secs(2));

    let running = timers.running().expect("çalışan zamanlayıcı olmalı");
    assert_eq!(FocusKind::Focus, running.kind);
    assert_eq!(TimeSpan::from_secs(5), running.value);
    assert!(running.counts_down);
}

#[test]
fn starting_another_timer_pauses_the_running_one() {
    let time = Arc::new(StepTimeProvider::new());
    let timers = create(&time);
    timers.toggle(FocusKind::Focus);
    time.advance(TimeSpan::from_secs(2));

    timers.toggle(FocusKind::Stopwatch);
    time.advance(TimeSpan::from_secs(3));

    assert_eq!(PomodoroState::Paused, timers.pomodoro.state());
    assert_eq!(TimeSpan::from_secs(8), timers.pomodoro.remaining());
    let running = timers.running().unwrap();
    assert_eq!(FocusKind::Stopwatch, running.kind);
    assert_eq!(TimeSpan::from_secs(3), running.value);
    assert!(!running.counts_down);
}

#[test]
fn toggle_on_running_timer_pauses() {
    let time = Arc::new(StepTimeProvider::new());
    let timers = create(&time);
    timers.toggle(FocusKind::Countdown);
    time.advance(TimeSpan::from_secs(10));

    timers.toggle(FocusKind::Countdown);
    time.advance(TimeSpan::from_secs(10));

    assert!(timers.running().is_none());
    assert_eq!(TimeSpan::from_secs(50), timers.countdown.remaining());
}

#[test]
fn idle_has_no_scheduled_timers() {
    // Boşta CPU kuralı: hiçbir şey çalışmıyorsa hiçbir zamanlayıcı kurulu kalmaz.
    let time = Arc::new(StepTimeProvider::new());
    let timers = create(&time);

    timers.toggle(FocusKind::Focus);
    assert_eq!(1, time.active_timers());
    timers.toggle(FocusKind::Focus);
    assert_eq!(0, time.active_timers());

    timers.toggle(FocusKind::Countdown);
    assert_eq!(1, time.active_timers());
    timers.reset(FocusKind::Countdown);
    assert_eq!(0, time.active_timers());

    timers.toggle(FocusKind::Stopwatch);
    timers.reset(FocusKind::Stopwatch);
    assert_eq!(0, time.active_timers());
    assert!(timers.running().is_none());
}

#[test]
fn select_pomodoro_kind_switches_between_focus_and_break() {
    let time = Arc::new(StepTimeProvider::new());
    let timers = create(&time);

    timers.select_pomodoro_kind(FocusKind::Break);
    assert_eq!(PomodoroMode::ShortBreak, timers.pomodoro.mode());
    assert_eq!(TimeSpan::from_secs(4), timers.read(FocusKind::Break).value);

    timers.select_pomodoro_kind(FocusKind::Focus);
    assert_eq!(PomodoroMode::Work, timers.pomodoro.mode());
}

#[test]
fn finished_is_raised_when_session_ends_and_break_starts_running() {
    let time = Arc::new(StepTimeProvider::new());
    let timers = create(&time);
    let finished: Arc<Mutex<Vec<FocusKind>>> = Arc::new(Mutex::new(Vec::new()));
    let f = Arc::clone(&finished);
    timers.finished().subscribe(move |kind| f.lock().unwrap().push(*kind));

    timers.toggle(FocusKind::Focus);
    time.advance(TimeSpan::from_secs(10));

    assert_eq!(vec![FocusKind::Focus], *finished.lock().unwrap());
    assert_eq!(FocusKind::Break, timers.running().unwrap().kind);
}

#[test]
fn finished_is_raised_when_countdown_ends() {
    let time = Arc::new(StepTimeProvider::new());
    let timers = create(&time);
    let finished: Arc<Mutex<Vec<FocusKind>>> = Arc::new(Mutex::new(Vec::new()));
    let f = Arc::clone(&finished);
    timers.finished().subscribe(move |kind| f.lock().unwrap().push(*kind));

    timers.toggle(FocusKind::Countdown);
    time.advance(TimeSpan::from_mins(1));

    assert_eq!(vec![FocusKind::Countdown], *finished.lock().unwrap());
    assert!(timers.running().is_none());
    assert_eq!(0, time.active_timers());
}

#[test]
fn changed_is_not_raised_on_every_second_tick() {
    // Saniyelik tik arayüze olay olarak akmaz; süre metni arayüzün kendi zamanlayıcısıyla okunur.
    let time = Arc::new(StepTimeProvider::new());
    let timers = create(&time);
    timers.toggle(FocusKind::Focus);
    let changes = Arc::new(AtomicI32::new(0));
    let c = Arc::clone(&changes);
    timers.changed().subscribe(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    time.advance(TimeSpan::from_secs(5));
    assert_eq!(0, changes.load(Ordering::SeqCst));

    timers.toggle(FocusKind::Focus);
    assert!(changes.load(Ordering::SeqCst) > 0);
}

#[test]
fn whole_minutes_rounds_down() {
    for (seconds, expected) in [(18 * 60 + 24, 18), (59, 0), (60, 1), (-1, 0)] {
        assert_eq!(expected, FocusReading::new(FocusKind::Focus, TimeSpan::from_secs(seconds), true).whole_minutes(), "{seconds}");
    }
}

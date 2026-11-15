//! Karşılığı: `PomodoroTests.cs`.

use std::sync::{Arc, Mutex};

use crate::timers::*;
use crate::time::TimeSpan;

#[test]
fn starts_in_work_mode_with_correct_duration() {
    let config = PomodoroConfig::new(TimeSpan::from_mins(25), TimeSpan::from_mins(5), TimeSpan::from_mins(15));
    let pomodoro = Pomodoro::new(Some(config), None);

    assert_eq!(PomodoroMode::Work, pomodoro.mode());
    assert_eq!(PomodoroState::Stopped, pomodoro.state());
    assert_eq!(TimeSpan::from_mins(25), pomodoro.remaining());
}

#[test]
fn start_changes_state_to_running() {
    let pomodoro = Pomodoro::new(None, None);
    pomodoro.start();
    assert_eq!(PomodoroState::Running, pomodoro.state());

    pomodoro.pause();
    assert_eq!(PomodoroState::Paused, pomodoro.state());
}

#[test]
fn tick_decrements_remaining_time() {
    let config = PomodoroConfig::new(TimeSpan::from_secs(10), TimeSpan::from_secs(5), TimeSpan::from_secs(15));
    let pomodoro = Pomodoro::new(Some(config), None);
    pomodoro.start();

    pomodoro.on_timer_tick();
    assert_eq!(TimeSpan::from_secs(9), pomodoro.remaining());
}

#[test]
fn session_completion_transitions_to_short_break_then_work() {
    let config = PomodoroConfig::with_sessions(TimeSpan::from_secs(2), TimeSpan::from_secs(1), TimeSpan::from_secs(5), 2);
    let pomodoro = Pomodoro::new(Some(config), None);
    pomodoro.start();

    let completed: Arc<Mutex<Option<PomodoroMode>>> = Arc::new(Mutex::new(None));
    let c = Arc::clone(&completed);
    pomodoro.session_completed().subscribe(move |m| *c.lock().unwrap() = Some(*m));

    // Tick 1
    pomodoro.on_timer_tick();
    assert_eq!(TimeSpan::from_secs(1), pomodoro.remaining());

    // Tick 2 (completes Work)
    pomodoro.on_timer_tick();
    assert_eq!(Some(PomodoroMode::Work), *completed.lock().unwrap());
    assert_eq!(1, pomodoro.completed_work_sessions());
    assert_eq!(PomodoroMode::ShortBreak, pomodoro.mode());
    assert_eq!(TimeSpan::from_secs(1), pomodoro.remaining());

    // Tick 3 (completes ShortBreak)
    pomodoro.on_timer_tick();
    assert_eq!(Some(PomodoroMode::ShortBreak), *completed.lock().unwrap());
    assert_eq!(PomodoroMode::Work, pomodoro.mode());
    assert_eq!(TimeSpan::from_secs(2), pomodoro.remaining());
}

#[test]
fn session_completion_after_target_sessions_transitions_to_long_break() {
    let config = PomodoroConfig::with_sessions(TimeSpan::from_secs(1), TimeSpan::from_secs(1), TimeSpan::from_secs(5), 2);
    let pomodoro = Pomodoro::new(Some(config), None);
    pomodoro.start();

    // 1st Work
    pomodoro.on_timer_tick();
    assert_eq!(PomodoroMode::ShortBreak, pomodoro.mode());

    // 1st ShortBreak
    pomodoro.on_timer_tick();
    assert_eq!(PomodoroMode::Work, pomodoro.mode());

    // 2nd Work -> should transition to LongBreak!
    pomodoro.on_timer_tick();
    assert_eq!(PomodoroMode::LongBreak, pomodoro.mode());
    assert_eq!(TimeSpan::from_secs(5), pomodoro.remaining());
}

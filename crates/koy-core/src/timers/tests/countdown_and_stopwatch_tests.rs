//! Karşılığı: `CountdownAndStopwatchTests.cs`.

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::time::testing::FakeClock;
use crate::time::TimeSpan;
use crate::timers::*;

fn create(clock: &Arc<FakeClock>, seconds: i64) -> CountdownTimer {
    CountdownTimer::new(TimeSpan::from_secs(seconds), Some(clock.clone()))
}

fn counter() -> Arc<AtomicI32> {
    Arc::new(AtomicI32::new(0))
}

// ---- CountdownTimerTests ----

#[test]
fn new_timer_is_idle_with_the_full_duration() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 90);

    assert_eq!(CountdownState::Idle, timer.state());
    assert_eq!(TimeSpan::from_secs(90), timer.remaining());
    assert_eq!(0, clock.active_timers());
}

#[test]
fn running_timer_counts_down_from_the_clock_without_ticking() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 60);

    timer.start();
    clock.advance(TimeSpan::from_secs(20));

    assert_eq!(CountdownState::Running, timer.state());
    assert_eq!(TimeSpan::from_secs(40), timer.remaining());
    // Saniyelik tik yok: çalışırken tek bir (bitiş için) zamanlayıcı var.
    assert_eq!(1, clock.active_timers());
}

#[test]
fn timer_finishes_once_at_the_end_and_raises_the_event() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 30);
    let finished = counter();
    let f = Arc::clone(&finished);
    timer.finished().subscribe(move |_| {
        f.fetch_add(1, Ordering::SeqCst);
    });

    timer.start();
    clock.advance(TimeSpan::from_secs(29));
    assert_eq!(0, finished.load(Ordering::SeqCst));

    clock.advance(TimeSpan::from_secs(2));

    assert_eq!(1, finished.load(Ordering::SeqCst));
    assert_eq!(CountdownState::Finished, timer.state());
    assert_eq!(TimeSpan::ZERO, timer.remaining());
    assert_eq!(0, clock.active_timers());
}

#[test]
fn pause_freezes_the_remaining_time_and_stops_the_background_timer() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 60);
    timer.start();
    clock.advance(TimeSpan::from_secs(15));

    timer.pause();
    clock.advance(TimeSpan::from_mins(10));

    assert_eq!(CountdownState::Paused, timer.state());
    assert_eq!(TimeSpan::from_secs(45), timer.remaining());
    assert_eq!(0, clock.active_timers());
}

#[test]
fn resume_after_pause_finishes_after_the_remaining_time() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 60);
    let finished = counter();
    let f = Arc::clone(&finished);
    timer.finished().subscribe(move |_| {
        f.fetch_add(1, Ordering::SeqCst);
    });

    timer.start();
    clock.advance(TimeSpan::from_secs(50));
    timer.pause();
    clock.advance(TimeSpan::from_hours(1));
    timer.start();
    clock.advance(TimeSpan::from_secs(9));
    assert_eq!(0, finished.load(Ordering::SeqCst));
    clock.advance(TimeSpan::from_secs(2));

    assert_eq!(1, finished.load(Ordering::SeqCst));
    assert_eq!(CountdownState::Finished, timer.state());
}

#[test]
fn reset_returns_to_the_full_duration() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 60);
    timer.start();
    clock.advance(TimeSpan::from_secs(10));

    timer.reset();

    assert_eq!(CountdownState::Idle, timer.state());
    assert_eq!(TimeSpan::from_secs(60), timer.remaining());
    assert_eq!(0, clock.active_timers());
}

#[test]
fn reset_while_running_prevents_a_late_finish() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 10);
    let finished = counter();
    let f = Arc::clone(&finished);
    timer.finished().subscribe(move |_| {
        f.fetch_add(1, Ordering::SeqCst);
    });
    timer.start();

    timer.reset();
    clock.advance(TimeSpan::from_mins(1));

    assert_eq!(0, finished.load(Ordering::SeqCst));
}

#[test]
fn start_after_finish_restarts_from_the_full_duration() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 5);
    timer.start();
    clock.advance(TimeSpan::from_secs(6));

    timer.start();

    assert_eq!(CountdownState::Running, timer.state());
    assert_eq!(TimeSpan::from_secs(5), timer.remaining());
}

#[test]
fn duration_can_only_change_while_not_running() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 60);

    assert!(timer.set_duration(TimeSpan::from_mins(5)));
    assert_eq!(TimeSpan::from_mins(5), timer.remaining());

    timer.start();
    assert!(!timer.set_duration(TimeSpan::from_mins(1)));
    assert_eq!(TimeSpan::from_mins(5), timer.duration());
}

#[test]
fn duration_is_clamped_to_the_allowed_range() {
    for (seconds, expected_seconds) in [(0, 1), (-30, 1), (10, 10)] {
        let clock = Arc::new(FakeClock::new());
        let timer = CountdownTimer::new(TimeSpan::from_secs(seconds), Some(clock.clone()));

        assert_eq!(TimeSpan::from_secs(expected_seconds), timer.duration(), "{seconds}");
    }
}

#[test]
fn duration_has_an_upper_limit() {
    let clock = Arc::new(FakeClock::new());
    let timer = CountdownTimer::new(TimeSpan::from_days(30), Some(clock.clone()));

    assert_eq!(CountdownTimer::MAX_DURATION, timer.duration());
}

#[test]
fn dispose_cancels_the_background_timer() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 10);
    let finished = counter();
    let f = Arc::clone(&finished);
    timer.finished().subscribe(move |_| {
        f.fetch_add(1, Ordering::SeqCst);
    });
    timer.start();

    timer.dispose();
    clock.advance(TimeSpan::from_mins(1));

    assert_eq!(0, finished.load(Ordering::SeqCst));
    assert_eq!(0, clock.active_timers());
}

#[test]
fn state_changes_are_reported() {
    let clock = Arc::new(FakeClock::new());
    let timer = create(&clock, 3);
    let states: Arc<Mutex<Vec<CountdownState>>> = Arc::new(Mutex::new(Vec::new()));
    let s = Arc::clone(&states);
    timer.state_changed().subscribe(move |state| s.lock().unwrap().push(*state));

    timer.start();
    timer.pause();
    timer.start();
    clock.advance(TimeSpan::from_secs(4));
    timer.reset();

    assert_eq!(
        vec![CountdownState::Running, CountdownState::Paused, CountdownState::Running, CountdownState::Finished, CountdownState::Idle],
        *states.lock().unwrap()
    );
}

// ---- LapStopwatchTests ----

#[test]
fn new_stopwatch_is_idle_at_zero() {
    let watch = LapStopwatch::new(Some(Arc::new(FakeClock::new())));

    assert_eq!(StopwatchState::Idle, watch.state());
    assert_eq!(TimeSpan::ZERO, watch.elapsed());
    assert!(watch.laps().is_empty());
}

#[test]
fn elapsed_follows_the_clock_only_while_running() {
    let clock = Arc::new(FakeClock::new());
    let watch = LapStopwatch::new(Some(clock.clone()));

    watch.start();
    clock.advance(TimeSpan::from_secs(12));
    watch.pause();
    clock.advance(TimeSpan::from_secs(100));

    assert_eq!(TimeSpan::from_secs(12), watch.elapsed());

    watch.start();
    clock.advance(TimeSpan::from_secs(3));

    assert_eq!(TimeSpan::from_secs(15), watch.elapsed());
}

#[test]
fn stopwatch_never_creates_a_timer() {
    let clock = Arc::new(FakeClock::new());
    let watch = LapStopwatch::new(Some(clock.clone()));

    watch.start();
    clock.advance(TimeSpan::from_mins(5));

    assert_eq!(0, clock.active_timers());
}

#[test]
fn laps_record_total_and_split_newest_first() {
    let clock = Arc::new(FakeClock::new());
    let watch = LapStopwatch::new(Some(clock.clone()));
    watch.start();

    clock.advance(TimeSpan::from_secs(10));
    watch.lap();
    clock.advance(TimeSpan::from_secs(7));
    watch.lap();

    let laps = watch.laps();
    assert_eq!(2, laps.len());
    assert_eq!(2, laps[0].number);
    assert_eq!(TimeSpan::from_secs(17), laps[0].total);
    assert_eq!(TimeSpan::from_secs(7), laps[0].split);
    assert_eq!(1, laps[1].number);
    assert_eq!(TimeSpan::from_secs(10), laps[1].split);
}

#[test]
fn lap_is_ignored_unless_running() {
    let clock = Arc::new(FakeClock::new());
    let watch = LapStopwatch::new(Some(clock.clone()));

    assert!(watch.lap().is_none());

    watch.start();
    clock.advance(TimeSpan::from_secs(2));
    watch.pause();

    assert!(watch.lap().is_none());
    assert!(watch.laps().is_empty());
}

#[test]
fn stopwatch_reset_clears_time_and_laps() {
    let clock = Arc::new(FakeClock::new());
    let watch = LapStopwatch::new(Some(clock.clone()));
    watch.start();
    clock.advance(TimeSpan::from_secs(5));
    watch.lap();

    watch.reset();

    assert_eq!(StopwatchState::Idle, watch.state());
    assert_eq!(TimeSpan::ZERO, watch.elapsed());
    assert!(watch.laps().is_empty());

    watch.start();
    clock.advance(TimeSpan::from_secs(1));
    assert_eq!(1, watch.lap().unwrap().number);
}

#[test]
fn lap_list_is_capped() {
    let clock = Arc::new(FakeClock::new());
    let watch = LapStopwatch::new(Some(clock.clone()));
    watch.start();

    for _ in 0..LapStopwatch::MAX_LAPS + 5 {
        clock.advance(TimeSpan::from_secs(1));
        watch.lap();
    }

    assert_eq!(LapStopwatch::MAX_LAPS, watch.laps().len());
    assert_eq!((LapStopwatch::MAX_LAPS + 5) as i32, watch.laps()[0].number);
}

// ---- CountdownPresetsTests ----

#[test]
fn step_moves_by_a_size_that_fits_the_current_duration() {
    let rows = [
        (5, true, 6),
        (9, true, 10),
        (10, true, 15),
        (12, true, 15),
        (59, true, 60),
        (60, true, 75),
        (1, false, 1),
        (7, false, 6),
        (10, false, 9),
        (12, false, 10),
        (15, false, 10),
        (60, false, 55),
        (70, false, 60),
        (75, false, 60),
    ];
    for (from, increase, expected) in rows {
        assert_eq!(TimeSpan::from_mins(expected), CountdownPresets::step(TimeSpan::from_mins(from), increase), "{from} {increase}");
    }
}

#[test]
fn step_stays_inside_the_allowed_range() {
    assert_eq!(CountdownTimer::MAX_DURATION, CountdownPresets::step(CountdownTimer::MAX_DURATION, true));
    assert_eq!(TimeSpan::from_mins(1), CountdownPresets::step(TimeSpan::from_secs(10), false));
}

#[test]
fn default_and_presets_are_valid_durations() {
    for m in CountdownPresets::MINUTES {
        let span = TimeSpan::from_mins(i64::from(m));
        assert!(span >= CountdownTimer::MIN_DURATION && span <= CountdownTimer::MAX_DURATION, "{m}");
    }
    assert!(CountdownPresets::MINUTES.contains(&CountdownPresets::DEFAULT_MINUTES));
}

// ---- DurationFormatTests ----

#[test]
fn countdown_formats_whole_seconds() {
    let rows = [(0, "00:00"), (-5, "00:00"), (1, "00:01"), (300, "05:00"), (3599, "59:59"), (3600, "1:00:00"), (3725, "1:02:05")];
    for (seconds, expected) in rows {
        assert_eq!(expected, DurationFormat::countdown(TimeSpan::from_secs(seconds)), "{seconds}");
    }
}

#[test]
fn countdown_rounds_a_fraction_of_a_second_up() {
    assert_eq!("00:01", DurationFormat::countdown(TimeSpan::from_millis(400)));
    assert_eq!("04:60".len(), DurationFormat::countdown(TimeSpan::from_millis(299_100)).len());
    assert_eq!("05:00", DurationFormat::countdown(TimeSpan::from_millis(299_100)));
}

#[test]
fn stopwatch_formats_tenths_and_truncates() {
    let rows = [
        (0, "00:00.0"),
        (100, "00:00.1"),
        (12_340, "00:12.3"),
        (59_999, "00:59.9"),
        (60_000, "01:00.0"),
        (3_600_000, "1:00:00.0"),
    ];
    for (milliseconds, expected) in rows {
        assert_eq!(expected, DurationFormat::stopwatch(TimeSpan::from_millis(milliseconds)), "{milliseconds}");
    }
}

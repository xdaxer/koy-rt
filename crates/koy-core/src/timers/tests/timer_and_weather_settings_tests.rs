//! Karşılığı: `TimerAndWeatherSettingsTests.cs` içindeki yalnız `PomodoroConfig` testleri.
//! Taşınmayan: `Weather_*` testleri (Weather modülü) ve `Pomodoro_settings_defaults_match_classic_25_5_15`
//! (Settings modülü: `AppSettings().Focus` varsayılanları gerekir).

use crate::timers::*;
use crate::time::TimeSpan;

#[test]
fn pomodoro_config_uses_custom_minutes() {
    let config = PomodoroConfig::from_minutes(50, 10, 30);

    assert_eq!(TimeSpan::from_mins(50), config.work_duration);
    assert_eq!(TimeSpan::from_mins(10), config.short_break_duration);
    assert_eq!(TimeSpan::from_mins(30), config.long_break_duration);
}

#[test]
fn pomodoro_config_clamps_absurd_values() {
    for (minutes, expected) in [(0, 1), (-5, 1), (100000, 180)] {
        let config = PomodoroConfig::from_minutes(minutes, minutes, minutes);

        assert_eq!(TimeSpan::from_mins(expected), config.work_duration, "{minutes}");
        assert_eq!(TimeSpan::from_mins(expected), config.short_break_duration, "{minutes}");
        assert_eq!(TimeSpan::from_mins(expected), config.long_break_duration, "{minutes}");
    }
}

#[test]
fn pomodoro_starts_with_the_configured_work_duration() {
    let pomodoro = Pomodoro::new(Some(PomodoroConfig::from_minutes(40, 8, 20)), None);

    assert_eq!(TimeSpan::from_mins(40), pomodoro.remaining());
}

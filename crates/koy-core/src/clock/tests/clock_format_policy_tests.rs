//! Karşılığı: `Clock/ClockFormatPolicyTests.cs`.
//! Taşınmayan: `Notch_defaults_keep_current_behaviour` (Settings modülüne bağlı: NotchSettings).

use chrono::{Datelike, NaiveDate, NaiveDateTime};

use crate::clock::*;

fn dt(y: i32, m: u32, d: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(0, 0, 0).unwrap()
}

#[test]
fn hour_format_setting_overrides_culture_only_when_explicit() {
    let rows = [
        (ClockHourFormat::Auto, true, true),
        (ClockHourFormat::Auto, false, false),
        (ClockHourFormat::Hour12, false, true),
        (ClockHourFormat::Hour24, true, false),
    ];
    for (setting, culture12, expected) in rows {
        assert_eq!(expected, ClockFormatPolicy::uses_twelve_hour_clock(setting, culture12), "{setting:?} {culture12}");
    }
}

#[test]
fn time_pattern_follows_hour_and_seconds() {
    let rows = [(false, false, "HH:mm"), (false, true, "HH:mm:ss"), (true, false, "h:mm tt"), (true, true, "h:mm:ss tt")];
    for (twelve_hour, seconds, expected) in rows {
        assert_eq!(expected, ClockFormatPolicy::time_pattern(twelve_hour, seconds), "{twelve_hour} {seconds}");
    }
}

#[test]
fn resolved_pattern_follows_culture_on_auto() {
    let rows = [
        ("en-US", ClockHourFormat::Auto, false, "h:mm tt"),
        ("tr-TR", ClockHourFormat::Auto, false, "HH:mm"),
        ("tr-TR", ClockHourFormat::Hour12, true, "h:mm:ss tt"),
        ("en-US", ClockHourFormat::Hour24, false, "HH:mm"),
    ];
    for (culture, setting, seconds, expected) in rows {
        let culture = Culture::from_name(culture).unwrap();
        assert_eq!(expected, ClockFormatPolicy::resolve_time_pattern(setting, seconds, &culture), "{} {setting:?}", culture.name);
    }
}

#[test]
fn time_pattern_formats_a_known_instant() {
    let at = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap().and_hms_opt(13, 7, 9).unwrap();
    let invariant = Culture::invariant();

    assert_eq!("13:07:09", format_date_time(at, None, &ClockFormatPolicy::time_pattern(false, true), &invariant).unwrap());
    assert_eq!("1:07 PM", format_date_time(at, None, &ClockFormatPolicy::time_pattern(true, false), &invariant).unwrap());
}

#[test]
fn first_day_of_week_uses_culture_only_for_system() {
    let rows = [
        (WeekStartSetting::System, DayOfWeek::Sunday, DayOfWeek::Sunday),
        (WeekStartSetting::System, DayOfWeek::Monday, DayOfWeek::Monday),
        (WeekStartSetting::Saturday, DayOfWeek::Monday, DayOfWeek::Saturday),
        (WeekStartSetting::Sunday, DayOfWeek::Monday, DayOfWeek::Sunday),
        (WeekStartSetting::Monday, DayOfWeek::Sunday, DayOfWeek::Monday),
    ];
    for (setting, culture, expected) in rows {
        assert_eq!(expected, ClockFormatPolicy::resolve_first_day_of_week(setting, culture), "{setting:?} {culture:?}");
    }
}

#[test]
fn week_of_year_with_monday_start_matches_iso_week() {
    // ISO 8601: 2026-03-22 (Pazar) 12. haftada (16-22 Mart).
    assert_eq!(12, ClockFormatPolicy::week_of_year(dt(2026, 3, 22), DayOfWeek::Monday));
}

#[test]
fn week_of_year_with_monday_start_matches_iso_week_for_every_day() {
    let mut day = dt(2000, 1, 1);
    while day.year() < 2041 {
        assert_eq!(day.iso_week().week() as i32, ClockFormatPolicy::week_of_year(day, DayOfWeek::Monday), "{day}");
        day += chrono::Duration::days(1);
    }
}

#[test]
fn week_of_year_moves_days_across_the_year_boundary() {
    // Calendar.GetWeekOfYear bunları 53 sayar; hafta, dördüncü gününün düştüğü yeni yıla aittir.
    let rows = [
        (2007, 12, 31, DayOfWeek::Monday, 1),
        (2024, 12, 29, DayOfWeek::Sunday, 1),
        // Yıl başındaki günler önceki yılın son haftasında kalır.
        (2021, 1, 1, DayOfWeek::Monday, 53),
    ];
    for (y, m, d, first_day, expected) in rows {
        assert_eq!(expected, ClockFormatPolicy::week_of_year(dt(y, m, d), first_day), "{y}-{m}-{d} {first_day:?}");
    }
}

#[test]
fn week_of_year_ignores_the_time_of_day() {
    let late = NaiveDate::from_ymd_opt(2007, 12, 31).unwrap().and_hms_opt(23, 59, 0).unwrap();
    assert_eq!(1, ClockFormatPolicy::week_of_year(late, DayOfWeek::Monday));
}

#[test]
fn week_of_year_first_week_starts_on_the_first_day_that_has_four_days_in_the_year() {
    // 4 gün kuralı: 2026 Ocak'ın ilk haftası Cumartesi başlangıçlıysa 3 Ocak Cumartesi'dir (Ocak 3-9, yılın 7 günü).
    assert_eq!(1, ClockFormatPolicy::week_of_year(dt(2026, 1, 3), DayOfWeek::Saturday));
    assert_eq!(1, ClockFormatPolicy::week_of_year(dt(2026, 1, 9), DayOfWeek::Saturday));
    assert_eq!(2, ClockFormatPolicy::week_of_year(dt(2026, 1, 10), DayOfWeek::Saturday));
}

#[test]
fn valid_custom_date_formats_are_accepted() {
    for format in ["dd.MM.yyyy", "ddd, d MMMM", "yyyy-MM-dd"] {
        assert!(ClockFormatPolicy::is_valid_custom_date_format(Some(format)), "{format}");
    }
}

#[test]
fn invalid_custom_date_formats_are_rejected() {
    for format in [None, Some(""), Some("   "), Some("\\"), Some("dd\tMM")] {
        assert!(!ClockFormatPolicy::is_valid_custom_date_format(format), "{format:?}");
    }
}

#[test]
fn custom_date_format_longer_than_the_limit_is_rejected() {
    let too_long = "d".repeat(ClockFormatPolicy::MAX_CUSTOM_DATE_FORMAT_LENGTH + 1);
    let at_limit = "d".repeat(ClockFormatPolicy::MAX_CUSTOM_DATE_FORMAT_LENGTH);

    assert!(!ClockFormatPolicy::is_valid_custom_date_format(Some(&too_long)));
    assert!(ClockFormatPolicy::is_valid_custom_date_format(Some(&at_limit)));
}

#[test]
fn second_clock_is_on_only_with_a_time_zone_id() {
    for (id, expected) in [(None, false), (Some(""), false), (Some("Tokyo Standard Time"), true)] {
        assert_eq!(expected, ClockFormatPolicy::has_second_clock(id), "{id:?}");
    }
}

#[test]
fn peek_line_adds_only_what_is_enabled() {
    let rows = [
        ("12:00", None, None, "12:00"),
        ("12:00", Some(""), Some("  "), "12:00"),
        ("12:00", Some("12 Nov"), None, "12:00 · 12 Nov"),
        ("12:00", None, Some("UTC+9 21:00"), "12:00 · UTC+9 21:00"),
        ("12:00", Some("12 Nov"), Some("UTC+9 21:00"), "12:00 · 12 Nov · UTC+9 21:00"),
        ("", None, Some("UTC+9 21:00"), " · UTC+9 21:00"),
    ];
    for (time, date, second, expected) in rows {
        assert_eq!(expected, ClockFormatPolicy::compose_peek_line(time, date, second), "{time:?} {date:?} {second:?}");
    }
}

#[test]
fn turkish_culture_names_and_designators_format() {
    // Ek denetim (C# testinde yok): kültür verisi biçimleyiciye gerçekten akıyor.
    let at = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap().and_hms_opt(13, 7, 9).unwrap();
    let tr = Culture::tr_tr();
    assert_eq!("5 Ocak Pazartesi", format_date_time(at, None, "d MMMM dddd", &tr).unwrap());
    assert_eq!("1:07 ÖS", format_date_time(at, None, "h:mm tt", &tr).unwrap());
}

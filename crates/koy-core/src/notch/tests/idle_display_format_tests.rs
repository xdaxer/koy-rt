use chrono::{NaiveDate, NaiveDateTime};

use crate::clock::Culture;
use crate::notch::IdleDisplayFormat;

fn tr() -> Culture {
    Culture::tr_tr()
}

fn en() -> Culture {
    Culture::en_us()
}

fn dt(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, s).unwrap()
}

// 2026-11-17 bir salı.
fn tuesday() -> NaiveDateTime {
    dt(2026, 11, 17, 19, 21, 5)
}

#[test]
fn day_time_uses_short_day_name_and_24_hour_clock() {
    assert_eq!("Sal 19:21", IdleDisplayFormat::day_time(tuesday(), &tr(), "HH:mm"));
    assert_eq!("Tue 19:21", IdleDisplayFormat::day_time(tuesday(), &en(), "HH:mm"));
    assert_eq!("Sal 07:05", IdleDisplayFormat::day_time(dt(2026, 11, 17, 7, 5, 0), &tr(), "HH:mm"));
}

#[test]
fn day_time_uses_the_given_time_pattern() {
    assert_eq!("Tue 7:21 PM", IdleDisplayFormat::day_time(tuesday(), &en(), "h:mm tt"));
    assert_eq!("Sal 19:21:05", IdleDisplayFormat::day_time(tuesday(), &tr(), "HH:mm:ss"));
}

#[test]
fn short_date_follows_language_day_month_order() {
    assert_eq!("17 Kas", IdleDisplayFormat::short_date(tuesday(), &tr(), None));
    assert_eq!("Nov 17", IdleDisplayFormat::short_date(tuesday(), &en(), None));
}

#[test]
fn short_date_uses_valid_custom_format_and_ignores_invalid_one() {
    assert_eq!("17.11.2026", IdleDisplayFormat::short_date(tuesday(), &tr(), Some("dd.MM.yyyy")));
    assert_eq!("17 Kas", IdleDisplayFormat::short_date(tuesday(), &tr(), Some("")));
    assert_eq!("Nov 17", IdleDisplayFormat::short_date(tuesday(), &en(), Some("   ")));
}

#[test]
fn percent_follows_language_sign_position() {
    // (değer, tr, en); taşan değer kıskaçlanır.
    let rows = [(43, "%43", "43%"), (100, "%100", "100%"), (150, "%100", "100%"), (-5, "%0", "0%")];
    for (value, tr_expected, en_expected) in rows {
        assert_eq!(tr_expected, IdleDisplayFormat::percent(value, &tr()), "{value}");
        assert_eq!(en_expected, IdleDisplayFormat::percent(value, &en()), "{value}");
    }
}

#[test]
fn is_low_only_when_discharging_at_or_below_threshold() {
    // (yüzde, şarjda, beklenen); şarjda düşük renk yok.
    let rows = [(12, false, true), (20, false, true), (21, false, false), (12, true, false)];
    for (percent, charging, expected) in rows {
        assert_eq!(expected, IdleDisplayFormat::is_low(percent, charging), "{percent} {charging}");
    }
}

#[test]
fn arc_point_starts_at_top_and_goes_clockwise() {
    // en üst, sağ, alt, sol
    let rows = [(0.0, 9.0, 1.75), (25.0, 16.25, 9.0), (50.0, 9.0, 16.25), (75.0, 1.75, 9.0)];
    for (percent, x, y) in rows {
        let (px, py) = IdleDisplayFormat::arc_point(percent, 7.25, 9.0);
        assert!((x - px).abs() < 0.0005, "{percent}: x {px}");
        assert!((y - py).abs() < 0.0005, "{percent}: y {py}");
    }
}

fn arc(percent: i32) -> String {
    IdleDisplayFormat::arc_path(percent, IdleDisplayFormat::RING_SIZE, IdleDisplayFormat::RING_RADIUS)
}

#[test]
fn arc_path_is_empty_at_zero() {
    assert_eq!("", arc(0));
    assert_eq!("", arc(-3));
}

#[test]
fn arc_path_uses_small_arc_up_to_half_and_large_arc_after() {
    assert_eq!("M 9,1.75 A 7.25,7.25 0 0 1 16.25,9", arc(25));
    assert!(arc(50).contains(" 0 0 1 "));
    assert!(arc(51).contains(" 0 1 1 "));
}

#[test]
fn arc_path_draws_full_circle_as_two_half_arcs() {
    assert_eq!("M 9,1.75 A 7.25,7.25 0 1 1 9,16.25 A 7.25,7.25 0 1 1 9,1.75", arc(100));
    assert_eq!(arc(100), arc(140));
}

#[test]
fn arc_path_is_culture_invariant() {
    // Rust'ta süreç kültürü yoktur; çıktı her zaman nokta ayraçlıdır (C# testi CurrentCulture = tr-TR ile sınar).
    assert!(!arc(43).split(' ').nth(1).unwrap().contains(",7"));
    assert!(arc(43).starts_with("M 9,1.75 A 7.25,7.25"));
}

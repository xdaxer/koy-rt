use chrono::{Duration, NaiveDate};

use crate::clock::DayOfWeek;

/// Hafta görünümünün gün dizilimi. Saf: tarih ve haftanın ilk günü dışarıdan gelir
/// (`ClockFormatPolicy::resolve_first_day_of_week`), böylece hafta görünümü ay ızgarasıyla aynı günle başlar.
pub struct WeekGrid;

impl WeekGrid {
    pub const DAYS_IN_WEEK: i32 = 7;

    /// Günün ait olduğu haftanın ilk günü (verilen günü içeren, `first_day` ile başlayan hafta).
    pub fn start(day: NaiveDate, first_day: DayOfWeek) -> NaiveDate {
        let offset = (DayOfWeek::of(day).index() - first_day.index() + Self::DAYS_IN_WEEK) % Self::DAYS_IN_WEEK;
        day - Duration::days(i64::from(offset))
    }

    /// Günün haftasındaki 7 gün, `first_day` ile başlayarak.
    pub fn days(day: NaiveDate, first_day: DayOfWeek) -> Vec<NaiveDate> {
        let start = Self::start(day, first_day);
        (0..Self::DAYS_IN_WEEK).map(|i| start + Duration::days(i64::from(i))).collect()
    }
}

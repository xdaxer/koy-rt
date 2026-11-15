use chrono::{Datelike, Duration, Local, NaiveDate};

use crate::clock::DayOfWeek;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarDay {
    pub date: NaiveDate,
    pub is_current_month: bool,
    pub is_today: bool,
    pub is_weekend: bool,
}

/// Ay görünümü için 6×7 = 42 günlük ızgara üretir. Haftanın ilk günü varsayılan Pazartesi; ayardan
/// (`ClockFormatPolicy::resolve_first_day_of_week`) Cumartesi ya da Pazar da olabilir.
/// Saf mantık, kültür ve tarih bağımsız test edilebilir.
pub struct MonthGrid;

impl MonthGrid {
    /// `Build(year, month, today)`: haftanın ilk günü Pazartesi. `today` verilmezse bugünün tarihi (`DateTime.Today`).
    pub fn build(year: i32, month: u32, today: Option<NaiveDate>) -> Vec<CalendarDay> {
        Self::build_with_first_day(year, month, today, DayOfWeek::Monday)
    }

    pub fn build_with_first_day(year: i32, month: u32, today: Option<NaiveDate>, first_day: DayOfWeek) -> Vec<CalendarDay> {
        let current_today = today.unwrap_or_else(|| Local::now().date_naive());
        let first_of_month = NaiveDate::from_ymd_opt(year, month, 1).expect("geçersiz yıl/ay");

        // Ayın ilk gününden geriye, seçilen haftanın ilk gününe kadar.
        let start_offset = (DayOfWeek::of(first_of_month).index() - first_day.index() + 7) % 7;

        let start_date = first_of_month - Duration::days(i64::from(start_offset));
        let mut days = Vec::with_capacity(42);

        for i in 0..42 {
            let date = start_date + Duration::days(i);
            let is_current_month = date.month() == month && date.year() == year;
            let is_today = date == current_today;
            let is_weekend = matches!(DayOfWeek::of(date), DayOfWeek::Saturday | DayOfWeek::Sunday);
            days.push(CalendarDay { date, is_current_month, is_today, is_weekend });
        }

        days
    }

    /// Izgara sütunlarının gün sırası: ilk gün seçimden başlar, yedi gün.
    pub fn weekday_order(first_day: DayOfWeek) -> Vec<DayOfWeek> {
        (0..7).map(|i| DayOfWeek::from_index((first_day.index() + i) % 7)).collect()
    }
}

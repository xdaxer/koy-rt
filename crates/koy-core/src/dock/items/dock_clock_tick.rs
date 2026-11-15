use chrono::NaiveDateTime;

use crate::clock::{format_date_time, Culture};
use crate::notch::ClockTick;
use crate::time::TimeSpan;

/// Dock saat öğesinin zamanlama ve biçim kuralları. Saniye gösterilmediği için dakika başında bir kez uyanılır.
pub struct DockClockTick;

impl DockClockTick {
    /// Bir sonraki dakika başına kalan süre (Core.Notch.ClockTick ile ortak hesaplanır).
    pub fn delay_to_next_minute(now: NaiveDateTime) -> TimeSpan {
        ClockTick::delay_to_next_minute(now)
    }

    /// `now.ToString("t", culture)`.
    pub fn format_time(now: NaiveDateTime, culture: &Culture) -> String {
        format_date_time(now, None, "t", culture).unwrap_or_default()
    }

    /// `now.ToString("ddd d MMM", culture)`.
    pub fn format_date(now: NaiveDateTime, culture: &Culture) -> String {
        format_date_time(now, None, "ddd d MMM", culture).unwrap_or_default()
    }
}

//! Karşılığı: `koy/src/Core/Clock/`.
//!
//! C#'ın `CultureInfo`, `DayOfWeek`, `TimeZoneInfo` ve özel tarih biçimi (`DateTime.ToString(format, culture)`)
//! karşılıkları Rust'ta yoktur; saat ve takvim mantığı bunlara dayandığı için bu modülde küçük birer karşılık
//! bulunur (`culture`, `day_of_week`, `time_zone`, `dotnet_format`). Takvim ve birimler modülü bunları kullanır.

mod clock_enums;
mod clock_format_policy;
mod culture;
mod day_of_week;
mod dotnet_format;
mod second_clock;
mod time_zone;

pub use clock_enums::{ClockHourFormat, WeekStartSetting};
pub use clock_format_policy::ClockFormatPolicy;
pub use culture::Culture;
pub use day_of_week::DayOfWeek;
pub use dotnet_format::{format_date_time, FormatException};
pub use second_clock::{SecondClock, SecondClockReading};
pub use time_zone::{AdjustmentRule, TimeZoneInfo, TransitionTime};

#[cfg(test)]
mod tests;

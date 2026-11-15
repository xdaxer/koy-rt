use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

/// C# `System.DayOfWeek`: Pazar = 0 ... Cumartesi = 6. `(int)day` dönüşümleri ([`DayOfWeek::index`]) C# ile aynı sayıyı verir
/// (chrono'nun `Weekday`i Pazartesi = 0 sayar, o yüzden kendi enum'umuz var).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DayOfWeek {
    Sunday = 0,
    Monday = 1,
    Tuesday = 2,
    Wednesday = 3,
    Thursday = 4,
    Friday = 5,
    Saturday = 6,
}

impl DayOfWeek {
    /// `(int)day`.
    pub const fn index(self) -> i32 {
        self as i32
    }

    /// `(DayOfWeek)(n)`; yalnız 0-6 için anlamlıdır (C#'ta dışı geçersiz enum değeri olur).
    pub fn from_index(n: i32) -> DayOfWeek {
        match n {
            0 => DayOfWeek::Sunday,
            1 => DayOfWeek::Monday,
            2 => DayOfWeek::Tuesday,
            3 => DayOfWeek::Wednesday,
            4 => DayOfWeek::Thursday,
            5 => DayOfWeek::Friday,
            6 => DayOfWeek::Saturday,
            _ => panic!("DayOfWeek dışı değer: {n}"),
        }
    }

    pub fn from_chrono(day: Weekday) -> DayOfWeek {
        Self::from_index(day.num_days_from_sunday() as i32)
    }

    /// `DateOnly.DayOfWeek` / `DateTime.DayOfWeek`.
    pub fn of(date: NaiveDate) -> DayOfWeek {
        Self::from_chrono(date.weekday())
    }
}

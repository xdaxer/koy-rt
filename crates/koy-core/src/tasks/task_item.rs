//! Karşılığı: `Tasks/TaskItem.cs`.
//!
//! JSON (`tasks.json`): C# sürümüyle birebir (PascalCase alan adları; `Guid` "D" metni, `DateOnly` `yyyy-MM-dd`,
//! `TimeOnly` `HH:mm:ss`, `DateTimeOffset` ISO 8601).

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use serde::{Deserialize, Deserializer, Serialize};

use super::guid::Guid;
use crate::time::DateTimeOffset;

/// JSON `null` → `T::default()` (C#'ta `null` dize sonradan `IsNullOrWhiteSpace` ile elenir).
fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// Odak görevi. `day` görevin ait olduğu gün (varsayılan oluşturulduğu gün); `time` o gün
/// için isteğe bağlı saat (HH:mm).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct TaskItem {
    pub id: Guid,
    #[serde(deserialize_with = "null_as_default")]
    pub title: String,
    pub day: NaiveDate,
    pub time: Option<NaiveTime>,
    pub done: bool,
    pub created_at: DateTimeOffset,
}

impl TaskItem {
    pub fn new(id: Guid, title: impl Into<String>, day: NaiveDate, time: Option<NaiveTime>, done: bool, created_at: DateTimeOffset) -> Self {
        TaskItem { id, title: title.into(), day, time, done, created_at }
    }
}

/// C# `default(TaskItem)` alanları: eksik JSON alanı bu değerleri alır (`DateOnly`/`DateTimeOffset` varsayılanı 0001-01-01).
impl Default for TaskItem {
    fn default() -> Self {
        let min_day = NaiveDate::from_ymd_opt(1, 1, 1).unwrap();
        TaskItem {
            id: Guid::EMPTY,
            title: String::new(),
            day: min_day,
            time: None,
            done: false,
            created_at: DateTime::<FixedOffset>::from_naive_utc_and_offset(
                min_day.and_hms_opt(0, 0, 0).unwrap(),
                FixedOffset::east_opt(0).unwrap(),
            ),
        }
    }
}

use super::clock_format_policy::ClockFormatPolicy;
use super::culture::Culture;
use super::dotnet_format::format_date_time;
use super::time_zone::TimeZoneInfo;
use crate::time::{DateTimeOffset, TimeSpan};

/// İkinci saat satırı: kısa etiket ve biçimlenmiş saat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondClockReading {
    pub label: String,
    pub time: String,
}

impl SecondClockReading {
    pub fn new(label: impl Into<String>, time: impl Into<String>) -> Self {
        SecondClockReading { label: label.into(), time: time.into() }
    }

    pub fn text(&self) -> String {
        if self.label.is_empty() { self.time.clone() } else { format!("{} {}", self.label, self.time) }
    }
}

/// İkinci saat dilimi (issue #77). Saf: şimdiki an dışarıdan gelir. Kimlik boş, bilinmeyen ya da bozuksa "ikinci saat yok"
/// (None) döner; hiçbir durumda istisna fırlatmaz.
pub struct SecondClock;

impl SecondClock {
    /// Kimliği saat dilimine çözer; boş, bulunamayan ya da bozuk kimlikte None.
    pub fn resolve(time_zone_id: Option<&str>) -> Option<TimeZoneInfo> {
        if !ClockFormatPolicy::has_second_clock(time_zone_id) {
            return None;
        }

        // C#'taki TimeZoneNotFound/InvalidTimeZone/Argument/Unauthorized/Security istisnalarının hepsi "yok" sayılır.
        TimeZoneInfo::find_system_time_zone_by_id(time_zone_id.unwrap_or_default().trim())
    }

    /// İkinci saat satırı; kimlik çözülemezse None. Etiket boşsa "UTC+3" gibi kısa fark etiketi kullanılır.
    pub fn read_by_id(
        time_zone_id: Option<&str>,
        now: DateTimeOffset,
        time_pattern: &str,
        culture: &Culture,
        label: Option<&str>,
    ) -> Option<SecondClockReading> {
        Self::resolve(time_zone_id).map(|zone| Self::read(&zone, now, time_pattern, culture, label))
    }

    /// Verilen saat dilimindeki saat satırı.
    pub fn read(zone: &TimeZoneInfo, now: DateTimeOffset, time_pattern: &str, culture: &Culture, label: Option<&str>) -> SecondClockReading {
        let local = TimeZoneInfo::convert_time(now, zone);
        let offset = TimeSpan::from_secs(i64::from(local.offset().local_minus_utc()));
        let text = match label {
            Some(l) if !crate::text::is_null_or_white_space(Some(l)) => l.trim().to_string(),
            _ => Self::offset_label(offset),
        };
        // Desen çağıranda bozuksa C#'ta FormatException fırlar; burada da panik (programcı hatası).
        let time = format_date_time(local.naive_local(), Some(offset), time_pattern, culture).expect("geçersiz saat deseni");
        SecondClockReading { label: text, time }
    }

    /// "UTC", "UTC+3", "UTC-5", "UTC+5:30" biçiminde UTC farkı.
    pub fn offset_label(offset: TimeSpan) -> String {
        if offset == TimeSpan::ZERO {
            return "UTC".to_string();
        }

        let sign = if offset < TimeSpan::ZERO { "-" } else { "+" };
        let abs = offset.duration();
        let hours = abs.total_hours() as i32;
        if abs.minutes() == 0 {
            format!("UTC{sign}{hours}")
        } else {
            format!("UTC{sign}{hours}:{:02}", abs.minutes())
        }
    }
}

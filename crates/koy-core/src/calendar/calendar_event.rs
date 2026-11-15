use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde::de::Error as _;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::iso_time;

/// Takvim görünümü: ay ızgarası, hafta listesi ya da tek gün.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CalendarViewMode {
    #[default]
    Month,
    Week,
    Day,
}

/// Etkinliğin kaynağı: yerel defter ya da salt okunur bağlı hesaplar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CalendarEventSource {
    #[default]
    Local = 0,
    Google = 1,
    Outlook = 2,
}

impl CalendarEventSource {
    pub(crate) fn name(self) -> &'static str {
        match self {
            CalendarEventSource::Local => "Local",
            CalendarEventSource::Google => "Google",
            CalendarEventSource::Outlook => "Outlook",
        }
    }

    pub(crate) fn from_number(n: i64) -> Option<CalendarEventSource> {
        match n {
            0 => Some(CalendarEventSource::Local),
            1 => Some(CalendarEventSource::Google),
            2 => Some(CalendarEventSource::Outlook),
            _ => None,
        }
    }

    fn from_name(name: &str) -> Option<CalendarEventSource> {
        // JsonStringEnumConverter okurken büyük/küçük harfe duyarsızdır.
        [CalendarEventSource::Local, CalendarEventSource::Google, CalendarEventSource::Outlook]
            .into_iter()
            .find(|s| s.name().eq_ignore_ascii_case(name))
    }
}

/// Takvim etkinliği. `start` yerel saattir (saat dilimi taşımaz); `duration_minutes` 0 ise tek bir an (hatırlatma gibi).
/// Harici kaynaklı etkinlikler salt okunurdur.
///
/// JSON (System.Text.Json biçimi, PascalCase): `Id, Title, Start, DurationMinutes, Source` ve C#'ın salt okunur
/// özellikleri olan `End, Date, IsReadOnly` (C# bunları da yazar, okurken yok sayar). `Source` burada metin olarak
/// yazılır (harici takvim önbelleği `UseStringEnumConverter` ile böyle yazar); `EventStore` sayı yazar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start: NaiveDateTime,
    pub duration_minutes: i32,
    pub source: CalendarEventSource,
}

impl CalendarEvent {
    /// `new CalendarEvent(id, title, start, durationMinutes)`: kaynak varsayılanı `Local`.
    pub fn new(id: impl Into<String>, title: impl Into<String>, start: NaiveDateTime, duration_minutes: i32) -> Self {
        Self::with_source(id, title, start, duration_minutes, CalendarEventSource::Local)
    }

    pub fn with_source(
        id: impl Into<String>,
        title: impl Into<String>,
        start: NaiveDateTime,
        duration_minutes: i32,
        source: CalendarEventSource,
    ) -> Self {
        CalendarEvent { id: id.into(), title: title.into(), start, duration_minutes, source }
    }

    pub fn end(&self) -> NaiveDateTime {
        self.start + Duration::minutes(i64::from(self.duration_minutes))
    }

    pub fn date(&self) -> NaiveDate {
        self.start.date()
    }

    pub fn is_read_only(&self) -> bool {
        self.source != CalendarEventSource::Local
    }

    /// `Source` alanını sayı yazan (EventStore) ya da metin yazan (önbellek) biçimde serileştirir.
    pub(crate) fn serialize_with<S: Serializer>(&self, serializer: S, numeric_source: bool) -> Result<S::Ok, S::Error> {
        let mut st = serializer.serialize_struct("CalendarEvent", 8)?;
        st.serialize_field("Id", &self.id)?;
        st.serialize_field("Title", &self.title)?;
        st.serialize_field("Start", &iso_time::format_date_time(&self.start))?;
        st.serialize_field("DurationMinutes", &self.duration_minutes)?;
        if numeric_source {
            st.serialize_field("Source", &(self.source as i32))?;
        } else {
            st.serialize_field("Source", self.source.name())?;
        }
        st.serialize_field("End", &iso_time::format_date_time(&self.end()))?;
        st.serialize_field("Date", &self.date().format("%Y-%m-%d").to_string())?;
        st.serialize_field("IsReadOnly", &self.is_read_only())?;
        st.end()
    }
}

impl Serialize for CalendarEvent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.serialize_with(serializer, false)
    }
}

/// Dosyadan okunan ham etkinlik: C# kayıt oluşturucusunun varsayılanlarıyla (eksik alan = varsayılan).
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RawCalendarEvent {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    start: Option<String>,
    #[serde(default)]
    duration_minutes: Option<i32>,
    #[serde(default)]
    source: Option<serde_json::Value>,
}

impl RawCalendarEvent {
    /// `numeric_source`: `Source` yalnız sayı kabul eder (EventStore'un enum dönüştürücüsüz bağlamı); aksi halde
    /// sayı ya da metin (`UseStringEnumConverter`).
    pub(crate) fn into_event(self, numeric_source: bool) -> Result<CalendarEvent, String> {
        let source = match self.source {
            None | Some(serde_json::Value::Null) => CalendarEventSource::Local,
            Some(serde_json::Value::Number(n)) => n.as_i64().and_then(CalendarEventSource::from_number).ok_or("geçersiz Source")?,
            Some(serde_json::Value::String(s)) if !numeric_source => CalendarEventSource::from_name(&s).ok_or("geçersiz Source")?,
            Some(_) => return Err("geçersiz Source".to_string()),
        };
        let start = match self.start {
            None => NaiveDate::from_ymd_opt(1, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap(),
            Some(text) => {
                let parsed = iso_time::parse_iso(&text).ok_or("geçersiz Start")?;
                iso_time::to_json_date_time(&parsed)
            }
        };
        Ok(CalendarEvent {
            id: self.id.unwrap_or_default(),
            title: self.title.unwrap_or_default(),
            start,
            duration_minutes: self.duration_minutes.unwrap_or(0),
            source,
        })
    }
}

impl<'de> Deserialize<'de> for CalendarEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        RawCalendarEvent::deserialize(deserializer)?.into_event(false).map_err(D::Error::custom)
    }
}

/// Bir günün etkinlikleri (hafta görünümünün satırı).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayAgenda {
    pub date: NaiveDate,
    pub is_today: bool,
    pub events: Vec<CalendarEvent>,
}

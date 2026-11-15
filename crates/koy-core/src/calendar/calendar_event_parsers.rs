use std::fmt;

use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde_json::Value;

use super::calendar_event::{CalendarEvent, CalendarEventSource};
use super::calendar_sync_contracts::OAuthTokenSet;
use super::iso_time::{self, ParsedIso};
use crate::clock::TimeZoneInfo;
use crate::text;
use crate::time::{DateTimeOffset, TimeSpan};

/// C#'ta ayrıştırıcıdan çağırana fırlayan istisnalar: `JsonException` (bozuk JSON) ve `InvalidOperationException`
/// (beklenen tipte olmayan değer: nesne olmayan kökte `TryGetProperty`, metin olmayan değerde `GetString`...).
#[derive(Debug)]
pub enum CalendarParseError {
    Json(serde_json::Error),
    InvalidOperation(&'static str),
}

impl fmt::Display for CalendarParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalendarParseError::Json(e) => write!(f, "{e}"),
            CalendarParseError::InvalidOperation(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for CalendarParseError {}

type ParseResult<T> = Result<T, CalendarParseError>;

const NOT_OBJECT: CalendarParseError = CalendarParseError::InvalidOperation("The requested operation requires an element of type 'Object'.");
const NOT_STRING: CalendarParseError = CalendarParseError::InvalidOperation("The requested operation requires an element of type 'String'.");
const NOT_NUMBER: CalendarParseError = CalendarParseError::InvalidOperation("The requested operation requires an element of type 'Number'.");

/// `JsonElement.TryGetProperty`: nesne değilse istisna.
fn prop<'a>(element: &'a Value, name: &str) -> ParseResult<Option<&'a Value>> {
    match element {
        Value::Object(map) => Ok(map.get(name)),
        _ => Err(NOT_OBJECT),
    }
}

/// `JsonElement.GetString()`: metin ya da null; başka tür istisna.
fn get_string(value: &Value) -> ParseResult<Option<&str>> {
    match value {
        Value::String(s) => Ok(Some(s)),
        Value::Null => Ok(None),
        _ => Err(NOT_STRING),
    }
}

fn parse_document(json: &str) -> ParseResult<Value> {
    serde_json::from_str(json).map_err(CalendarParseError::Json)
}

fn is_blank(value: Option<&str>) -> bool {
    text::is_null_or_white_space(value)
}

/// Google Calendar API v3 ve Microsoft Graph yanıtlarını `CalendarEvent` listesine dönüştüren saf ayrıştırıcı.
pub struct CalendarEventParsers;

impl CalendarEventParsers {
    /// `ParseTokenResponse(json, nowUtc, existingRefreshToken = "", accountLabel = "")`.
    pub fn parse_token_response(
        json: &str,
        now_utc: DateTimeOffset,
        existing_refresh_token: &str,
        account_label: &str,
    ) -> ParseResult<Option<OAuthTokenSet>> {
        let root = parse_document(json)?;
        let Some(access_prop) = prop(&root, "access_token")? else {
            return Ok(None);
        };

        let access_token = get_string(access_prop)?;
        if is_blank(access_token) {
            return Ok(None);
        }

        let refresh_token = match prop(&root, "refresh_token")? {
            Some(refresh_prop) => get_string(refresh_prop)?.unwrap_or(existing_refresh_token),
            None => existing_refresh_token,
        };

        let expires_in = match prop(&root, "expires_in")? {
            Some(exp_prop) => match exp_prop {
                // TryGetInt32: tam sayı ve int aralığında değilse false (varsayılan 3600); sayı olmayan değer istisna.
                Value::Number(n) => match n.as_i64().and_then(|v| i32::try_from(v).ok()) {
                    Some(secs) => secs.max(60),
                    None => 3600,
                },
                _ => return Err(NOT_NUMBER),
            },
            None => 3600,
        };

        Ok(Some(OAuthTokenSet::new(
            access_token.unwrap_or_default(),
            refresh_token,
            now_utc + Duration::seconds(i64::from(expires_in)),
            account_label,
        )))
    }

    /// `ParseGoogleEvents(json, localZone = null)`: bölge verilmezse `TimeZoneInfo.Local`.
    pub fn parse_google_events(json: &str, local_zone: Option<&TimeZoneInfo>) -> ParseResult<Vec<CalendarEvent>> {
        if is_blank(Some(json)) {
            return Ok(Vec::new());
        }

        let local = TimeZoneInfo::local();
        let zone = local_zone.unwrap_or(&local);
        let doc = parse_document(json)?;
        let Some(Value::Array(items)) = prop(&doc, "items")? else {
            return Ok(Vec::new());
        };

        let mut list = Vec::new();
        for item in items {
            if let Some(status) = prop(item, "status")? {
                if get_string(status)?.is_some_and(|s| text::eq_ignore_case(s, "cancelled")) {
                    continue;
                }
            }

            let id = match prop(item, "id")? {
                Some(v) => get_string(v)?,
                None => None,
            };
            let summary = match prop(item, "summary")? {
                Some(v) => get_string(v)?,
                None => None,
            };
            if is_blank(id) || is_blank(summary) {
                continue;
            }

            let Some(start_obj) = prop(item, "start")? else {
                continue;
            };

            let Some((start_local, is_all_day)) = Self::try_parse_google_date_time(start_obj, zone)? else {
                continue;
            };

            let mut duration_minutes = 0;
            if !is_all_day {
                if let Some(end_obj) = prop(item, "end")? {
                    if let Some((end_local, _)) = Self::try_parse_google_date_time(end_obj, zone)? {
                        duration_minutes = duration_between(start_local, end_local);
                    }
                }
            }

            list.push(CalendarEvent::with_source(
                format!("google:{}", id.unwrap_or_default()),
                summary.unwrap_or_default().trim(),
                start_local,
                duration_minutes,
                CalendarEventSource::Google,
            ));
        }

        Ok(list)
    }

    /// `ParseOutlookEvents(json, localZone = null)`: bölge verilmezse `TimeZoneInfo.Local`.
    pub fn parse_outlook_events(json: &str, local_zone: Option<&TimeZoneInfo>) -> ParseResult<Vec<CalendarEvent>> {
        if is_blank(Some(json)) {
            return Ok(Vec::new());
        }

        let local = TimeZoneInfo::local();
        let zone = local_zone.unwrap_or(&local);
        let doc = parse_document(json)?;
        let Some(Value::Array(items)) = prop(&doc, "value")? else {
            return Ok(Vec::new());
        };

        let mut list = Vec::new();
        for item in items {
            if matches!(prop(item, "isCancelled")?, Some(Value::Bool(true))) {
                continue;
            }

            let id = match prop(item, "id")? {
                Some(v) => get_string(v)?,
                None => None,
            };
            let subject = match prop(item, "subject")? {
                Some(v) => get_string(v)?,
                None => None,
            };
            if is_blank(id) || is_blank(subject) {
                continue;
            }

            let is_all_day = matches!(prop(item, "isAllDay")?, Some(Value::Bool(true)));
            let Some(start_obj) = prop(item, "start")? else {
                continue;
            };
            let Some(start_local) = Self::try_parse_graph_date_time(start_obj, zone, is_all_day)? else {
                continue;
            };

            let mut duration_minutes = 0;
            if !is_all_day {
                if let Some(end_obj) = prop(item, "end")? {
                    if let Some(end_local) = Self::try_parse_graph_date_time(end_obj, zone, false)? {
                        duration_minutes = duration_between(start_local, end_local);
                    }
                }
            }

            list.push(CalendarEvent::with_source(
                format!("outlook:{}", id.unwrap_or_default()),
                subject.unwrap_or_default().trim(),
                start_local,
                duration_minutes,
                CalendarEventSource::Outlook,
            ));
        }

        Ok(list)
    }

    /// Dönen çift: (yerel saat, tüm gün mü). Ayrıştırılamıyorsa `None`.
    fn try_parse_google_date_time(obj: &Value, local_zone: &TimeZoneInfo) -> ParseResult<Option<(NaiveDateTime, bool)>> {
        if let Some(dt_prop) = prop(obj, "dateTime")? {
            if let Some(dto) = get_string(dt_prop)?.and_then(try_parse_date_time_offset) {
                return Ok(Some((TimeZoneInfo::convert_time(dto, local_zone).naive_local(), false)));
            }
        }

        if let Some(d_prop) = prop(obj, "date")? {
            if let Some(date) = get_string(d_prop)?.and_then(try_parse_exact_date) {
                return Ok(Some((date.and_hms_opt(0, 0, 0).unwrap(), true)));
            }
        }

        Ok(None)
    }

    fn try_parse_graph_date_time(obj: &Value, local_zone: &TimeZoneInfo, is_all_day: bool) -> ParseResult<Option<NaiveDateTime>> {
        let Some(dt_prop) = prop(obj, "dateTime")? else {
            return Ok(None);
        };

        let raw = get_string(dt_prop)?;
        if is_blank(raw) {
            return Ok(None);
        }
        let raw = raw.unwrap_or_default();

        // DateTime.TryParse(raw, Invariant, None): saat farkı varsa yerel saate çevrilir.
        let Some(parsed_iso) = iso_time::parse_iso(raw) else {
            return Ok(None);
        };
        let parsed = iso_time::to_local_naive(&parsed_iso);

        if is_all_day {
            return Ok(Some(parsed.date().and_hms_opt(0, 0, 0).unwrap()));
        }

        let tz_name = match prop(obj, "timeZone")? {
            Some(v) => get_string(v)?,
            None => None,
        };
        if tz_name.is_some_and(|t| text::eq_ignore_case(t, "UTC")) {
            // SpecifyKind(parsed, Utc): değer olduğu gibi UTC duvar saati sayılır.
            return Ok(Some(TimeZoneInfo::convert_time_from_utc(parsed, local_zone)));
        }

        let has_offset_marker = raw.ends_with('Z') || raw.contains('+') || (raw.chars().count() > 10 && raw.chars().skip(10).any(|c| c == '-'));
        if has_offset_marker {
            if let Some(dto) = try_parse_date_time_offset(raw) {
                return Ok(Some(TimeZoneInfo::convert_time(dto, local_zone).naive_local()));
            }
        }

        // DateTime.SpecifyKind(parsed, Unspecified): değer değişmez.
        Ok(Some(parsed))
    }
}

/// `(int)Math.Round((end - start).TotalMinutes)` 0 - 24 saate sıkıştırılmış.
fn duration_between(start: NaiveDateTime, end: NaiveDateTime) -> i32 {
    let minutes = TimeSpan::from_chrono(end - start).total_minutes();
    crate::text::clamp(text::round(minutes) as i32, 0, 24 * 60)
}

/// `DateTimeOffset.TryParse(s, Invariant, RoundtripKind)` (yalnız ISO 8601 biçimleri).
fn try_parse_date_time_offset(text: &str) -> Option<DateTimeOffset> {
    let parsed: ParsedIso = iso_time::parse_iso(text)?;
    Some(iso_time::to_date_time_offset(&parsed))
}

/// `DateOnly.TryParseExact(s, "yyyy-MM-dd", Invariant, None)`.
fn try_parse_exact_date(text: &str) -> Option<NaiveDate> {
    let b = text.as_bytes();
    if !text.is_ascii() || b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let digits = |s: &str| if s.bytes().all(|c| c.is_ascii_digit()) { s.parse::<u32>().ok() } else { None };
    NaiveDate::from_ymd_opt(digits(&text[0..4])? as i32, digits(&text[5..7])?, digits(&text[8..10])?)
}

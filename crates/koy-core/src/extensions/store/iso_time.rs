//! ISO 8601 tarih/saat metni: indeksin `generated` alanı (`DateTimeOffset.TryParse`), kayıt dosyasının `installedAt`
//! alanı (System.Text.Json `DateTimeOffset`) ve indeksin `updated` alanı (`DateOnly.TryParseExact`).
//!
//! .NET'in `DateTimeOffset.TryParse` kadar gevşek değildir: yalnız `yyyy-MM-dd[Thh:mm[:ss[.fffffff]]][Z|±hh[:mm]]`
//! biçimi bilinir (mağaza betiği ve kayıt dosyası başka biçim üretmez). Biçim `calendar` modülündeki ISO ayrıştırıcıyla
//! aynıdır; o modülün yardımcıları özel olduğu için burada ayrıca (küçültülmüş) bulunur.

use chrono::{Datelike, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Timelike};

use crate::time::DateTimeOffset;

/// Ayrıştırılmış metin. `offset_seconds` yoksa metinde saat farkı yoktu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParsedIso {
    pub naive: NaiveDateTime,
    pub offset_seconds: Option<i32>,
}

pub(crate) fn parse_iso(text: &str) -> Option<ParsedIso> {
    let s = text.trim();
    if !s.is_ascii() || s.len() < 10 {
        return None;
    }
    let b = s.as_bytes();
    if b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let year: i32 = digits(&s[0..4])?;
    let month: u32 = digits(&s[5..7])?;
    let day: u32 = digits(&s[8..10])?;
    if year < 1 {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    if s.len() == 10 {
        return Some(ParsedIso { naive: date.and_hms_opt(0, 0, 0)?, offset_seconds: None });
    }
    if b[10] != b'T' && b[10] != b' ' {
        return None;
    }

    let rest = &s[11..];
    let rb = rest.as_bytes();
    if rb.len() < 5 || rb[2] != b':' {
        return None;
    }
    let hour: u32 = digits(&rest[0..2])?;
    let minute: u32 = digits(&rest[3..5])?;
    let mut pos = 5;
    let mut second = 0;
    let mut nanos = 0u32;
    if rb.len() > pos && rb[pos] == b':' {
        if rb.len() < pos + 3 {
            return None;
        }
        second = digits(&rest[pos + 1..pos + 3])?;
        pos += 3;
        if rb.len() > pos && rb[pos] == b'.' {
            let start = pos + 1;
            let mut end = start;
            while end < rb.len() && rb[end].is_ascii_digit() {
                end += 1;
            }
            if end == start {
                return None;
            }
            let mut frac = rest[start..end].to_string();
            frac.truncate(9);
            while frac.len() < 9 {
                frac.push('0');
            }
            nanos = frac.parse().ok()?;
            pos = end;
        }
    }
    let time = NaiveTime::from_hms_nano_opt(hour, minute, second, nanos)?;

    let zone = &rest[pos..];
    let offset_seconds = if zone.is_empty() {
        None
    } else if zone == "Z" || zone == "z" {
        Some(0)
    } else {
        let sign = match zone.as_bytes()[0] {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        let body = &zone[1..];
        let (h, m): (i32, i32) = match body.len() {
            2 => (digits(body)?, 0),
            4 => (digits(&body[0..2])?, digits(&body[2..4])?),
            5 if body.as_bytes()[2] == b':' => (digits(&body[0..2])?, digits(&body[3..5])?),
            _ => return None,
        };
        if h > 14 || m > 59 {
            return None;
        }
        Some(sign * (h * 3600 + m * 60))
    };

    Some(ParsedIso { naive: date.and_time(time), offset_seconds })
}

fn digits<T: std::str::FromStr>(s: &str) -> Option<T> {
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// `DateTimeOffset.TryParse(text, InvariantCulture, DateTimeStyles.AssumeUniversal)`: saat farkı yoksa UTC.
pub(crate) fn parse_date_time_offset_assume_universal(text: &str) -> Option<DateTimeOffset> {
    let parsed = parse_iso(text)?;
    let offset = FixedOffset::east_opt(parsed.offset_seconds.unwrap_or(0))?;
    offset.from_local_datetime(&parsed.naive).single()
}

/// System.Text.Json `DateTimeOffset` okuması: saat farkı yoksa makinenin yerel farkı.
pub(crate) fn parse_json_date_time_offset(text: &str) -> Option<DateTimeOffset> {
    let parsed = parse_iso(text)?;
    let offset = match parsed.offset_seconds {
        Some(secs) => FixedOffset::east_opt(secs)?,
        None => match Local.from_local_datetime(&parsed.naive).earliest() {
            Some(local) => local.offset().fix(),
            None => Local.offset_from_utc_datetime(&parsed.naive).fix(),
        },
    };
    offset.from_local_datetime(&parsed.naive).single()
}

/// System.Text.Json `DateTimeOffset` yazımı: `2026-11-06T09:00:00[.fffffff]+03:00`.
pub(crate) fn format_json_date_time_offset(dt: &DateTimeOffset) -> String {
    let local = dt.naive_local();
    let mut out = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        local.year(),
        local.month(),
        local.day(),
        local.hour(),
        local.minute(),
        local.second()
    );
    let ticks = (local.nanosecond() % 1_000_000_000) / 100;
    if ticks > 0 {
        out.push('.');
        out.push_str(format!("{ticks:07}").trim_end_matches('0'));
    }
    let secs = dt.offset().local_minus_utc();
    let sign = if secs < 0 { '-' } else { '+' };
    let abs = secs.abs();
    out.push_str(&format!("{sign}{:02}:{:02}", abs / 3600, abs / 60 % 60));
    out
}

/// `DateOnly.TryParseExact(text, "yyyy-MM-dd", InvariantCulture, DateTimeStyles.None)`: tam on karakter.
pub(crate) fn parse_date_only_exact(text: &str) -> Option<NaiveDate> {
    let b = text.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let year: i32 = digits(&text[0..4])?;
    let month: u32 = digits(&text[5..7])?;
    let day: u32 = digits(&text[8..10])?;
    if year < 1 {
        return None;
    }
    NaiveDate::from_ymd_opt(year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_generated_assumes_utc() {
        let z = parse_date_time_offset_assume_universal("2026-11-12T00:00:00Z").unwrap();
        let none = parse_date_time_offset_assume_universal("2026-11-12").unwrap();
        assert_eq!(z, none);
        assert_eq!(0, z.offset().local_minus_utc());
        assert!(parse_date_time_offset_assume_universal("yarin").is_none());
    }

    #[test]
    fn json_round_trip_keeps_offset_and_ticks() {
        let at = FixedOffset::east_opt(3 * 3600).unwrap().with_ymd_and_hms(2026, 11, 12, 10, 30, 0).unwrap();
        let text = format_json_date_time_offset(&at);
        assert_eq!("2026-11-12T10:30:00+03:00", text);
        assert_eq!(at, parse_json_date_time_offset(&text).unwrap());

        let with_ticks = parse_json_date_time_offset("2026-11-12T10:30:00.1234567+00:00").unwrap();
        assert_eq!("2026-11-12T10:30:00.1234567+00:00", format_json_date_time_offset(&with_ticks));
        let trimmed = parse_json_date_time_offset("2026-11-12T10:30:00.5Z").unwrap();
        assert_eq!("2026-11-12T10:30:00.5+00:00", format_json_date_time_offset(&trimmed));
    }

    #[test]
    fn date_only_is_strict() {
        assert!(parse_date_only_exact("2026-11-12").is_some());
        for bad in ["12.11.2026", "2026-1-5", "2026-13-01", "2026-02-30", "0000-01-01", "2026-11-12T00:00", " 2026-11-12"] {
            assert!(parse_date_only_exact(bad).is_none(), "{bad}");
        }
    }
}

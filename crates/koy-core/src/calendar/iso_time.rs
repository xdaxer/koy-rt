//! ISO 8601 tarih/saat metni: System.Text.Json'ın `DateTime`/`DateTimeOffset` biçimi ve Google/Graph yanıtlarındaki
//! tarih metinleri için ayrıştırma ve yazma. .NET'in `DateTime.TryParse` kadar gevşek değildir: yalnız
//! `yyyy-MM-dd[Thh:mm[:ss[.fffffff]]][Z|±hh[:mm]]` biçimi bilinir (API'ler başka biçim vermez).

use chrono::{Datelike, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Timelike};

use crate::time::DateTimeOffset;

/// Ayrıştırılmış metin. `offset_seconds` yoksa metinde saat farkı yoktu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParsedIso {
    pub naive: NaiveDateTime,
    pub offset_seconds: Option<i32>,
    /// Metin "Z" ile bitiyordu (System.Text.Json bunu `Kind=Utc` okur ve değeri çevirmez).
    pub zulu: bool,
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
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    if s.len() == 10 {
        return Some(ParsedIso { naive: date.and_hms_opt(0, 0, 0)?, offset_seconds: None, zulu: false });
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

    Some(ParsedIso { naive: date.and_time(time), offset_seconds, zulu: zone.eq_ignore_ascii_case("Z") })
}

fn digits<T: std::str::FromStr>(s: &str) -> Option<T> {
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Yerel saat dilimine çevrilmiş duvar saati (`DateTime.TryParse`'ın saat farkı olan metinde yaptığı gibi).
pub(crate) fn to_local_naive(p: &ParsedIso) -> NaiveDateTime {
    match p.offset_seconds {
        Some(secs) => {
            let fixed = FixedOffset::east_opt(secs).expect("geçerli saat farkı");
            fixed.from_local_datetime(&p.naive).unwrap().with_timezone(&Local).naive_local()
        }
        None => p.naive,
    }
}

/// System.Text.Json `DateTime` okuması: "Z" değeri olduğu gibi bırakır, başka saat farkını yerel saate çevirir.
pub(crate) fn to_json_date_time(p: &ParsedIso) -> NaiveDateTime {
    if p.zulu { p.naive } else { to_local_naive(p) }
}

/// `DateTimeOffset.TryParse`: saat farkı yoksa makinenin yerel farkı kullanılır.
pub(crate) fn to_date_time_offset(p: &ParsedIso) -> DateTimeOffset {
    let offset = match p.offset_seconds {
        Some(secs) => FixedOffset::east_opt(secs).expect("geçerli saat farkı"),
        None => local_offset_for(p.naive),
    };
    offset.from_local_datetime(&p.naive).unwrap()
}

fn local_offset_for(naive: NaiveDateTime) -> FixedOffset {
    match Local.from_local_datetime(&naive).earliest() {
        Some(local) => local.offset().fix(),
        None => Local.offset_from_utc_datetime(&naive).fix(),
    }
}

/// System.Text.Json `DateTime` (Kind=Unspecified) yazımı: `2026-11-06T09:00:00[.fffffff]`.
pub(crate) fn format_date_time(dt: &NaiveDateTime) -> String {
    let mut out = format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", dt.year(), dt.month(), dt.day(), dt.hour(), dt.minute(), dt.second());
    push_fraction(&mut out, dt.nanosecond());
    out
}

/// System.Text.Json `DateTimeOffset` yazımı: `2026-11-06T09:00:00[.fffffff]+03:00`.
pub(crate) fn format_date_time_offset(dt: &DateTimeOffset) -> String {
    let mut out = format_date_time(&dt.naive_local());
    let secs = dt.offset().local_minus_utc();
    let sign = if secs < 0 { '-' } else { '+' };
    let abs = secs.abs();
    out.push_str(&format!("{sign}{:02}:{:02}", abs / 3600, abs / 60 % 60));
    out
}

fn push_fraction(out: &mut String, nanos: u32) {
    let ticks = (nanos % 1_000_000_000) / 100;
    if ticks > 0 {
        let frac = format!("{ticks:07}");
        out.push('.');
        out.push_str(frac.trim_end_matches('0'));
    }
}

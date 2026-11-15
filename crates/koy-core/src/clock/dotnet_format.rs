//! .NET özel tarih/saat biçimi (`DateTime.ToString(format, culture)` / `DateTimeOffset.ToString(format, culture)`).
//!
//! Saat ve tarih desenleri ("HH:mm", "h:mm tt", "ddd, d MMMM") Core'da .NET desen metni olarak taşınır
//! (`ClockFormatPolicy`); metni biçimlendirmek için aynı dilbilgisi gerekir. Geçersiz desen `FormatException`
//! olur ([`FormatException`]); `ClockFormatPolicy.IsValidCustomDateFormat` bunu yakalayarak çalışır.

use std::fmt;

use chrono::{Datelike, Local, NaiveDateTime, Offset, TimeZone, Timelike};

use super::culture::Culture;
use super::day_of_week::DayOfWeek;
use crate::time::TimeSpan;

/// C# `FormatException`: desen ya da biçim belirtici geçersiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatException;

impl fmt::Display for FormatException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Input string was not in a correct format.")
    }
}

impl std::error::Error for FormatException {}

/// `ToString(format, culture)`. `offset` verilirse `DateTimeOffset` gibi o farkla, `None` ise `Kind=Unspecified`
/// bir `DateTime` gibi biçimlenir (`K` boş çıkar, `z` makinenin yerel farkını kullanır).
/// Tek karakterlik desen standart biçim belirticidir ("d", "t", "o" ...).
pub fn format_date_time(dt: NaiveDateTime, offset: Option<TimeSpan>, format: &str, culture: &Culture) -> Result<String, FormatException> {
    let chars: Vec<char> = format.chars().collect();
    if chars.len() == 1 {
        return format_standard(dt, offset, chars[0], culture);
    }
    format_custom(dt, offset, &chars, culture)
}

fn format_standard(dt: NaiveDateTime, offset: Option<TimeSpan>, spec: char, culture: &Culture) -> Result<String, FormatException> {
    // Standart belirticiler desen metnine çevrilip özel biçimleyiciden geçer.
    let pattern: String = match spec {
        'd' => culture.short_date_pattern.to_string(),
        'D' => culture.long_date_pattern.to_string(),
        'f' => format!("{} {}", culture.long_date_pattern, culture.short_time_pattern),
        'F' => format!("{} {}", culture.long_date_pattern, culture.long_time_pattern),
        'g' => format!("{} {}", culture.short_date_pattern, culture.short_time_pattern),
        'G' => format!("{} {}", culture.short_date_pattern, culture.long_time_pattern),
        'm' | 'M' => culture.month_day_pattern.to_string(),
        'y' | 'Y' => culture.year_month_pattern.to_string(),
        't' => culture.short_time_pattern.to_string(),
        'T' => culture.long_time_pattern.to_string(),
        's' => "yyyy'-'MM'-'dd'T'HH':'mm':'ss".to_string(),
        'o' | 'O' => "yyyy'-'MM'-'dd'T'HH':'mm':'ss.fffffffK".to_string(),
        'r' | 'R' => {
            return format_custom(to_utc(dt, offset), Some(TimeSpan::ZERO), &"ddd, dd MMM yyyy HH':'mm':'ss 'GMT'".chars().collect::<Vec<_>>(), &Culture::invariant());
        }
        'u' => {
            return format_custom(to_utc(dt, offset), Some(TimeSpan::ZERO), &"yyyy'-'MM'-'dd HH':'mm':'ss'Z'".chars().collect::<Vec<_>>(), &Culture::invariant());
        }
        'U' => {
            let pattern: Vec<char> = format!("{} {}", culture.long_date_pattern, culture.long_time_pattern).chars().collect();
            return format_custom(to_utc(dt, offset), Some(TimeSpan::ZERO), &pattern, culture);
        }
        _ => return Err(FormatException),
    };
    let pattern: Vec<char> = pattern.chars().collect();
    format_custom(dt, offset, &pattern, culture)
}

fn to_utc(dt: NaiveDateTime, offset: Option<TimeSpan>) -> NaiveDateTime {
    match offset {
        Some(o) => dt - o.to_chrono(),
        None => {
            let local = Local.from_local_datetime(&dt).earliest();
            local.map_or(dt, |l| l.naive_utc())
        }
    }
}

/// Özel desen ayrıştırıcısı (`DateTimeFormat.FormatCustomized`).
fn format_custom(dt: NaiveDateTime, offset: Option<TimeSpan>, format: &[char], culture: &Culture) -> Result<String, FormatException> {
    let mut out = String::new();
    let mut i = 0;
    while i < format.len() {
        let ch = format[i];
        let mut token_len = 1;
        match ch {
            'g' => {
                token_len = repeat_len(format, i, ch);
                out.push_str(culture.era_name);
            }
            'h' => {
                token_len = repeat_len(format, i, ch);
                let mut hour = dt.hour() % 12;
                if hour == 0 {
                    hour = 12;
                }
                push_digits(&mut out, i64::from(hour), token_len, false);
            }
            'H' => {
                token_len = repeat_len(format, i, ch);
                push_digits(&mut out, i64::from(dt.hour()), token_len, false);
            }
            'm' => {
                token_len = repeat_len(format, i, ch);
                push_digits(&mut out, i64::from(dt.minute()), token_len, false);
            }
            's' => {
                token_len = repeat_len(format, i, ch);
                push_digits(&mut out, i64::from(dt.second()), token_len, false);
            }
            'f' | 'F' => {
                token_len = repeat_len(format, i, ch);
                if token_len > 7 {
                    return Err(FormatException);
                }
                let ticks = i64::from(dt.nanosecond() % 1_000_000_000) / 100;
                let fraction = ticks / 10_i64.pow(7 - token_len as u32);
                if ch == 'f' {
                    out.push_str(&format!("{fraction:0width$}", width = token_len));
                } else {
                    let digits = format!("{fraction:0width$}", width = token_len);
                    let trimmed = digits.trim_end_matches('0');
                    if trimmed.is_empty() {
                        // Hiç anlamlı kesir yoksa kendinden önceki '.' da düşer.
                        if out.ends_with('.') {
                            out.pop();
                        }
                    } else {
                        out.push_str(trimmed);
                    }
                }
            }
            't' => {
                token_len = repeat_len(format, i, ch);
                let designator = if dt.hour() < 12 { culture.am_designator } else { culture.pm_designator };
                if token_len == 1 {
                    if let Some(first) = designator.chars().next() {
                        out.push(first);
                    }
                } else {
                    out.push_str(designator);
                }
            }
            'd' => {
                token_len = repeat_len(format, i, ch);
                if token_len <= 2 {
                    push_digits(&mut out, i64::from(dt.day()), token_len, false);
                } else {
                    let day = DayOfWeek::of(dt.date()).index() as usize;
                    out.push_str(if token_len == 3 { culture.abbreviated_day_names[day] } else { culture.day_names[day] });
                }
            }
            'M' => {
                token_len = repeat_len(format, i, ch);
                let month = dt.month() as usize;
                if token_len <= 2 {
                    push_digits(&mut out, month as i64, token_len, false);
                } else if token_len == 3 {
                    out.push_str(culture.abbreviated_month_names[month - 1]);
                } else {
                    out.push_str(culture.month_names[month - 1]);
                }
            }
            'y' => {
                token_len = repeat_len(format, i, ch);
                let year = i64::from(dt.year());
                if token_len <= 2 {
                    push_digits(&mut out, year % 100, token_len, false);
                } else {
                    push_digits(&mut out, year, token_len, true);
                }
            }
            'z' => {
                token_len = repeat_len(format, i, ch);
                let total = zone_offset(dt, offset);
                let sign = if total < TimeSpan::ZERO { '-' } else { '+' };
                let abs = total.duration();
                out.push(sign);
                let hours = i64::from(abs.hours()) + i64::from(abs.days()) * 24;
                if token_len == 1 {
                    out.push_str(&hours.to_string());
                } else if token_len == 2 {
                    push_digits(&mut out, hours, 2, false);
                } else {
                    push_digits(&mut out, hours, 2, false);
                    out.push(':');
                    push_digits(&mut out, i64::from(abs.minutes()), 2, false);
                }
            }
            'K' => {
                // Kind=Unspecified DateTime için boş; DateTimeOffset için "+hh:mm".
                if let Some(o) = offset {
                    let sign = if o < TimeSpan::ZERO { '-' } else { '+' };
                    let abs = o.duration();
                    out.push(sign);
                    push_digits(&mut out, i64::from(abs.hours()), 2, false);
                    out.push(':');
                    push_digits(&mut out, i64::from(abs.minutes()), 2, false);
                }
            }
            ':' => out.push_str(culture.time_separator),
            '/' => out.push_str(culture.date_separator),
            '\'' | '"' => {
                // Tırnaklı metin: kapanış tırnağı yoksa FormatException.
                let mut j = i + 1;
                let mut literal = String::new();
                let mut closed = false;
                while j < format.len() {
                    let c = format[j];
                    j += 1;
                    if c == ch {
                        closed = true;
                        break;
                    }
                    if c == '\\' {
                        // Tırnak içinde ters eğik çizgi sonraki karakteri kaçırır; sonda yalnızsa hata.
                        if j < format.len() {
                            literal.push(format[j]);
                            j += 1;
                        } else {
                            return Err(FormatException);
                        }
                    } else {
                        literal.push(c);
                    }
                }
                if !closed {
                    return Err(FormatException);
                }
                out.push_str(&literal);
                token_len = j - i;
            }
            '%' => {
                // "%c": tek karakterlik özel biçim. Sonda ya da "%%" ise hata.
                match format.get(i + 1) {
                    Some(&next) if next != '%' => {
                        out.push_str(&format_custom(dt, offset, &[next], culture)?);
                        token_len = 2;
                    }
                    _ => return Err(FormatException),
                }
            }
            '\\' => match format.get(i + 1) {
                Some(&next) => {
                    out.push(next);
                    token_len = 2;
                }
                None => return Err(FormatException),
            },
            other => out.push(other),
        }
        i += token_len;
    }
    Ok(out)
}

fn repeat_len(format: &[char], pos: usize, ch: char) -> usize {
    format[pos..].iter().take_while(|&&c| c == ch).count()
}

/// `FormatDigits`: en az `len` basamak; `override_limit` yoksa en çok 2 basamağa kadar sıfır doldurur.
fn push_digits(out: &mut String, value: i64, len: usize, override_limit: bool) {
    let len = if !override_limit && len > 2 { 2 } else { len };
    out.push_str(&format!("{value:0len$}"));
}

fn zone_offset(dt: NaiveDateTime, offset: Option<TimeSpan>) -> TimeSpan {
    match offset {
        Some(o) => o,
        None => match Local.from_local_datetime(&dt).earliest() {
            Some(local) => TimeSpan::from_secs(i64::from(local.offset().fix().local_minus_utc())),
            None => TimeSpan::ZERO,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn sample() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, 5).unwrap().and_hms_opt(13, 7, 9).unwrap()
    }

    fn fmt(pattern: &str) -> Result<String, FormatException> {
        format_date_time(sample(), None, pattern, &Culture::invariant())
    }

    #[test]
    fn custom_patterns() {
        assert_eq!(fmt("HH:mm:ss").unwrap(), "13:07:09");
        assert_eq!(fmt("h:mm tt").unwrap(), "1:07 PM");
        assert_eq!(fmt("dd.MM.yyyy").unwrap(), "05.01.2026");
        assert_eq!(fmt("ddd, d MMMM").unwrap(), "Mon, 5 January");
        assert_eq!(fmt("yy-M-d 'at' H").unwrap(), "26-1-5 at 13");
        assert_eq!(fmt("%d").unwrap(), "5");
        assert_eq!(fmt("d\\d").unwrap(), "5d");
        assert_eq!(fmt("fff").unwrap(), "000");
        assert_eq!(fmt("HH:mm:ss.FFF").unwrap(), "13:07:09");
    }

    #[test]
    fn standard_specifiers() {
        assert_eq!(fmt("d").unwrap(), "01/05/2026");
        assert_eq!(fmt("s").unwrap(), "2026-01-05T13:07:09");
        assert_eq!(fmt("t").unwrap(), "13:07");
        assert_eq!(format_date_time(sample(), None, "d", &Culture::tr_tr()).unwrap(), "5.01.2026");
    }

    #[test]
    fn bad_patterns_throw() {
        assert_eq!(fmt("\\"), Err(FormatException));
        assert_eq!(fmt("%"), Err(FormatException));
        assert_eq!(fmt("a"), Err(FormatException));
        assert_eq!(fmt("'abc"), Err(FormatException));
        assert_eq!(fmt("ffffffff"), Err(FormatException));
        assert_eq!(fmt("dd%"), Err(FormatException));
    }

    #[test]
    fn offset_is_formatted_for_date_time_offset() {
        let plus3 = Some(TimeSpan::from_hours(3));
        assert_eq!(format_date_time(sample(), plus3, "zzz", &Culture::invariant()).unwrap(), "+03:00");
        assert_eq!(format_date_time(sample(), plus3, "%z", &Culture::invariant()).unwrap(), "+3");
        assert_eq!(format_date_time(sample(), plus3, "o", &Culture::invariant()).unwrap(), "2026-01-05T13:07:09.0000000+03:00");
    }
}

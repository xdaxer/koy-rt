use std::cmp::Ordering;

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};

use super::calendar_event::{CalendarEvent, DayAgenda};
use super::week_grid::WeekGrid;
use crate::clock::DayOfWeek;
use crate::text;
use crate::time::TimeSpan;

/// Etkinlik listesinden gün, hafta ve "sıradaki" görünümlerini üretir. Saf mantık: saat ve liste dışarıdan gelir.
/// Hafta Pazartesi başlar (ay ızgarasıyla aynı).
pub struct EventAgenda;

/// `StringComparer.CurrentCultureIgnoreCase` yerine `OrdinalIgnoreCase` sıralaması (C# ile aynı değil: kültüre duyarlı
/// sıralama ICU/NLS verisine bağlı; Core'da kültür verisi yok). ASCII ve Türkçe başlıklar için sıra aynı çıkar.
fn compare_titles(a: &str, b: &str) -> Ordering {
    text::cmp_ignore_case(a, b)
}

impl EventAgenda {
    pub const MAX_TITLE_LENGTH: usize = 60;
    pub const MAX_DURATION_MINUTES: i32 = 24 * 60;

    /// Verilen günün etkinlikleri, başlangıç saatine göre.
    pub fn for_day(events: &[CalendarEvent], day: NaiveDate) -> Vec<CalendarEvent> {
        let mut list: Vec<CalendarEvent> = events.iter().filter(|e| e.date() == day).cloned().collect();
        // LINQ OrderBy/ThenBy kararlıdır.
        list.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| compare_titles(&a.title, &b.title)));
        list
    }

    /// Günün ait olduğu haftanın ilk günü (Pazartesi).
    pub fn week_start(day: NaiveDate) -> NaiveDate {
        Self::week_start_with_first_day(day, DayOfWeek::Monday)
    }

    pub fn week_start_with_first_day(day: NaiveDate, first_day: DayOfWeek) -> NaiveDate {
        WeekGrid::start(day, first_day)
    }

    /// Günün haftasındaki 7 gün (Pazartesi ile başlar) ve her günün etkinlikleri.
    pub fn for_week(events: &[CalendarEvent], day: NaiveDate, today: NaiveDate) -> Vec<DayAgenda> {
        Self::for_week_with_first_day(events, day, today, DayOfWeek::Monday)
    }

    /// Günün haftasındaki 7 gün (`first_day` ile başlar) ve her günün etkinlikleri.
    pub fn for_week_with_first_day(events: &[CalendarEvent], day: NaiveDate, today: NaiveDate, first_day: DayOfWeek) -> Vec<DayAgenda> {
        WeekGrid::days(day, first_day)
            .into_iter()
            .map(|date| DayAgenda { date, is_today: date == today, events: Self::for_day(events, date) })
            .collect()
    }

    /// Bugünün sıradaki etkinliği: şu an sürenler ve henüz başlamamış olanlar arasında en erken başlayan.
    /// Bitmiş etkinlikler ve başka günler sayılmaz; yoksa None.
    pub fn next_today(events: &[CalendarEvent], now: NaiveDateTime) -> Option<CalendarEvent> {
        let today = now.date();
        // OrderBy(Start).FirstOrDefault(): eşit başlangıçta listedeki ilki (`min_by_key` de ilkini verir).
        events
            .iter()
            .filter(|e| e.date() == today && if e.duration_minutes == 0 { e.start >= now } else { e.end() > now })
            .min_by_key(|e| e.start)
            .cloned()
    }

    /// Ana sayfanın "sıradaki" listesi: şu an süren ya da önümüzdeki `days` gün içinde başlayacak
    /// etkinlikler, başlangıca göre, en çok `count` tane. Bitmiş etkinlikler sayılmaz; süresiz (an)
    /// etkinlik başlangıcı geçince düşer. `days` varsayılanı 14: [`EventAgenda::upcoming_default`].
    pub fn upcoming(events: &[CalendarEvent], now: NaiveDateTime, count: i32, days: i32) -> Vec<CalendarEvent> {
        if count <= 0 {
            return Vec::new();
        }

        let horizon = now.date().and_time(NaiveTime::MIN) + Duration::days(i64::from(days.max(1)));
        let mut list: Vec<CalendarEvent> = events
            .iter()
            .filter(|e| (if e.duration_minutes == 0 { e.start >= now } else { e.end() > now }) && e.start < horizon)
            .cloned()
            .collect();
        list.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| compare_titles(&a.title, &b.title)));
        list.truncate(count as usize);
        list
    }

    /// `Upcoming(events, now, count)`: `days` = 14.
    pub fn upcoming_default(events: &[CalendarEvent], now: NaiveDateTime, count: i32) -> Vec<CalendarEvent> {
        Self::upcoming(events, now, count, 14)
    }

    /// Etkinliğe kalan süre (başlamışsa sıfır); "10 sa 13 dk sonra" metni App'te kurulur.
    pub fn until(e: &CalendarEvent, now: NaiveDateTime) -> TimeSpan {
        if e.start > now { TimeSpan::from_chrono(e.start - now) } else { TimeSpan::ZERO }
    }

    /// Başlığı kırpar ve uzunluğu sınırlar; boşsa None (etkinlik eklenemez).
    pub fn normalize_title(title: Option<&str>) -> Option<String> {
        let trimmed = title?.trim();
        if trimmed.is_empty() {
            return None;
        }

        // C# `trimmed[..60]` UTF-16 birimi sayar; Rust'ta aynı sınırı UTF-16 uzunluğuyla uygula (sarmal çiftini bölmez).
        let mut units = 0;
        let mut cut = trimmed.len();
        for (index, ch) in trimmed.char_indices() {
            units += ch.len_utf16();
            if units > Self::MAX_TITLE_LENGTH {
                cut = index;
                break;
            }
        }
        if cut == trimmed.len() {
            Some(trimmed.to_string())
        } else {
            Some(trimmed[..cut].trim_end().to_string())
        }
    }

    /// "14:30", "9:05", "1430" gibi girdiyi saate çevirir; geçersizse None (C#'taki `bool TryParseTime(text, out time)`).
    pub fn try_parse_time(text: Option<&str>) -> Option<NaiveTime> {
        let value = text?.trim().replace('.', ":");
        if value.is_empty() {
            return None;
        }

        // C# `char.IsDigit` Unicode rakamlarını da sayar ama sonraki int.TryParse yalnız ASCII kabul eder; sonuç aynı.
        let value = if !value.contains(':') && value.chars().all(|c| c.is_ascii_digit()) && matches!(value.len(), 3 | 4) {
            let (head, tail) = value.split_at(value.len() - 2);
            format!("{head}:{tail}")
        } else {
            value
        };

        let parts: Vec<&str> = value.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        // NumberStyles.None: işaret, boşluk, ondalık yok; yalnız rakamlar.
        let parse = |s: &str| -> Option<i32> {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) { None } else { s.parse().ok() }
        };
        let hour = parse(parts[0])?;
        let minute = parse(parts[1])?;
        if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || parts[1].len() != 2 {
            return None;
        }

        NaiveTime::from_hms_opt(hour as u32, minute as u32, 0)
    }

    pub fn clamp_duration(minutes: i32) -> i32 {
        text::clamp(minutes, 0, Self::MAX_DURATION_MINUTES)
    }
}

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use super::clock_enums::{ClockHourFormat, WeekStartSetting};
use super::culture::Culture;
use super::day_of_week::DayOfWeek;
use super::dotnet_format::format_date_time;

/// Saat ve tarih biçim kararları. Saf: kültür ve tarih dışarıdan gelir. Core yalnız .NET biçim desenini ve sayıyı
/// döndürür; metni App biçimlendirir (AGENTS: Core değer döndürür, App biçimlendirir).
pub struct ClockFormatPolicy;

impl ClockFormatPolicy {
    pub const MAX_CUSTOM_DATE_FORMAT_LENGTH: usize = 64;

    /// 12 saatlik mi: Hour12/Hour24 zorlar, Auto kültüre bakar.
    pub fn uses_twelve_hour_clock(setting: ClockHourFormat, culture_uses_twelve_hour: bool) -> bool {
        match setting {
            ClockHourFormat::Hour12 => true,
            ClockHourFormat::Hour24 => false,
            _ => culture_uses_twelve_hour,
        }
    }

    /// Kültürün kısa saat deseni 12 saatlik mi ("h:mm tt" gibi küçük h).
    pub fn culture_uses_twelve_hour(culture: &Culture) -> bool {
        culture.short_time_pattern.contains('h')
    }

    /// Ayar ve kültürden son saat deseni (Auto kültürü izler).
    pub fn resolve_time_pattern(setting: ClockHourFormat, show_seconds: bool, culture: &Culture) -> String {
        Self::time_pattern(Self::uses_twelve_hour_clock(setting, Self::culture_uses_twelve_hour(culture)), show_seconds)
    }

    /// Saat deseni: "HH:mm", "HH:mm:ss", "h:mm tt", "h:mm:ss tt". "tt" öğlenden önce/sonra işaretini kültürden alır.
    pub fn time_pattern(twelve_hour: bool, show_seconds: bool) -> String {
        let hour = if twelve_hour { "h" } else { "HH" };
        let pattern = if show_seconds { format!("{hour}:mm:ss") } else { format!("{hour}:mm") };
        if twelve_hour { format!("{pattern} tt") } else { pattern }
    }

    /// Haftanın ilk günü: seçim System değilse seçilen gün, değilse kültürün ilk günü.
    pub fn resolve_first_day_of_week(setting: WeekStartSetting, culture_first_day: DayOfWeek) -> DayOfWeek {
        match setting {
            WeekStartSetting::Saturday => DayOfWeek::Saturday,
            WeekStartSetting::Sunday => DayOfWeek::Sunday,
            WeekStartSetting::Monday => DayOfWeek::Monday,
            _ => culture_first_day,
        }
    }

    /// Hafta numarası. Kural: yılın ilk haftası, yılın en az dört gününü içeren ilk hafta (ISO 8601 ile aynı mantık;
    /// haftanın ilk günü seçime göre). Yıl sonundaki günler sonraki yılın 1. haftasına düşebilir (Pazartesi
    /// başlangıçta 31.12.2007 → 1). Takvimde gösterilir.
    // Calendar.GetWeekOfYear(FirstFourDayWeek) yıl sonunu sonraki yıla taşımaz (31.12.2007 → 53), bu yüzden elle:
    // bir hafta, dördüncü gününün düştüğü yıla aittir; o yılın 1. haftası 4 Ocak'ı içeren haftadır.
    pub fn week_of_year(date: NaiveDateTime, first_day: DayOfWeek) -> i32 {
        let week_start = Self::start_of_week(date.date(), first_day);
        let year = (week_start + Duration::days(3)).year();
        let first_week_start = Self::start_of_week(NaiveDate::from_ymd_opt(year, 1, 4).expect("4 Ocak her yılda vardır"), first_day);
        ((week_start - first_week_start).num_days() / 7) as i32 + 1
    }

    fn start_of_week(date: NaiveDate, first_day: DayOfWeek) -> NaiveDate {
        date - Duration::days(i64::from((DayOfWeek::of(date).index() - first_day.index() + 7) % 7))
    }

    /// Özel tarih biçimi geçerli mi: boş olmayan, en fazla [`Self::MAX_CUSTOM_DATE_FORMAT_LENGTH`] karakter, denetim
    /// karakteri içermeyen ve örnek tarihte biçimlenebilen metin.
    pub fn is_valid_custom_date_format(format: Option<&str>) -> bool {
        let Some(format) = format else { return false };
        if crate::text::is_null_or_white_space(Some(format)) || format.encode_utf16().count() > Self::MAX_CUSTOM_DATE_FORMAT_LENGTH {
            return false;
        }
        if format.chars().any(char::is_control) {
            return false;
        }

        // Özel biçim denetimi için sabit örnek tarih: kayıt öncesi biçim hatası burada yakalanır.
        let sample_date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap().and_hms_opt(13, 7, 9).unwrap();
        match format_date_time(sample_date, None, format, &Culture::invariant()) {
            Ok(text) => !text.is_empty(),
            Err(_) => false,
        }
    }

    /// İkinci saat açık mı: saat dilimi kimliği dolu olmalı.
    pub fn has_second_clock(time_zone_id: Option<&str>) -> bool {
        !crate::text::is_null_or_white_space(time_zone_id)
    }

    /// Göz atan çentikteki saat satırı: "saat · tarih · ikinci saat". Tarih ve ikinci saat boş/null ise eklenmez,
    /// yani hiçbiri açık değilse yalnız saat döner (mevcut davranış).
    pub fn compose_peek_line(time: &str, date: Option<&str>, second_clock: Option<&str>) -> String {
        let mut line = time.to_string();
        if !crate::text::is_null_or_white_space(date) {
            line.push_str(" · ");
            line.push_str(date.unwrap_or_default());
        }
        if !crate::text::is_null_or_white_space(second_clock) {
            line.push_str(" · ");
            line.push_str(second_clock.unwrap_or_default());
        }
        line
    }
}

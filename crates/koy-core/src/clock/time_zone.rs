use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc};

use super::day_of_week::DayOfWeek;
use crate::time::{DateTimeOffset, TimeSpan};

/// `TimeZoneInfo.TransitionTime`: yaz saatine geçiş/dönüş anı (saat dilimindeki yerel duvar saatiyle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionTime {
    /// Her yıl aynı gün ("her yıl 5 Nisan").
    Fixed { month: u32, day: u32, time_of_day: NaiveTime },
    /// Ayın n. haftasının belirli günü; `week` 5 ise ayın son haftası.
    Floating { month: u32, week: u32, day_of_week: DayOfWeek, time_of_day: NaiveTime },
}

impl TransitionTime {
    fn date_time(&self, year: i32) -> NaiveDateTime {
        match *self {
            TransitionTime::Fixed { month, day, time_of_day } => {
                NaiveDate::from_ymd_opt(year, month, day).expect("geçersiz geçiş günü").and_time(time_of_day)
            }
            TransitionTime::Floating { month, week, day_of_week, time_of_day } => {
                let first = NaiveDate::from_ymd_opt(year, month, 1).expect("geçersiz geçiş ayı");
                let offset = (day_of_week.index() - DayOfWeek::of(first).index() + 7) % 7;
                let mut date = first + chrono::Duration::days((offset + (week as i32 - 1) * 7) as i64);
                // 5. hafta "son hafta" demektir: ay dışına taştıysa bir hafta geri.
                if date.month() != month {
                    date -= chrono::Duration::days(7);
                }
                date.and_time(time_of_day)
            }
        }
    }
}

/// Tüm yıllar için geçerli tek bir yaz saati kuralı (`TimeZoneInfo.AdjustmentRule` sadeleştirilmiş).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdjustmentRule {
    pub daylight_delta: TimeSpan,
    /// Standart saatle ifade edilen yerel duvar saati.
    pub daylight_transition_start: TransitionTime,
    /// Yaz saatiyle ifade edilen yerel duvar saati.
    pub daylight_transition_end: TransitionTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Rules,
    /// Makinenin yerel saat dilimi (`TimeZoneInfo.Local`), chrono üzerinden.
    Local,
}

/// C# `TimeZoneInfo` için küçük karşılık. Core saf kalsın diye Windows kayıt defterini okumaz: tanıdığı sistem
/// kimlikleri [`TimeZoneInfo::find_system_time_zone_by_id`] içindeki gömülü tablodadır (Windows saat dilimi adları;
/// tabloda olmayan kimlik "bulunamadı" sayılır). Gerçek sistem verisi gerekirse Platform katmanı doldurur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeZoneInfo {
    id: String,
    display_name: String,
    standard_name: String,
    base_utc_offset: TimeSpan,
    rule: Option<AdjustmentRule>,
    kind: Kind,
}

impl TimeZoneInfo {
    /// `TimeZoneInfo.Utc`.
    pub fn utc() -> TimeZoneInfo {
        TimeZoneInfo::create_custom_time_zone("UTC", TimeSpan::ZERO, "(UTC) Coordinated Universal Time", "Coordinated Universal Time")
    }

    /// `TimeZoneInfo.Local`.
    pub fn local() -> TimeZoneInfo {
        TimeZoneInfo {
            id: "Local".to_string(),
            display_name: "Local".to_string(),
            standard_name: "Local".to_string(),
            base_utc_offset: TimeSpan::ZERO,
            rule: None,
            kind: Kind::Local,
        }
    }

    /// `TimeZoneInfo.CreateCustomTimeZone(id, baseUtcOffset, displayName, standardDisplayName)`.
    pub fn create_custom_time_zone(id: &str, base_utc_offset: TimeSpan, display_name: &str, standard_name: &str) -> TimeZoneInfo {
        TimeZoneInfo {
            id: id.to_string(),
            display_name: display_name.to_string(),
            standard_name: standard_name.to_string(),
            base_utc_offset,
            rule: None,
            kind: Kind::Rules,
        }
    }

    /// Yaz saati kurallı özel saat dilimi.
    pub fn create_custom_time_zone_with_rule(
        id: &str,
        base_utc_offset: TimeSpan,
        display_name: &str,
        standard_name: &str,
        rule: AdjustmentRule,
    ) -> TimeZoneInfo {
        TimeZoneInfo { rule: Some(rule), ..Self::create_custom_time_zone(id, base_utc_offset, display_name, standard_name) }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn standard_name(&self) -> &str {
        &self.standard_name
    }

    pub fn base_utc_offset(&self) -> TimeSpan {
        self.base_utc_offset
    }

    pub fn supports_daylight_saving_time(&self) -> bool {
        self.rule.is_some() || self.kind == Kind::Local
    }

    /// `FindSystemTimeZoneById`: bulunamazsa ya da kimlik bozuksa (boş, denetim karakteri, yol ayracı) `None`;
    /// C#'ta bunlar `TimeZoneNotFoundException`/`ArgumentException`/`InvalidTimeZoneException` olup `SecondClock`
    /// hepsini yakalar. Karşılaştırma büyük/küçük harfe duyarsızdır (Windows kayıt defteri gibi).
    pub fn find_system_time_zone_by_id(id: &str) -> Option<TimeZoneInfo> {
        if id.is_empty() || id.chars().any(|c| c.is_control() || c == '\\' || c == '/') {
            return None;
        }
        SYSTEM_ZONES.iter().find(|z| z.id.eq_ignore_ascii_case(id)).map(ZoneEntry::build)
    }

    /// `GetSystemTimeZones`: (kimlik, görünen ad) çiftleri, UTC farkına göre sıralı.
    pub fn system_time_zones() -> Vec<(&'static str, &'static str)> {
        let mut zones: Vec<&ZoneEntry> = SYSTEM_ZONES.iter().collect();
        zones.sort_by_key(|z| z.base_minutes);
        zones.into_iter().map(|z| (z.id, z.display)).collect()
    }

    /// Verilen UTC anındaki UTC farkı (`GetUtcOffset`).
    pub fn get_utc_offset(&self, utc: DateTime<Utc>) -> TimeSpan {
        match self.kind {
            Kind::Local => TimeSpan::from_secs(i64::from(Local.offset_from_utc_datetime(&utc.naive_utc()).fix().local_minus_utc())),
            Kind::Rules => match self.rule {
                Some(rule) if self.is_daylight(rule, utc.naive_utc()) => self.base_utc_offset + rule.daylight_delta,
                _ => self.base_utc_offset,
            },
        }
    }

    fn is_daylight(&self, rule: AdjustmentRule, utc: NaiveDateTime) -> bool {
        let year = (utc + self.base_utc_offset.to_chrono()).year();
        let start_utc = rule.daylight_transition_start.date_time(year) - self.base_utc_offset.to_chrono();
        let end_utc = rule.daylight_transition_end.date_time(year) - (self.base_utc_offset + rule.daylight_delta).to_chrono();
        if start_utc < end_utc {
            utc >= start_utc && utc < end_utc
        } else {
            // Güney yarımküre: yaz saati yıl başında ve sonunda.
            !(utc >= end_utc && utc < start_utc)
        }
    }

    /// `TimeZoneInfo.ConvertTime(DateTimeOffset, zone)`.
    pub fn convert_time(at: DateTimeOffset, zone: &TimeZoneInfo) -> DateTimeOffset {
        let utc = at.with_timezone(&Utc);
        let offset = zone.get_utc_offset(utc);
        let seconds = (offset.ticks() / TimeSpan::TICKS_PER_SECOND) as i32;
        let fixed = FixedOffset::east_opt(seconds).expect("geçersiz UTC farkı");
        utc.with_timezone(&fixed)
    }

    /// `TimeZoneInfo.ConvertTimeFromUtc(DateTime, zone)`: UTC duvar saatinden bölgenin duvar saatine.
    pub fn convert_time_from_utc(utc: NaiveDateTime, zone: &TimeZoneInfo) -> NaiveDateTime {
        let offset = zone.get_utc_offset(utc.and_utc());
        utc + offset.to_chrono()
    }
}

struct ZoneEntry {
    id: &'static str,
    display: &'static str,
    base_minutes: i64,
    dst: Dst,
}

#[derive(Clone, Copy)]
enum Dst {
    None,
    Us,
    Eu,
    Australia,
    NewZealand,
}

impl ZoneEntry {
    fn build(&self) -> TimeZoneInfo {
        let base = TimeSpan::from_mins(self.base_minutes);
        let hm = |h: u32, m: u32| NaiveTime::from_hms_opt(h, m, 0).unwrap();
        let sunday = DayOfWeek::Sunday;
        let hour = TimeSpan::from_hours(1);
        let rule = match self.dst {
            Dst::None => None,
            // 2. Pazar Mart 02:00 (standart) → 1. Pazar Kasım 02:00 (yaz).
            Dst::Us => Some(AdjustmentRule {
                daylight_delta: hour,
                daylight_transition_start: TransitionTime::Floating { month: 3, week: 2, day_of_week: sunday, time_of_day: hm(2, 0) },
                daylight_transition_end: TransitionTime::Floating { month: 11, week: 1, day_of_week: sunday, time_of_day: hm(2, 0) },
            }),
            // Son Pazar Mart/Ekim, 01:00 UTC'de: yerel duvar saatine çevrilir.
            Dst::Eu => {
                let start_local = hm(1, 0).overflowing_add_signed(base.to_chrono()).0;
                let end_local = hm(1, 0).overflowing_add_signed((base + hour).to_chrono()).0;
                Some(AdjustmentRule {
                    daylight_delta: hour,
                    daylight_transition_start: TransitionTime::Floating { month: 3, week: 5, day_of_week: sunday, time_of_day: start_local },
                    daylight_transition_end: TransitionTime::Floating { month: 10, week: 5, day_of_week: sunday, time_of_day: end_local },
                })
            }
            // 1. Pazar Ekim 02:00 → 1. Pazar Nisan 03:00 (yaz).
            Dst::Australia => Some(AdjustmentRule {
                daylight_delta: hour,
                daylight_transition_start: TransitionTime::Floating { month: 10, week: 1, day_of_week: sunday, time_of_day: hm(2, 0) },
                daylight_transition_end: TransitionTime::Floating { month: 4, week: 1, day_of_week: sunday, time_of_day: hm(3, 0) },
            }),
            // Son Pazar Eylül 02:00 → 1. Pazar Nisan 03:00 (yaz).
            Dst::NewZealand => Some(AdjustmentRule {
                daylight_delta: hour,
                daylight_transition_start: TransitionTime::Floating { month: 9, week: 5, day_of_week: sunday, time_of_day: hm(2, 0) },
                daylight_transition_end: TransitionTime::Floating { month: 4, week: 1, day_of_week: sunday, time_of_day: hm(3, 0) },
            }),
        };
        TimeZoneInfo {
            id: self.id.to_string(),
            display_name: self.display.to_string(),
            standard_name: self.id.to_string(),
            base_utc_offset: base,
            rule,
            kind: Kind::Rules,
        }
    }
}

const fn z(id: &'static str, display: &'static str, base_minutes: i64, dst: Dst) -> ZoneEntry {
    ZoneEntry { id, display, base_minutes, dst }
}

// Windows saat dilimi kimlikleri: sık kullanılanlar. UTC farkları standart saat içindir.
static SYSTEM_ZONES: &[ZoneEntry] = &[
    z("UTC", "(UTC) Coordinated Universal Time", 0, Dst::None),
    z("Greenwich Standard Time", "(UTC+00:00) Monrovia, Reykjavik", 0, Dst::None),
    z("GMT Standard Time", "(UTC+00:00) Dublin, Edinburgh, Lisbon, London", 0, Dst::Eu),
    z("W. Europe Standard Time", "(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna", 60, Dst::Eu),
    z("Central Europe Standard Time", "(UTC+01:00) Belgrade, Bratislava, Budapest, Ljubljana, Prague", 60, Dst::Eu),
    z("Romance Standard Time", "(UTC+01:00) Brussels, Copenhagen, Madrid, Paris", 60, Dst::Eu),
    z("Central European Standard Time", "(UTC+01:00) Sarajevo, Skopje, Warsaw, Zagreb", 60, Dst::Eu),
    z("E. Europe Standard Time", "(UTC+02:00) Chisinau", 120, Dst::Eu),
    z("FLE Standard Time", "(UTC+02:00) Helsinki, Kyiv, Riga, Sofia, Tallinn, Vilnius", 120, Dst::Eu),
    z("GTB Standard Time", "(UTC+02:00) Athens, Bucharest", 120, Dst::Eu),
    z("Turkey Standard Time", "(UTC+03:00) Istanbul", 180, Dst::None),
    z("Arab Standard Time", "(UTC+03:00) Kuwait, Riyadh", 180, Dst::None),
    z("E. Africa Standard Time", "(UTC+03:00) Nairobi", 180, Dst::None),
    z("Russian Standard Time", "(UTC+03:00) Moscow, St. Petersburg", 180, Dst::None),
    z("Arabian Standard Time", "(UTC+04:00) Abu Dhabi, Muscat", 240, Dst::None),
    z("India Standard Time", "(UTC+05:30) Chennai, Kolkata, Mumbai, New Delhi", 330, Dst::None),
    z("China Standard Time", "(UTC+08:00) Beijing, Chongqing, Hong Kong, Urumqi", 480, Dst::None),
    z("Singapore Standard Time", "(UTC+08:00) Kuala Lumpur, Singapore", 480, Dst::None),
    z("W. Australia Standard Time", "(UTC+08:00) Perth", 480, Dst::None),
    z("Tokyo Standard Time", "(UTC+09:00) Osaka, Sapporo, Tokyo", 540, Dst::None),
    z("Korea Standard Time", "(UTC+09:00) Seoul", 540, Dst::None),
    z("AUS Eastern Standard Time", "(UTC+10:00) Canberra, Melbourne, Sydney", 600, Dst::Australia),
    z("New Zealand Standard Time", "(UTC+12:00) Auckland, Wellington", 720, Dst::NewZealand),
    z("E. South America Standard Time", "(UTC-03:00) Brasilia", -180, Dst::None),
    z("Eastern Standard Time", "(UTC-05:00) Eastern Time (US & Canada)", -300, Dst::Us),
    z("SA Pacific Standard Time", "(UTC-05:00) Bogota, Lima, Quito, Rio Branco", -300, Dst::None),
    z("Central Standard Time", "(UTC-06:00) Central Time (US & Canada)", -360, Dst::Us),
    z("Central America Standard Time", "(UTC-06:00) Central America", -360, Dst::None),
    z("Mountain Standard Time", "(UTC-07:00) Mountain Time (US & Canada)", -420, Dst::Us),
    z("Pacific Standard Time", "(UTC-08:00) Pacific Time (US & Canada)", -480, Dst::Us),
    z("Alaskan Standard Time", "(UTC-09:00) Alaska", -540, Dst::Us),
    z("Hawaiian Standard Time", "(UTC-10:00) Hawaii", -600, Dst::None),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, mo: u32, d: u32, h: u32) -> DateTime<Utc> {
        NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, 0, 0).unwrap().and_utc()
    }

    #[test]
    fn finds_known_ids_and_rejects_bad_ones() {
        assert!(TimeZoneInfo::find_system_time_zone_by_id("UTC").is_some());
        assert!(TimeZoneInfo::find_system_time_zone_by_id(" UTC").is_none());
        assert!(TimeZoneInfo::find_system_time_zone_by_id("tokyo standard time").is_some());
        assert!(TimeZoneInfo::find_system_time_zone_by_id("Not/A_Real_Zone").is_none());
        assert!(TimeZoneInfo::find_system_time_zone_by_id("Bad\0Id").is_none());
    }

    #[test]
    fn daylight_rules_follow_the_calendar() {
        let berlin = TimeZoneInfo::find_system_time_zone_by_id("W. Europe Standard Time").unwrap();
        assert_eq!(berlin.get_utc_offset(utc(2026, 1, 15, 12)), TimeSpan::from_hours(1));
        assert_eq!(berlin.get_utc_offset(utc(2026, 7, 15, 12)), TimeSpan::from_hours(2));
        // 2026: yaz saati 29 Mart 01:00 UTC'de başlar, 25 Ekim 01:00 UTC'de biter.
        assert_eq!(berlin.get_utc_offset(utc(2026, 3, 29, 0)), TimeSpan::from_hours(1));
        assert_eq!(berlin.get_utc_offset(utc(2026, 3, 29, 1)), TimeSpan::from_hours(2));
        assert_eq!(berlin.get_utc_offset(utc(2026, 10, 25, 0)), TimeSpan::from_hours(2));
        assert_eq!(berlin.get_utc_offset(utc(2026, 10, 25, 1)), TimeSpan::from_hours(1));

        let ny = TimeZoneInfo::find_system_time_zone_by_id("Eastern Standard Time").unwrap();
        assert_eq!(ny.get_utc_offset(utc(2026, 7, 1, 12)), TimeSpan::from_hours(-4));
        assert_eq!(ny.get_utc_offset(utc(2026, 12, 1, 12)), TimeSpan::from_hours(-5));

        let sydney = TimeZoneInfo::find_system_time_zone_by_id("AUS Eastern Standard Time").unwrap();
        assert_eq!(sydney.get_utc_offset(utc(2026, 1, 15, 12)), TimeSpan::from_hours(11));
        assert_eq!(sydney.get_utc_offset(utc(2026, 7, 15, 12)), TimeSpan::from_hours(10));
    }
}

use super::day_of_week::DayOfWeek;

/// C# `CultureInfo` için küçük karşılık: Core'un ihtiyaç duyduğu biçim verileri (ondalık ayracı, öğleden önce/sonra
/// işaretleri, kısa saat deseni, gün/ay adları, haftanın ilk günü).
///
/// Yalnız Koy'un dil seçeneklerine yetecek kültürler gömülüdür (Invariant, en-US, en-GB, tr-TR, de-DE); verisi
/// Windows/ICU'dakine yakındır ama birebir değildir. Başka kültür gerekirse [`Culture::from_name`]'e eklenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Culture {
    pub name: &'static str,
    pub decimal_separator: &'static str,
    pub time_separator: &'static str,
    pub date_separator: &'static str,
    pub am_designator: &'static str,
    pub pm_designator: &'static str,
    pub era_name: &'static str,
    pub first_day_of_week: DayOfWeek,
    /// `DateTimeFormat.ShortTimePattern` ("h:mm tt" / "HH:mm").
    pub short_time_pattern: &'static str,
    pub long_time_pattern: &'static str,
    pub short_date_pattern: &'static str,
    pub long_date_pattern: &'static str,
    pub month_day_pattern: &'static str,
    pub year_month_pattern: &'static str,
    /// Pazar'dan başlayarak.
    pub day_names: [&'static str; 7],
    pub abbreviated_day_names: [&'static str; 7],
    pub month_names: [&'static str; 12],
    pub abbreviated_month_names: [&'static str; 12],
}

const EN_DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const EN_DAYS_ABBR: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const EN_MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
];
const EN_MONTHS_ABBR: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const TR_DAYS: [&str; 7] = ["Pazar", "Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi"];
const TR_DAYS_ABBR: [&str; 7] = ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"];
const TR_MONTHS: [&str; 12] = [
    "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık",
];
const TR_MONTHS_ABBR: [&str; 12] = ["Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara"];

const DE_DAYS: [&str; 7] = ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"];
const DE_DAYS_ABBR: [&str; 7] = ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"];
const DE_MONTHS: [&str; 12] = [
    "Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember",
];
const DE_MONTHS_ABBR: [&str; 12] = ["Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"];

impl Culture {
    /// `CultureInfo.InvariantCulture`.
    pub const fn invariant() -> Culture {
        Culture {
            name: "",
            decimal_separator: ".",
            time_separator: ":",
            date_separator: "/",
            am_designator: "AM",
            pm_designator: "PM",
            era_name: "A.D.",
            first_day_of_week: DayOfWeek::Sunday,
            short_time_pattern: "HH:mm",
            long_time_pattern: "HH:mm:ss",
            short_date_pattern: "MM/dd/yyyy",
            long_date_pattern: "dddd, dd MMMM yyyy",
            month_day_pattern: "MMMM dd",
            year_month_pattern: "yyyy MMMM",
            day_names: EN_DAYS,
            abbreviated_day_names: EN_DAYS_ABBR,
            month_names: EN_MONTHS,
            abbreviated_month_names: EN_MONTHS_ABBR,
        }
    }

    pub const fn en_us() -> Culture {
        Culture {
            name: "en-US",
            short_time_pattern: "h:mm tt",
            long_time_pattern: "h:mm:ss tt",
            short_date_pattern: "M/d/yyyy",
            long_date_pattern: "dddd, MMMM d, yyyy",
            month_day_pattern: "MMMM d",
            year_month_pattern: "MMMM yyyy",
            ..Culture::invariant()
        }
    }

    pub const fn en_gb() -> Culture {
        Culture {
            name: "en-GB",
            first_day_of_week: DayOfWeek::Monday,
            short_date_pattern: "dd/MM/yyyy",
            long_date_pattern: "dddd, d MMMM yyyy",
            month_day_pattern: "d MMMM",
            year_month_pattern: "MMMM yyyy",
            ..Culture::invariant()
        }
    }

    pub const fn tr_tr() -> Culture {
        Culture {
            name: "tr-TR",
            decimal_separator: ",",
            date_separator: ".",
            am_designator: "ÖÖ",
            pm_designator: "ÖS",
            era_name: "M.S.",
            first_day_of_week: DayOfWeek::Monday,
            short_date_pattern: "d.MM.yyyy",
            long_date_pattern: "d MMMM yyyy dddd",
            month_day_pattern: "d MMMM",
            year_month_pattern: "MMMM yyyy",
            day_names: TR_DAYS,
            abbreviated_day_names: TR_DAYS_ABBR,
            month_names: TR_MONTHS,
            abbreviated_month_names: TR_MONTHS_ABBR,
            ..Culture::invariant()
        }
    }

    pub const fn de_de() -> Culture {
        Culture {
            name: "de-DE",
            decimal_separator: ",",
            date_separator: ".",
            era_name: "n. Chr.",
            first_day_of_week: DayOfWeek::Monday,
            short_date_pattern: "dd.MM.yyyy",
            long_date_pattern: "dddd, d. MMMM yyyy",
            month_day_pattern: "d. MMMM",
            year_month_pattern: "MMMM yyyy",
            day_names: DE_DAYS,
            abbreviated_day_names: DE_DAYS_ABBR,
            month_names: DE_MONTHS,
            abbreviated_month_names: DE_MONTHS_ABBR,
            ..Culture::invariant()
        }
    }

    /// `new CultureInfo(name)`: gömülü kültürlerde bulunur, bilinmeyen ad için `None`
    /// (C#'ta `CultureNotFoundException`). Ad büyük/küçük harfe duyarsızdır; boş ad Invariant'tır.
    pub fn from_name(name: &str) -> Option<Culture> {
        match name.to_ascii_lowercase().as_str() {
            "" => Some(Culture::invariant()),
            "en-us" | "en" => Some(Culture::en_us()),
            "en-gb" => Some(Culture::en_gb()),
            "tr-tr" | "tr" => Some(Culture::tr_tr()),
            "de-de" | "de" => Some(Culture::de_de()),
            _ => None,
        }
    }
}

use super::decimal::Decimal;
use super::search_util::utf16_len;
use super::SearchText;
use crate::clock::Culture;
use crate::text;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UnitCategory {
    #[default]
    Length,
    Mass,
    Temperature,
    Data,
    Speed,
}

/// Bir birim. Sıcaklık dışında `factor`: kategorinin taban birimine çarpan.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureUnit {
    pub symbol: &'static str,
    pub category: UnitCategory,
    pub factor: f64,
}

impl MeasureUnit {
    pub const fn new(symbol: &'static str, category: UnitCategory, factor: f64) -> Self {
        MeasureUnit { symbol, category, factor }
    }
}

/// Başarılı çeviri: girdi, iki birim ve sonuç (yuvarlanmamış).
#[derive(Debug, Clone, PartialEq)]
pub struct UnitConversion {
    pub input: f64,
    pub from: MeasureUnit,
    pub to: MeasureUnit,
    pub value: f64,
}

/// Birim çevirici: uzunluk, kütle, sıcaklık, veri boyutu, hız. Saf mantık, ağ yok, hiçbir yol panik atmaz.
/// Taban birimler: metre, kilogram, bayt, m/s; sıcaklık Celsius üzerinden (ofsetli) çevrilir.
/// Veri boyutu: kb, mb, gb, tb SI'dır (1 kb = 1000 bayt, 1 gb = 1000 mb). İkili katlar için kib, mib, gib, tib (1024) yazılır.
pub struct UnitConverter;

const MAX_INPUT_LENGTH: usize = 64;
const ABSOLUTE_ZERO_CELSIUS: f64 = -273.15;
const SIGNIFICANT_DIGITS: i32 = 6;
const MAX_DECIMAL_PLACES: i32 = 12;

const KM: MeasureUnit = MeasureUnit::new("km", UnitCategory::Length, 1000.0);
const M: MeasureUnit = MeasureUnit::new("m", UnitCategory::Length, 1.0);
const CM: MeasureUnit = MeasureUnit::new("cm", UnitCategory::Length, 0.01);
const MI: MeasureUnit = MeasureUnit::new("mi", UnitCategory::Length, 1609.344);
const FT: MeasureUnit = MeasureUnit::new("ft", UnitCategory::Length, 0.3048);
const IN: MeasureUnit = MeasureUnit::new("in", UnitCategory::Length, 0.0254);

const KG: MeasureUnit = MeasureUnit::new("kg", UnitCategory::Mass, 1.0);
const G: MeasureUnit = MeasureUnit::new("g", UnitCategory::Mass, 0.001);
const LB: MeasureUnit = MeasureUnit::new("lb", UnitCategory::Mass, 0.45359237);
const OZ: MeasureUnit = MeasureUnit::new("oz", UnitCategory::Mass, 0.028349523125);

const CELSIUS: MeasureUnit = MeasureUnit::new("°C", UnitCategory::Temperature, 1.0);
const FAHRENHEIT: MeasureUnit = MeasureUnit::new("°F", UnitCategory::Temperature, 1.0);
const KELVIN: MeasureUnit = MeasureUnit::new("K", UnitCategory::Temperature, 1.0);

const BYTE: MeasureUnit = MeasureUnit::new("B", UnitCategory::Data, 1.0);
const KB: MeasureUnit = MeasureUnit::new("kB", UnitCategory::Data, 1e3);
const MB: MeasureUnit = MeasureUnit::new("MB", UnitCategory::Data, 1e6);
const GB: MeasureUnit = MeasureUnit::new("GB", UnitCategory::Data, 1e9);
const TB: MeasureUnit = MeasureUnit::new("TB", UnitCategory::Data, 1e12);
const KIB: MeasureUnit = MeasureUnit::new("KiB", UnitCategory::Data, 1024.0);
const MIB: MeasureUnit = MeasureUnit::new("MiB", UnitCategory::Data, 1024.0 * 1024.0);
const GIB: MeasureUnit = MeasureUnit::new("GiB", UnitCategory::Data, 1024.0 * 1024.0 * 1024.0);
const TIB: MeasureUnit = MeasureUnit::new("TiB", UnitCategory::Data, 1024.0 * 1024.0 * 1024.0 * 1024.0);

const KMH: MeasureUnit = MeasureUnit::new("km/h", UnitCategory::Speed, 1.0 / 3.6);
const MPH: MeasureUnit = MeasureUnit::new("mph", UnitCategory::Speed, 0.44704);
const MS: MeasureUnit = MeasureUnit::new("m/s", UnitCategory::Speed, 1.0);

const SEPARATORS: [&str; 4] = ["to", "in", "into", "as"];

fn head_pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?P<num>[+-]?(?:\d[\d.,]*|[.,]\d[\d.,]*))\s*(?P<rest>\S.*)?$").unwrap())
}

fn arrow_pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\s*(->|=>|→|=|>)\s*").unwrap())
}

/// Anahtarlar `SearchText.Fold` ile sadeleşmiş (küçük harf, Türkçe harfler sade).
fn aliases() -> &'static HashMap<&'static str, MeasureUnit> {
    static MAP: OnceLock<HashMap<&'static str, MeasureUnit>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();
        let mut add = |unit: MeasureUnit, names: &[&'static str]| {
            for name in names {
                map.insert(*name, unit.clone());
            }
        };

        add(KM, &["km", "kilometer", "kilometers", "kilometre", "kilometres"]);
        add(M, &["m", "meter", "meters", "metre", "metres"]);
        add(CM, &["cm", "centimeter", "centimeters", "centimetre", "centimetres", "santimetre"]);
        add(MI, &["mi", "mile", "miles", "mil"]);
        add(FT, &["ft", "foot", "feet", "ayak"]);
        add(IN, &["in", "inch", "inches", "inc"]);

        add(KG, &["kg", "kilogram", "kilograms", "kilo"]);
        add(G, &["g", "gr", "gram", "grams"]);
        add(LB, &["lb", "lbs", "pound", "pounds"]);
        add(OZ, &["oz", "ounce", "ounces", "ons"]);

        add(CELSIUS, &["c", "°c", "ºc", "celsius", "santigrat"]);
        add(FAHRENHEIT, &["f", "°f", "ºf", "fahrenheit"]);
        add(KELVIN, &["k", "kelvin"]);

        add(BYTE, &["b", "byte", "bytes"]);
        add(KB, &["kb", "kilobyte", "kilobytes"]);
        add(MB, &["mb", "megabyte", "megabytes"]);
        add(GB, &["gb", "gigabyte", "gigabytes"]);
        add(TB, &["tb", "terabyte", "terabytes"]);
        add(KIB, &["kib", "kibibyte", "kibibytes"]);
        add(MIB, &["mib", "mebibyte", "mebibytes"]);
        add(GIB, &["gib", "gibibyte", "gibibytes"]);
        add(TIB, &["tib", "tebibyte", "tebibytes"]);

        add(KMH, &["km/h", "kmh", "kph", "kmph"]);
        add(MPH, &["mph"]);
        add(MS, &["m/s", "mps"]);
        map
    })
}

impl UnitConverter {
    /// "10 km to mi", "100 c in f", "5 kg lb", "10,5km->mi" gibi metni çözer ve çevirir.
    pub fn try_convert(text: Option<&str>, culture: &Culture) -> Option<UnitConversion> {
        let text = text?;
        if text::is_null_or_white_space(Some(text)) || utf16_len(text) > MAX_INPUT_LENGTH {
            return None;
        }

        let normalized = text.trim().replace('−', "-");
        let head = head_pattern().captures(&normalized)?;
        let rest_match = head.name("rest")?;

        let input = Self::try_parse_number(Some(&head["num"]), culture)?;

        let rest = arrow_pattern().replace_all(&SearchText::fold_str(rest_match.as_str()), " to ").into_owned();
        let tokens: Vec<&str> = rest.split(' ').filter(|t| !t.is_empty()).collect();
        let (from_name, to_name) = match tokens.len() {
            2 => (tokens[0], tokens[1]),
            3 if SEPARATORS.contains(&tokens[1]) => (tokens[0], tokens[2]),
            _ => return None,
        };

        let from = aliases().get(from_name)?.clone();
        let to = aliases().get(to_name)?.clone();

        let value = Self::try_convert_value(input, &from, &to)?;

        Some(UnitConversion { input, from, to, value })
    }

    /// Aynı kategorideki iki birim arasında çevirir; kategori farklıysa, sıcaklık mutlak sıfırın altındaysa ya da
    /// sonuç sonsuzsa `None`. (C# aşırı yüklemesi `TryConvert(double, MeasureUnit, MeasureUnit, out double)`.)
    pub fn try_convert_value(value: f64, from: &MeasureUnit, to: &MeasureUnit) -> Option<f64> {
        if from.category != to.category || !value.is_finite() {
            return None;
        }

        let result;
        if from.category == UnitCategory::Temperature {
            let celsius = Self::to_celsius(value, from);
            if celsius < ABSOLUTE_ZERO_CELSIUS - 1e-9 {
                return None;
            }

            result = Self::from_celsius(celsius, to);
        } else {
            result = value * from.factor / to.factor;
        }

        if result.is_finite() { Some(result) } else { None }
    }

    /// Takma ad ("KM/H", "°C", "miles") ile birim bulur.
    pub fn try_find_unit(name: Option<&str>) -> Option<MeasureUnit> {
        let name = name?;
        if text::is_null_or_white_space(Some(name)) {
            return None;
        }
        aliases().get(SearchText::fold_str(name.trim()).as_str()).cloned()
    }

    fn to_celsius(value: f64, unit: &MeasureUnit) -> f64 {
        if unit.symbol == FAHRENHEIT.symbol {
            (value - 32.0) * 5.0 / 9.0
        } else if unit.symbol == KELVIN.symbol {
            value - 273.15
        } else {
            value
        }
    }

    fn from_celsius(celsius: f64, unit: &MeasureUnit) -> f64 {
        if unit.symbol == FAHRENHEIT.symbol {
            celsius * 9.0 / 5.0 + 32.0
        } else if unit.symbol == KELVIN.symbol {
            celsius + 273.15
        } else {
            celsius
        }
    }

    /// "10,5" ve "10.5" ondalıktır. Tek ayraç kültürün binlik ayracıysa ve ardından tam 3 hane geliyorsa binliktir
    /// (tr: "1.500" = 1500, en: "1,500" = 1500). Birden çok aynı ayraç binliktir; ikisi birlikte ise sonuncusu ondalıktır.
    pub fn try_parse_number(text: Option<&str>, culture: &Culture) -> Option<f64> {
        let text = text?;
        if text::is_null_or_white_space(Some(text)) {
            return None;
        }

        let trimmed = text.trim();
        let mut chars: Vec<char> = trimmed.chars().collect();
        let mut sign = "";
        if chars[0] == '+' || chars[0] == '-' {
            sign = if chars[0] == '-' { "-" } else { "" };
            chars.remove(0);
        }

        if chars.is_empty()
            || !chars.iter().any(|c| c.is_ascii_digit())
            || chars.iter().any(|&c| !c.is_ascii_digit() && c != '.' && c != ',')
        {
            return None;
        }

        let s: String = chars.iter().collect();
        let dots = chars.iter().filter(|&&c| c == '.').count();
        let commas = chars.iter().filter(|&&c| c == ',').count();
        let clean: String;
        if dots > 0 && commas > 0 {
            let last_dot = chars.iter().rposition(|&c| c == '.').unwrap();
            let last_comma = chars.iter().rposition(|&c| c == ',').unwrap();
            let decimal_char = if last_dot > last_comma { '.' } else { ',' };
            let group_char = if decimal_char == '.' { ',' } else { '.' };
            let decimal_count = chars.iter().filter(|&&c| c == decimal_char).count();
            let last_group = chars.iter().rposition(|&c| c == group_char).unwrap();
            let first_decimal = chars.iter().position(|&c| c == decimal_char).unwrap();
            if decimal_count != 1 || last_group > first_decimal {
                return None;
            }

            clean = s.replace(group_char, "").replace(decimal_char, ".");
        } else if dots + commas > 1 {
            // Aynı ayraç tekrar ediyor: binlik. "1.2.3" gibi bozuk gruplar geçersiz.
            let separator = if dots > 0 { '.' } else { ',' };
            let groups: Vec<&str> = s.split(separator).collect();
            if groups[0].is_empty() || groups[0].len() > 3 || groups.iter().skip(1).any(|g| g.len() != 3) {
                return None;
            }

            clean = groups.concat();
        } else if dots + commas == 1 {
            let separator = if dots > 0 { '.' } else { ',' };
            let index = s.find(separator).unwrap();
            let before = &s[..index];
            let after = &s[index + 1..];
            let culture_group = super::decimal::group_separator(culture);
            let is_group = culture_group.len() == 1
                && culture_group.starts_with(separator)
                && after.len() == 3
                && (1..=3).contains(&before.len())
                && before != "0";
            clean = if is_group { format!("{before}{after}") } else { format!("{before}.{after}") };
        } else {
            clean = s;
        }

        let value: f64 = format!("{sign}{clean}").parse().ok()?;
        if value.is_finite() { Some(value) } else { None }
    }

    /// 6 anlamlı haneye yuvarlar (büyük sayılarda tam sayıya), gereksiz sıfırları atar; üstel yazım yok.
    pub fn format_number(value: f64, culture: &Culture, grouping: bool) -> String {
        if !value.is_finite() {
            return String::new();
        }

        if value == 0.0 {
            return "0".to_string();
        }

        let magnitude = value.abs().log10().floor() as i32;
        let places = text::clamp(SIGNIFICANT_DIGITS - 1 - magnitude, 0, MAX_DECIMAL_PLACES);
        if value.abs() < 7.9e27 {
            if let Some(d) = Decimal::from_f64(value) {
                let rounded = d.round(places as u32, true);
                if !rounded.is_zero() {
                    return rounded.format(grouping, MAX_DECIMAL_PLACES as u32, culture);
                }
            }
        }

        // Çok büyük ya da çok küçük: ondalık türe sığmaz, bilimsel yazım.
        format_general(value, SIGNIFICANT_DIGITS as usize, culture)
    }
}

/// `double.ToString("G<precision>", culture)`: anlamlı hane sayısı kadar; üs >= duyarlık ya da < -5 ise bilimsel.
fn format_general(value: f64, precision: usize, culture: &Culture) -> String {
    let text = format!("{:.*e}", precision - 1, value.abs());
    let (mantissa, exp_text) = text.split_once('e').unwrap();
    let exp: i32 = exp_text.parse().unwrap();
    let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };

    let mut out = String::new();
    if value < 0.0 {
        out.push('-');
    }

    if exp >= precision as i32 || exp < -5 {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push_str(culture.decimal_separator);
            out.push_str(&digits[1..]);
        }
        out.push('E');
        out.push(if exp < 0 { '-' } else { '+' });
        out.push_str(&format!("{:02}", exp.abs()));
    } else if exp >= 0 {
        let int_len = exp as usize + 1;
        if digits.len() <= int_len {
            out.push_str(digits);
            out.push_str(&"0".repeat(int_len - digits.len()));
        } else {
            out.push_str(&digits[..int_len]);
            out.push_str(culture.decimal_separator);
            out.push_str(&digits[int_len..]);
        }
    } else {
        out.push('0');
        out.push_str(culture.decimal_separator);
        out.push_str(&"0".repeat((-exp - 1) as usize));
        out.push_str(digits);
    }
    out
}

use super::unit_conversion::UnitConversion;
use super::unit_enums::WindSpeedUnit;
use crate::clock::Culture;

/// Rüzgâr hızı metni. Open-Meteo `wind_speed_10m` değerini varsayılan olarak km/sa verir; girdi km/sa'tir.
/// Birim sembolleri uluslararası olduğundan çeviri gerekmez.
pub struct WindSpeedFormatter;

impl WindSpeedFormatter {
    pub fn symbol(unit: WindSpeedUnit) -> &'static str {
        match unit {
            WindSpeedUnit::MetersPerSecond => "m/s",
            WindSpeedUnit::MilesPerHour => "mph",
            WindSpeedUnit::Knots => "kn",
            _ => "km/h",
        }
    }

    /// "12 km/h"; m/s tek ondalık gösterir ("3.3 m/s"). Geçersiz (NaN/sonsuz/negatif) değerde None.
    pub fn format(kilometers_per_hour: f64, unit: WindSpeedUnit, culture: &Culture) -> Option<String> {
        if !kilometers_per_hour.is_finite() || kilometers_per_hour < 0.0 {
            return None;
        }

        let value = UnitConversion::wind_speed(kilometers_per_hour, unit);
        let number = if unit == WindSpeedUnit::MetersPerSecond {
            to_string_fixed(value, 1, culture)
        } else {
            // MidpointRounding.AwayFromZero = f64::round.
            to_string_fixed(value.round(), 0, culture)
        };
        Some(format!("{number} {}", Self::symbol(unit)))
    }
}

/// `double.ToString("0")` / `ToString("0.0", culture)` karşılığı. .NET özel sayı biçimi değeri önce 15 anlamlı
/// basamağa yuvarlar, sonra o basamak dizisini yarım yukarı (>= 5) yuvarlayarak keser; Rust `{:.1}` ise ikilik
/// değerin tam açılımını yarım çifte yuvarlar (0.25 → "0.2", .NET "0.3"). Bu yüzden basamaklar elle işlenir.
fn to_string_fixed(value: f64, decimals: usize, culture: &Culture) -> String {
    const SIGNIFICANT: usize = 15;
    let negative = value.is_sign_negative();
    let sci = format!("{:.*e}", SIGNIFICANT - 1, value.abs());
    let (mantissa, exp) = sci.split_once('e').expect("bilimsel gösterim");
    let exp: i32 = exp.parse().expect("üs");
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    // Ondalık noktadan önceki basamak sayısı.
    let mut point = exp + 1;

    // Yalnız `decimals` ondalıktan sonrasını yarım yukarı yuvarla.
    let keep = point + decimals as i32;
    if keep < 0 {
        digits.clear();
    } else if (keep as usize) < digits.len() {
        let round_up = digits[keep as usize] >= 5;
        digits.truncate(keep as usize);
        if round_up {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    point += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }

    let digit_at = |index: i32| -> char {
        if index >= 0 && (index as usize) < digits.len() { (b'0' + digits[index as usize]) as char } else { '0' }
    };

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if point <= 0 {
        out.push('0');
    } else {
        for i in 0..point {
            out.push(digit_at(i));
        }
    }
    if decimals > 0 {
        out.push_str(culture.decimal_separator);
        for i in 0..decimals as i32 {
            out.push(digit_at(point + i));
        }
    }
    out
}

#[cfg(test)]
mod format_tests {
    use super::*;

    #[test]
    fn fixed_formatting_rounds_half_up_on_digits() {
        let inv = Culture::invariant();
        assert_eq!("0.3", to_string_fixed(0.25, 1, &inv));
        assert_eq!("3.3", to_string_fixed(12.0 / 3.6, 1, &inv));
        assert_eq!("10.0", to_string_fixed(36.0 / 3.6, 1, &inv));
        assert_eq!("0.0", to_string_fixed(0.0, 1, &inv));
        assert_eq!("0", to_string_fixed(0.0, 0, &inv));
        assert_eq!("100", to_string_fixed(99.95, 0, &inv));
        assert_eq!("0.1", to_string_fixed(0.05, 1, &inv));
        assert_eq!("0.0", to_string_fixed(0.04, 1, &inv));
        assert_eq!("1,0", to_string_fixed(0.96, 1, &Culture::tr_tr()));
    }
}

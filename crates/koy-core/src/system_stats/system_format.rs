use crate::clock::Culture;
use crate::settings::AppLanguage;
use crate::text;
use crate::units::{NetworkSpeedUnit, UnitConversion};

/// Sistem verisi biçimlendirme: ikili birimler (1 KB = 1024 bayt, Görev Yöneticisi gibi).
/// Kültür dışarıdan gelir: ondalık ayracı, yüzde yazımı ve "/sn" kısaltması uygulama diline uyar.
pub struct SystemFormat;

impl SystemFormat {
    /// Hız: "512 B/sn", "360 KB/sn", "2,5 MB/sn", "1,2 GB/sn". 10'dan küçük değerde bir ondalık.
    pub fn speed(bytes_per_second: i64, culture: &Culture) -> String {
        let (value, unit) = scale(bytes_per_second.max(0));
        number(value, culture, true, 10) + " " + unit + per_second(culture)
    }

    /// Hız, seçilen birimle: bayt seçiliyse [`Self::speed`] (ikili, KB/sn); bit seçiliyse ondalık
    /// ön ekli bit: "800 b/sn", "360 kb/sn", "2,5 Mb/sn", "1,2 Gb/sn" (1 kb = 1000 bit, ağ hızı geleneği).
    /// (C#: `Speed(long, NetworkSpeedUnit, CultureInfo)` aşırı yüklemesi.)
    pub fn speed_with_unit(bytes_per_second: i64, unit: NetworkSpeedUnit, culture: &Culture) -> String {
        if unit != NetworkSpeedUnit::Bits {
            return Self::speed(bytes_per_second, culture);
        }

        let mut bits = UnitConversion::network_speed(bytes_per_second.max(0) as f64, unit);
        const UNITS: [&str; 5] = ["b", "kb", "Mb", "Gb", "Tb"];
        let mut i = 0;
        while bits >= 1000.0 && i < UNITS.len() - 1 {
            bits /= 1000.0;
            i += 1;
        }

        if text::round(bits) >= 1000.0 && i < UNITS.len() - 1 {
            bits /= 1000.0;
            i += 1;
        }

        number(bits, culture, true, 10) + " " + UNITS[i] + per_second(culture)
    }

    /// Bellek: "9,2 / 16 GB" (kullanılan her zaman bir ondalık, toplam tam sayıysa ondalıksız).
    pub fn memory(used_bytes: i64, total_bytes: i64, culture: &Culture) -> String {
        const GB: f64 = 1024.0 * 1024.0 * 1024.0;
        let used = used_bytes.max(0) as f64 / GB;
        let total = total_bytes.max(0) as f64 / GB;
        to_string_fixed(used, 1, culture) + " / " + &number(total, culture, true, 0) + " GB"
    }

    /// Tek bellek değeri: "128 MB", "1,4 GB".
    pub fn size(bytes: i64, culture: &Culture) -> String {
        let (value, unit) = scale(bytes.max(0));
        number(value, culture, true, 10) + " " + unit
    }

    /// Yüzde: Türkçede "%42", diğer dillerde "42%". Girdi 0-100 aralığına kısılır, yuvarlanır.
    pub fn percent(percent: f64, culture: &Culture) -> String {
        // MidpointRounding.AwayFromZero = f64::round.
        let number = (text::clamp(percent, 0.0, 100.0).round() as i32).to_string();
        if AppLanguage::is_turkish(culture.name) { format!("%{number}") } else { format!("{number}%") }
    }
}

fn per_second(culture: &Culture) -> &'static str {
    if AppLanguage::is_turkish(culture.name) { "/sn" } else { "/s" }
}

fn scale(bytes: i64) -> (f64, &'static str) {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut i = 0;
    while value >= 1024.0 && i < UNITS.len() - 1 {
        value /= 1024.0;
        i += 1;
    }

    // Yuvarlama 1024'e denk gelirse bir üst birime geç ("1024 KB" yerine "1 MB").
    if text::round(value) >= 1024.0 && i < UNITS.len() - 1 {
        value /= 1024.0;
        i += 1;
    }

    (value, UNITS[i])
}

// 10'dan küçükse bir ondalık (2,5), değilse tam sayı (360). Sıfır ondalık kırpılır (2 MB).
// C# `alwaysOneDecimalUnder` varsayılanı 10.
fn number(value: f64, culture: &Culture, trim_zero: bool, always_one_decimal_under: i32) -> String {
    // MidpointRounding.AwayFromZero.
    let rounded = text::round_digits_away(value, 1);
    if rounded < always_one_decimal_under as f64 || (always_one_decimal_under == 0 && rounded % 1.0 != 0.0) {
        let text = to_string_fixed(rounded, 1, culture);
        // C# `text.EndsWith(sep + "0", Ordinal)` sonra `text[..^2]` (2 karakter keser).
        let zero_tail = format!("{}0", culture.decimal_separator);
        return if trim_zero && text.ends_with(&zero_tail) {
            let keep = text.chars().count() - 2;
            text.chars().take(keep).collect()
        } else {
            text
        };
    }

    to_string_fixed(value.round(), 0, culture)
}

/// `double.ToString("0")` / `ToString("0.0", culture)` karşılığı. .NET özel sayı biçimi değeri önce 15 anlamlı
/// basamağa yuvarlar, sonra o basamak dizisini yarım yukarı (>= 5) yuvarlayarak keser; Rust `{:.1}` ise ikilik
/// değerin tam açılımını yarım çifte yuvarlar (0.25 → "0.2", .NET "0.3"). Bu yüzden basamaklar elle işlenir.
/// (`units::WindSpeedFormatter` içindeki özel yardımcının aynısı; orası `pub` değil.)
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

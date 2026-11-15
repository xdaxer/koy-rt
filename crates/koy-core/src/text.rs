//! C# metin ve sayı davranışlarının birebir karşılıkları.
//!
//! Rust'ın varsayılanları birkaç yerde C#'tan ayrılır; algoritma korunsun diye bu yardımcılar kullanılır:
//! - `Math.Round(x)` bankacı yuvarlaması yapar (0.5 → 0, 1.5 → 2, 2.5 → 2). Rust `f64::round` sıfırdan uzağa
//!   yuvarlar. Karşılığı [`round`] / [`round_digits`]; `MidpointRounding.AwayFromZero` için `f64::round`.
//! - `StringComparison.OrdinalIgnoreCase` her karakteri değişmez büyük harfe çevirip karşılaştırır.
//! - `ToLower()` / `ToUpper()` kültürsüz çağrı mevcut kültürü kullanır; Koy'da kültüre bağlı olmaması
//!   gereken yerler `ToLowerInvariant` kullanır ([`to_lower_invariant`]). Türkçe kültür: [`to_lower_tr`].

use std::cmp::Ordering;

/// `Math.Round(double)`: yarımda çifte yuvarlar.
#[inline]
pub fn round(x: f64) -> f64 {
    x.round_ties_even()
}

/// `Math.Round(double, digits)`: yarımda çifte yuvarlar. .NET'in yaklaşımıyla aynı: 10^digits ile ölçekle,
/// yuvarla, böl.
pub fn round_digits(x: f64, digits: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let power = 10f64.powi(digits);
    let scaled = x * power;
    if !scaled.is_finite() {
        return x;
    }
    scaled.round_ties_even() / power
}

/// `Math.Round(double, digits, MidpointRounding.AwayFromZero)`.
pub fn round_digits_away(x: f64, digits: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let power = 10f64.powi(digits);
    (x * power).round() / power
}

/// `Math.Clamp(value, min, max)`: C# min > max ise istisna atar; burada da panikler.
pub fn clamp<T: PartialOrd>(value: T, min: T, max: T) -> T {
    assert!(!(min > max), "'{}' cannot be greater than {}.", "min", "max");
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

fn upper_char(c: char) -> char {
    // Basit eşleme: tek karaktere dönüşmeyen (ß → SS gibi) karakterler olduğu gibi kalır (.NET ile aynı).
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

fn lower_char(c: char) -> char {
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// `string.Equals(a, b, StringComparison.OrdinalIgnoreCase)`.
pub fn eq_ignore_case(a: &str, b: &str) -> bool {
    let mut x = a.chars();
    let mut y = b.chars();
    loop {
        match (x.next(), y.next()) {
            (None, None) => return true,
            (Some(c), Some(d)) => {
                if c != d && upper_char(c) != upper_char(d) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

/// `string.Compare(a, b, StringComparison.OrdinalIgnoreCase)`.
pub fn cmp_ignore_case(a: &str, b: &str) -> Ordering {
    a.chars().map(upper_char).cmp(b.chars().map(upper_char))
}

/// `a.Contains(b, StringComparison.OrdinalIgnoreCase)`.
pub fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    find_ignore_case(haystack, needle).is_some()
}

/// `a.IndexOf(b, StringComparison.OrdinalIgnoreCase)`: bayt konumu döner (C#'ta karakter konumu).
pub fn find_ignore_case(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let n: Vec<char> = needle.chars().map(upper_char).collect();
    let h: Vec<(usize, char)> = haystack.char_indices().map(|(i, c)| (i, upper_char(c))).collect();
    if n.len() > h.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&s| h[s..s + n.len()].iter().map(|p| p.1).eq(n.iter().copied())).map(|s| h[s].0)
}

/// `a.StartsWith(b, StringComparison.OrdinalIgnoreCase)`.
pub fn starts_with_ignore_case(s: &str, prefix: &str) -> bool {
    let mut it = s.chars();
    for p in prefix.chars() {
        match it.next() {
            Some(c) if c == p || upper_char(c) == upper_char(p) => {}
            _ => return false,
        }
    }
    true
}

/// `a.EndsWith(b, StringComparison.OrdinalIgnoreCase)`.
pub fn ends_with_ignore_case(s: &str, suffix: &str) -> bool {
    let mut it = s.chars().rev();
    for p in suffix.chars().rev() {
        match it.next() {
            Some(c) if c == p || upper_char(c) == upper_char(p) => {}
            _ => return false,
        }
    }
    true
}

/// `ToLowerInvariant()`: karakter başına basit eşleme (uzunluk değişmez).
pub fn to_lower_invariant(s: &str) -> String {
    s.chars().map(lower_char).collect()
}

/// `ToUpperInvariant()`.
pub fn to_upper_invariant(s: &str) -> String {
    s.chars().map(upper_char).collect()
}

/// `ToLower(new CultureInfo("tr-TR"))`: I → ı, İ → i.
pub fn to_lower_tr(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'I' => 'ı',
            'İ' => 'i',
            _ => lower_char(c),
        })
        .collect()
}

/// `ToUpper(new CultureInfo("tr-TR"))`: i → İ, ı → I.
pub fn to_upper_tr(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'i' => 'İ',
            'ı' => 'I',
            _ => upper_char(c),
        })
        .collect()
}

/// `string.IsNullOrWhiteSpace` (null yerine `None`).
pub fn is_null_or_white_space(s: Option<&str>) -> bool {
    s.is_none_or(|v| v.chars().all(char::is_whitespace))
}

/// `char.IsWhiteSpace` ile `Trim()`: Rust `str::trim` de Unicode boşluklarını kırpar; ikisi aynı kümeyi kullanır.
pub fn trim(s: &str) -> &str {
    s.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bankers_rounding_matches_dotnet() {
        assert_eq!(round(0.5), 0.0);
        assert_eq!(round(1.5), 2.0);
        assert_eq!(round(2.5), 2.0);
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(round_digits(1.25, 1), 1.2);
        assert_eq!(round_digits_away(1.25, 1), 1.3);
    }

    #[test]
    fn ordinal_ignore_case() {
        assert!(eq_ignore_case("Koy.App", "koy.app"));
        assert!(!eq_ignore_case("a", "ab"));
        assert!(contains_ignore_case("Hello World", "WORLD"));
        assert_eq!(find_ignore_case("abcABC", "Bc"), Some(1));
        assert!(starts_with_ignore_case("Chrome.exe", "chrome"));
        assert!(ends_with_ignore_case("Chrome.EXE", ".exe"));
        assert_eq!(cmp_ignore_case("abc", "ABD"), Ordering::Less);
    }

    #[test]
    fn turkish_casing() {
        assert_eq!(to_lower_tr("IŞIK İZMİR"), "ışık izmir");
        assert_eq!(to_upper_tr("istanbul ılık"), "İSTANBUL ILIK");
        assert_eq!(to_lower_invariant("ABC"), "abc");
    }
}

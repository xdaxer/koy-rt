use super::decimal::{group_separator, Decimal};
use crate::clock::Culture;
use crate::text;

/// Dört işlem, üs, parantez ve temel işlevleri (sqrt, sin...) hesaplayan küçük ayrıştırıcı. Yazılan metin kod gibi
/// çalıştırılmaz; uzunluk ve iç içe parantez sayısı sınırlıdır.
///
/// Sayılar C#'taki gibi `decimal` ([`Decimal`]) ile hesaplanır; `OverflowException` / `DivideByZeroException`
/// yolları `None` (sonuç yok) olur.
pub struct ExpressionCalculator;

const MAX_DEPTH: i32 = 24;
const MAX_EXPONENT: i32 = 1024;

impl ExpressionCalculator {
    pub const MAX_LENGTH: usize = 128;

    /// Hesap gibi mi görünüyor: rakam ve işlem, işlev çağrısı ya da 0x. Tek başına "123" hesap değildir.
    pub fn looks_like_expression(text: &str) -> bool {
        let body = Self::body(text);
        let len = body.chars().count();
        if len == 0 || len > Self::MAX_LENGTH || !body.chars().any(|c| c.is_ascii_digit()) {
            return false;
        }

        body.chars().skip(1).any(|c| matches!(c, '+' | '-' | '*' | '/' | '^' | '×' | '÷' | 'x' | 'X' | '('))
    }

    /// Sonucu ekran ve pano biçiminde verir. "hex=255", "bin=5", "oct=8" sonucu o tabanda yazar.
    /// Başarılıysa `Some((display, copy))`.
    pub fn try_calculate(text: &str, culture: &Culture) -> Option<(String, String)> {
        let mut body = text.trim().to_string();
        let mut radix = 0u32;
        for (prefix, value) in [("hex=", 16u32), ("bin=", 2), ("oct=", 8)] {
            if text::starts_with_ignore_case(&body, prefix) {
                radix = value;
                body = format!("={}", body.chars().skip(prefix.chars().count()).collect::<String>());
                break;
            }
        }

        // Taban öneki varsa tek sayı da hesaptır ("hex=255").
        let result = if radix != 0 { Self::try_parse_body(&body, culture)? } else { Self::try_evaluate(&body, culture)? };

        if radix != 0 {
            if !result.is_integer() || result.is_negative() {
                return None;
            }
            // `result > long.MaxValue`
            let n = result.to_i64()?;

            let text = match radix {
                16 => format!("0x{}", format!("{n:x}").to_uppercase()),
                2 => format!("0b{n:b}"),
                _ => format!("0{n:o}"),
            };
            return Some((text.clone(), text));
        }

        Some((Self::format_for_display(&result, culture), Self::format_for_copy(&result, culture)))
    }

    pub fn try_evaluate(text: &str, culture: &Culture) -> Option<Decimal> {
        if !Self::looks_like_expression(text) {
            return None;
        }
        Self::try_parse_body(text, culture)
    }

    fn try_parse_body(text: &str, culture: &Culture) -> Option<Decimal> {
        let body = Self::body(text);
        let len = body.chars().count();
        if len == 0 || len > Self::MAX_LENGTH {
            return None;
        }

        Parser::new(&body, culture).try_parse()
    }

    /// Ekranda gösterilen sonuç (binlik ayırıcılı): `ToString("#,0.##########", culture)`.
    pub fn format_for_display(value: &Decimal, culture: &Culture) -> String {
        value.format(true, 10, culture)
    }

    /// Panoya kopyalanan sonuç (ayırıcısız, başka yere yapıştırılabilir): `ToString("0.##########", culture)`.
    pub fn format_for_copy(value: &Decimal, culture: &Culture) -> String {
        value.format(false, 10, culture)
    }

    fn body(text: &str) -> String {
        let mut trimmed = text.trim();
        if let Some(rest) = trimmed.strip_prefix('=') {
            trimmed = rest.trim();
        }
        match trimmed.strip_suffix('=') {
            Some(rest) => rest.trim().to_string(),
            None => trimmed.to_string(),
        }
    }
}

struct Parser {
    text: Vec<char>,
    group_separator: String,
    position: usize,
    depth: i32,
}

impl Parser {
    fn new(text: &str, culture: &Culture) -> Self {
        Parser { text: text.chars().collect(), group_separator: group_separator(culture).to_string(), position: 0, depth: 0 }
    }

    fn try_parse(&mut self) -> Option<Decimal> {
        let value = self.try_expression()?;
        self.skip_spaces();
        if self.position == self.text.len() { Some(value) } else { None }
    }

    fn try_expression(&mut self) -> Option<Decimal> {
        let mut value = self.try_term()?;

        loop {
            let op = self.peek();
            if op != '+' && op != '-' {
                return Some(value);
            }

            self.position += 1;
            let right = self.try_term()?;

            value = if op == '+' { value.add(&right)? } else { value.sub(&right)? };
        }
    }

    fn try_term(&mut self) -> Option<Decimal> {
        let mut value = self.try_unary()?;

        loop {
            let op = self.peek();
            if !matches!(op, '*' | '/' | '×' | '÷' | 'x' | 'X') {
                return Some(value);
            }

            self.position += 1;
            let right = self.try_unary()?;

            if op == '/' || op == '÷' {
                if right.is_zero() {
                    return None;
                }
                value = value.div(&right)?;
            } else {
                value = value.mul(&right)?;
            }
        }
    }

    fn try_unary(&mut self) -> Option<Decimal> {
        let op = self.peek();
        if op != '-' && op != '+' {
            return self.try_power();
        }

        if !self.enter() {
            return None;
        }

        self.position += 1;
        let value = self.try_unary();
        self.depth -= 1;
        let value = value?;
        Some(if op == '-' { value.negate() } else { value })
    }

    fn try_power(&mut self) -> Option<Decimal> {
        let value = self.try_primary()?;

        if self.peek() != '^' {
            return Some(value);
        }

        self.position += 1;
        if !self.enter() {
            return None;
        }

        let exponent = self.try_unary();
        self.depth -= 1;
        Self::try_raise(&value, &exponent?)
    }

    fn try_raise(value: &Decimal, exponent: &Decimal) -> Option<Decimal> {
        if !exponent.is_integer() {
            let power = value.to_f64().powf(exponent.to_f64());
            if power.is_nan() || power.is_infinite() {
                return None;
            }

            return Decimal::from_f64(power);
        }

        // Math.Abs(exponent) > MaxExponent
        let abs_exponent = if exponent.is_negative() { exponent.negate() } else { *exponent };
        if abs_exponent.cmp_value(&Decimal::from_i64(MAX_EXPONENT as i64)) == std::cmp::Ordering::Greater {
            return None;
        }

        let count = abs_exponent.to_i64()?;
        let mut result = Decimal::ONE;
        for _ in 0..count {
            result = result.mul(value)?;
        }

        if !exponent.is_negative() {
            return Some(result);
        }

        if result.is_zero() {
            return None;
        }

        Decimal::ONE.div(&result)
    }

    fn try_primary(&mut self) -> Option<Decimal> {
        let next = self.peek();
        if next.is_ascii_alphabetic() {
            return self.try_function();
        }

        if next != '(' {
            return self.try_number();
        }

        if !self.enter() {
            return None;
        }

        self.position += 1;
        let value = self.try_expression();
        self.depth -= 1;
        let value = value?;
        if self.peek() != ')' {
            return None;
        }

        self.position += 1;
        Some(value)
    }

    fn try_function(&mut self) -> Option<Decimal> {
        let start = self.position;
        while self.position < self.text.len() && self.text[self.position].is_ascii_alphanumeric() {
            self.position += 1;
        }

        let name = text::to_lower_invariant(&self.text[start..self.position].iter().collect::<String>());
        if name == "pi" {
            return Decimal::from_f64(std::f64::consts::PI);
        }

        if name == "e" {
            return Decimal::from_f64(std::f64::consts::E);
        }

        if self.peek() != '(' || !self.enter() {
            return None;
        }

        self.position += 1;
        let argument = self.try_expression();
        self.depth -= 1;
        let argument = argument?;
        if self.peek() != ')' {
            return None;
        }

        self.position += 1;
        Self::try_apply(&name, &argument)
    }

    fn try_apply(name: &str, argument: &Decimal) -> Option<Decimal> {
        let x = argument.to_f64();
        let result = match name {
            "sqrt" => x.sqrt(),
            "abs" => x.abs(),
            "sin" => x.sin(),
            "cos" => x.cos(),
            "tan" => x.tan(),
            "ln" => x.ln(),
            "log" => x.log10(),
            "exp" => x.exp(),
            "floor" => x.floor(),
            "ceil" => x.ceil(),
            "round" => text::round(x),
            _ => return None,
        };

        if result.is_nan() || result.is_infinite() {
            return None;
        }

        Decimal::from_f64(result)
    }

    fn try_hex_number(&mut self) -> Option<Decimal> {
        let start = self.position + 2;
        let mut end = start;
        while end < self.text.len() && self.text[end].is_ascii_hexdigit() {
            end += 1;
        }

        if end == start {
            return None;
        }

        // long.TryParse(AllowHexSpecifier): en çok 16 hane (baştaki sıfırlar sayılmaz); üst bit doluysa negatif.
        let digits: String = self.text[start..end].iter().collect();
        let trimmed = digits.trim_start_matches('0');
        if trimmed.len() > 16 {
            return None;
        }
        let number = if trimmed.is_empty() { 0 } else { u64::from_str_radix(trimmed, 16).ok()? as i64 };

        self.position = end;
        Some(Decimal::from_i64(number))
    }

    fn try_number(&mut self) -> Option<Decimal> {
        self.skip_spaces();

        if self.position + 2 < self.text.len()
            && self.text[self.position] == '0'
            && matches!(self.text[self.position + 1], 'x' | 'X')
            && self.text[self.position + 2].is_ascii_hexdigit()
        {
            return self.try_hex_number();
        }

        let mut digits = String::new();
        let mut has_decimal = false;
        while self.position < self.text.len() {
            let c = self.text[self.position];
            if c.is_ascii_digit() {
                digits.push(c);
                self.position += 1;
                continue;
            }

            if (c != '.' && c != ',') || !self.is_digit_at(self.position + 1) {
                break;
            }

            // Yerel binlik ayırıcı ancak ardından tam üç rakam gelirse binliktir: Türkçede "1.000" bin, "3.5" üç buçuk.
            self.position += 1;
            if !has_decimal && !digits.is_empty() && self.is_group_separator(c) && self.is_group_of_three(self.position) {
                continue;
            }

            if has_decimal {
                return None;
            }

            has_decimal = true;
            digits.push('.');
        }

        if digits.is_empty() { None } else { Decimal::parse(&digits) }
    }

    fn is_group_separator(&self, c: char) -> bool {
        let mut it = self.group_separator.chars();
        matches!((it.next(), it.next()), (Some(g), None) if g == c)
    }

    fn is_group_of_three(&self, start: usize) -> bool {
        self.is_digit_at(start) && self.is_digit_at(start + 1) && self.is_digit_at(start + 2) && !self.is_digit_at(start + 3)
    }

    fn is_digit_at(&self, index: usize) -> bool {
        index < self.text.len() && self.text[index].is_ascii_digit()
    }

    fn enter(&mut self) -> bool {
        self.depth += 1;
        self.depth <= MAX_DEPTH
    }

    fn peek(&mut self) -> char {
        self.skip_spaces();
        if self.position < self.text.len() { self.text[self.position] } else { '\0' }
    }

    fn skip_spaces(&mut self) {
        while self.position < self.text.len() && self.text[self.position] == ' ' {
            self.position += 1;
        }
    }
}

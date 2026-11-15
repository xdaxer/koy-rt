//! .NET `System.Decimal` karşılığı (96 bit mantis, 0..28 ölçek). Yeni crate eklenemediği için küçük bir elle yazım:
//! `ExpressionCalculator` ("0,1+0,2" tam 0,3 olmalı) ve `UnitConverter.FormatNumber` ondalık aritmetiğe dayanır.
//!
//! Davranışlar .NET ile aynıdır: taşma `None` döner (C#'ta `OverflowException`), çarpma/toplama ölçeği büyütüp
//! sığmayınca en yakına çifte yuvarlayarak küçültür, bölme 28 basamağa kadar uzar ve sondaki sıfırları atar,
//! `(decimal)double` 15 anlamlı haneye yuvarlar.

use crate::clock::Culture;
use std::cmp::Ordering;

const MAX96: u128 = (1u128 << 96) - 1;

/// Küçük, işaretsiz büyük tam sayı (32 bitlik uzuvlar, küçük uçtan). Yalnız bu dosyanın ihtiyacı kadar.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Big(Vec<u32>);

impl Big {
    fn from_u128(mut v: u128) -> Big {
        let mut limbs = Vec::new();
        while v != 0 {
            limbs.push(v as u32);
            v >>= 32;
        }
        Big(limbs)
    }

    fn trim(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }

    fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    fn cmp(&self, other: &Big) -> Ordering {
        if self.0.len() != other.0.len() {
            return self.0.len().cmp(&other.0.len());
        }
        for i in (0..self.0.len()).rev() {
            if self.0[i] != other.0[i] {
                return self.0[i].cmp(&other.0[i]);
            }
        }
        Ordering::Equal
    }

    fn add(&self, other: &Big) -> Big {
        let n = self.0.len().max(other.0.len());
        let mut out = Vec::with_capacity(n + 1);
        let mut carry = 0u64;
        for i in 0..n {
            let s = *self.0.get(i).unwrap_or(&0) as u64 + *other.0.get(i).unwrap_or(&0) as u64 + carry;
            out.push(s as u32);
            carry = s >> 32;
        }
        if carry != 0 {
            out.push(carry as u32);
        }
        Big(out)
    }

    /// `self - other` (self >= other).
    fn sub(&self, other: &Big) -> Big {
        let mut out = Vec::with_capacity(self.0.len());
        let mut borrow = 0i64;
        for i in 0..self.0.len() {
            let mut d = self.0[i] as i64 - *other.0.get(i).unwrap_or(&0) as i64 - borrow;
            if d < 0 {
                d += 1 << 32;
                borrow = 1;
            } else {
                borrow = 0;
            }
            out.push(d as u32);
        }
        let mut b = Big(out);
        b.trim();
        b
    }

    fn mul(&self, other: &Big) -> Big {
        if self.is_zero() || other.is_zero() {
            return Big(Vec::new());
        }
        let mut out = vec![0u32; self.0.len() + other.0.len()];
        for (i, &a) in self.0.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in other.0.iter().enumerate() {
                let t = out[i + j] as u64 + a as u64 * b as u64 + carry;
                out[i + j] = t as u32;
                carry = t >> 32;
            }
            let mut k = i + other.0.len();
            while carry != 0 {
                let t = out[k] as u64 + carry;
                out[k] = t as u32;
                carry = t >> 32;
                k += 1;
            }
        }
        let mut b = Big(out);
        b.trim();
        b
    }

    fn mul_small(&self, m: u32) -> Big {
        self.mul(&Big::from_u128(m as u128))
    }

    /// Küçük bölenle böler; kalanı döner.
    fn div_small(&mut self, d: u32) -> u32 {
        let mut rem = 0u64;
        for i in (0..self.0.len()).rev() {
            let cur = (rem << 32) | self.0[i] as u64;
            self.0[i] = (cur / d as u64) as u32;
            rem = cur % d as u64;
        }
        self.trim();
        rem as u32
    }

    fn bit_len(&self) -> usize {
        match self.0.last() {
            None => 0,
            Some(&top) => (self.0.len() - 1) * 32 + (32 - top.leading_zeros() as usize),
        }
    }

    fn bit(&self, i: usize) -> bool {
        self.0.get(i / 32).is_some_and(|l| (l >> (i % 32)) & 1 == 1)
    }

    fn shl1_or(&mut self, bit: bool) {
        let mut carry = bit as u32;
        for l in self.0.iter_mut() {
            let next = *l >> 31;
            *l = (*l << 1) | carry;
            carry = next;
        }
        if carry != 0 {
            self.0.push(carry);
        }
    }

    fn set_bit(&mut self, i: usize) {
        while self.0.len() <= i / 32 {
            self.0.push(0);
        }
        self.0[i / 32] |= 1 << (i % 32);
    }

    /// (bölüm, kalan); bölen sıfır olmamalı.
    fn divmod(&self, d: &Big) -> (Big, Big) {
        let mut q = Big(Vec::new());
        let mut r = Big(Vec::new());
        for i in (0..self.bit_len()).rev() {
            r.shl1_or(self.bit(i));
            r.trim();
            if r.cmp(d) != Ordering::Less {
                r = r.sub(d);
                q.set_bit(i);
            }
        }
        q.trim();
        (q, r)
    }

    fn pow10(n: u32) -> Big {
        let mut b = Big::from_u128(1);
        for _ in 0..n {
            b = b.mul_small(10);
        }
        b
    }

    fn to_u128(&self) -> Option<u128> {
        if self.0.len() > 4 {
            return None;
        }
        let mut v = 0u128;
        for (i, &l) in self.0.iter().enumerate() {
            v |= (l as u128) << (32 * i);
        }
        Some(v)
    }

    fn max96() -> Big {
        Big::from_u128(MAX96)
    }

    fn is_odd(&self) -> bool {
        self.0.first().is_some_and(|l| l & 1 == 1)
    }

    fn inc(&self) -> Big {
        self.add(&Big::from_u128(1))
    }
}

/// `System.Decimal`. Değer: `(-1)^neg * mant / 10^scale`; mantis en çok 2^96-1, ölçek 0..28.
#[derive(Debug, Clone, Copy)]
pub struct Decimal {
    neg: bool,
    mant: u128,
    scale: u8,
}

impl Decimal {
    pub const ZERO: Decimal = Decimal { neg: false, mant: 0, scale: 0 };
    pub const ONE: Decimal = Decimal { neg: false, mant: 1, scale: 0 };
    /// `decimal.MaxValue` (7922816251426433759354395033.5 değil, 79228162514264337593543950335).
    pub const MAX_MANTISSA: u128 = MAX96;

    pub fn from_i64(v: i64) -> Decimal {
        Decimal { neg: v < 0, mant: v.unsigned_abs() as u128, scale: 0 }
    }

    pub fn is_zero(&self) -> bool {
        self.mant == 0
    }

    pub fn is_negative(&self) -> bool {
        self.neg && self.mant != 0
    }

    /// Tamsayı mı (`value == decimal.Truncate(value)`).
    pub fn is_integer(&self) -> bool {
        self.truncate().cmp_value(self) == Ordering::Equal
    }

    /// Ölçek, değeri değiştirmeden sondaki sıfırlar atılarak.
    fn big(&self) -> Big {
        Big::from_u128(self.mant)
    }

    /// Büyük sonuçtan 96 bit ve ölçek 28 sınırına indirger; taşarsa `None`.
    fn normalize(neg: bool, mut cur: Big, mut scale: i32) -> Option<Decimal> {
        let max = Big::max96();
        loop {
            let mut sticky = false;
            let mut last_rem = 0u32;
            let mut dropped = false;
            while scale > 28 || cur.cmp(&max) == Ordering::Greater {
                if scale <= 0 {
                    return None;
                }
                let rem = cur.div_small(10);
                if last_rem != 0 {
                    sticky = true;
                }
                last_rem = rem;
                scale -= 1;
                dropped = true;
            }
            if dropped && (last_rem > 5 || (last_rem == 5 && (sticky || cur.is_odd()))) {
                cur = cur.inc();
                if cur.cmp(&max) == Ordering::Greater {
                    continue;
                }
            }
            let mant = cur.to_u128()?;
            let neg = neg && mant != 0;
            return Some(Decimal { neg, mant, scale: scale as u8 });
        }
    }

    fn aligned(a: &Decimal, b: &Decimal) -> (Big, Big, i32) {
        let scale = a.scale.max(b.scale) as i32;
        let x = a.big().mul(&Big::pow10((scale - a.scale as i32) as u32));
        let y = b.big().mul(&Big::pow10((scale - b.scale as i32) as u32));
        (x, y, scale)
    }

    /// `a + b` (`checked`: taşarsa `None`).
    pub fn add(&self, other: &Decimal) -> Option<Decimal> {
        let (x, y, scale) = Self::aligned(self, other);
        if self.neg == other.neg {
            return Self::normalize(self.neg, x.add(&y), scale);
        }
        match x.cmp(&y) {
            Ordering::Equal => Some(Decimal { neg: false, mant: 0, scale: scale as u8 }),
            Ordering::Greater => Self::normalize(self.neg, x.sub(&y), scale),
            Ordering::Less => Self::normalize(other.neg, y.sub(&x), scale),
        }
    }

    pub fn sub(&self, other: &Decimal) -> Option<Decimal> {
        self.add(&other.negate())
    }

    pub fn negate(&self) -> Decimal {
        Decimal { neg: !self.neg && self.mant != 0, ..*self }
    }

    pub fn mul(&self, other: &Decimal) -> Option<Decimal> {
        let big = self.big().mul(&other.big());
        Self::normalize(self.neg != other.neg, big, self.scale as i32 + other.scale as i32)
    }

    /// `a / b`; bölen sıfırsa ya da sonuç taşarsa `None`.
    pub fn div(&self, other: &Decimal) -> Option<Decimal> {
        if other.mant == 0 {
            return None;
        }
        let neg = self.neg != other.neg;
        let shift = (other.scale as i32 - self.scale as i32).max(0) as u32;
        let scale0 = (self.scale as i32 - other.scale as i32).max(0);
        let num = self.big().mul(&Big::pow10(shift));
        let den = other.big();
        let (mut q, mut r) = num.divmod(&den);
        let mut scale = scale0;
        let max = Big::max96();
        while !r.is_zero() && scale < 28 {
            let (d, r2) = r.mul_small(10).divmod(&den);
            let nq = q.mul_small(10).add(&d);
            if nq.cmp(&max) == Ordering::Greater {
                break;
            }
            q = nq;
            r = r2;
            scale += 1;
        }
        if !r.is_zero() {
            let twice = r.mul_small(2);
            match twice.cmp(&den) {
                Ordering::Greater => q = q.inc(),
                Ordering::Equal if q.is_odd() => q = q.inc(),
                _ => {}
            }
        }
        // Sondaki sıfırlar atılır (en az doğal ölçeğe kadar).
        while scale > scale0 {
            let mut t = q.clone();
            if t.div_small(10) != 0 {
                break;
            }
            q = t;
            scale -= 1;
        }
        Self::normalize(neg, q, scale)
    }

    /// Sayısal karşılaştırma (ölçekten bağımsız).
    pub fn cmp_value(&self, other: &Decimal) -> Ordering {
        let a_neg = self.is_negative();
        let b_neg = other.is_negative();
        if a_neg != b_neg {
            return if a_neg { Ordering::Less } else { Ordering::Greater };
        }
        let (x, y, _) = Self::aligned(self, other);
        let o = x.cmp(&y);
        if a_neg { o.reverse() } else { o }
    }

    /// `decimal.Truncate`: kesri atar.
    pub fn truncate(&self) -> Decimal {
        if self.scale == 0 {
            return *self;
        }
        let mut b = self.big();
        for _ in 0..self.scale {
            b.div_small(10);
        }
        let mant = b.to_u128().unwrap_or(0);
        Decimal { neg: self.neg && mant != 0, mant, scale: 0 }
    }

    /// `Math.Round(value, places, mode)`: `away` true ise `MidpointRounding.AwayFromZero`, değilse çifte (varsayılan).
    pub fn round(&self, places: u32, away: bool) -> Decimal {
        if self.scale as u32 <= places {
            return *self;
        }
        let drop = self.scale as u32 - places;
        let mut b = self.big();
        let mut last = 0u32;
        let mut sticky = false;
        for _ in 0..drop {
            if last != 0 {
                sticky = true;
            }
            last = b.div_small(10);
        }
        let round_up = if away { last >= 5 } else { last > 5 || (last == 5 && (sticky || b.is_odd())) };
        if round_up {
            b = b.inc();
        }
        let mant = b.to_u128().unwrap_or(0);
        Decimal { neg: self.neg && mant != 0, mant, scale: places as u8 }
    }

    /// `(long)value`: kesirsiz kısım; sığmazsa `None` (C#'ta taşma).
    pub fn to_i64(&self) -> Option<i64> {
        let t = self.truncate();
        if t.neg {
            if t.mant <= (i64::MAX as u128) + 1 { Some((t.mant as i128).wrapping_neg() as i64) } else { None }
        } else if t.mant <= i64::MAX as u128 {
            Some(t.mant as i64)
        } else {
            None
        }
    }

    /// `(double)value` (.NET: mantis çift sayıya, sonra 10^ölçeğe bölünür).
    pub fn to_f64(&self) -> f64 {
        let m = self.mant as f64;
        let v = if self.scale == 0 { m } else { m / pow10_f64(self.scale as i32) };
        if self.neg { -v } else { v }
    }

    /// `(decimal)double`: 15 anlamlı haneye yuvarlar; sığmazsa (7,9e28 üstü) `None` (C#'ta `OverflowException`).
    pub fn from_f64(x: f64) -> Option<Decimal> {
        if !x.is_finite() {
            return None;
        }
        let neg = x < 0.0;
        let a = x.abs();
        if a < 1e-28 {
            return Some(Decimal::ZERO);
        }
        if a >= 7.922816251426434e28 {
            return None;
        }
        let text = format!("{:.14e}", a);
        let (mant_text, exp_text) = text.split_once('e')?;
        let exp: i32 = exp_text.parse().ok()?;
        let digits: String = mant_text.chars().filter(|c| c.is_ascii_digit()).collect();
        let mant: u128 = digits.parse().ok()?;
        // değer = mant * 10^(exp - 14)
        let mut scale = 14 - exp;
        let mut big = Big::from_u128(mant);
        if scale < 0 {
            big = big.mul(&Big::pow10((-scale) as u32));
            scale = 0;
        }
        // Sondaki sıfırları at.
        while scale > 0 {
            let mut t = big.clone();
            if t.div_small(10) != 0 {
                break;
            }
            big = t;
            scale -= 1;
        }
        Self::normalize(neg, big, scale)
    }

    /// `decimal.TryParse(text, NumberStyles.AllowDecimalPoint, InvariantCulture)`: yalnız ASCII rakam ve en çok bir '.'.
    pub fn parse(text: &str) -> Option<Decimal> {
        let (int_part, frac_part) = match text.split_once('.') {
            Some((i, f)) => (i, f),
            None => (text, ""),
        };
        if frac_part.contains('.') || !int_part.chars().chain(frac_part.chars()).all(|c| c.is_ascii_digit()) {
            return None;
        }
        if int_part.is_empty() && frac_part.is_empty() {
            return None;
        }
        let mut big = Big::from_u128(0);
        for c in int_part.chars().chain(frac_part.chars()) {
            big = big.mul_small(10).add(&Big::from_u128(c.to_digit(10).unwrap() as u128));
        }
        Self::normalize(false, big, frac_part.len() as i32)
    }

    /// `ToString("#,0.####")` / `("0.####")` türü özel biçimler: en çok `max_decimals` ondalık (sıfırdan uzağa
    /// yuvarlanır, sondaki sıfırlar atılır), tam kısım en az bir hane, isteğe bağlı üçlü gruplama.
    /// Yuvarlanınca sıfır olan değer "-0" değil "0" yazılır (decimal için .NET böyle yapar).
    pub fn format(&self, grouping: bool, max_decimals: u32, culture: &Culture) -> String {
        let r = self.round(max_decimals, true);
        let mut digits = r.mant.to_string();
        let scale = r.scale as usize;
        while digits.len() <= scale {
            digits.insert(0, '0');
        }
        let (int_digits, frac_digits) = digits.split_at(digits.len() - scale);
        let frac = frac_digits.trim_end_matches('0');
        let mut int_text = String::new();
        if grouping {
            let sep = group_separator(culture);
            for (i, c) in int_digits.chars().enumerate() {
                if i > 0 && (int_digits.len() - i) % 3 == 0 {
                    int_text.push_str(sep);
                }
                int_text.push(c);
            }
        } else {
            int_text.push_str(int_digits);
        }
        let mut out = String::new();
        if r.neg && r.mant != 0 {
            out.push('-');
        }
        out.push_str(&int_text);
        if !frac.is_empty() {
            out.push_str(culture.decimal_separator);
            out.push_str(frac);
        }
        out
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Decimal) -> bool {
        self.cmp_value(other) == Ordering::Equal
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Decimal) -> Option<Ordering> {
        Some(self.cmp_value(other))
    }
}

fn pow10_f64(n: i32) -> f64 {
    format!("1e{n}").parse().unwrap()
}

/// `CultureInfo.NumberFormat.NumberGroupSeparator`: gömülü kültürlerde ondalık ayıracı "," olanlar "." kullanır.
pub fn group_separator(culture: &Culture) -> &'static str {
    if culture.decimal_separator == "," { "." } else { "," }
}

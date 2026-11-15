use crate::text;

/// sRGB renk (alfa yok; saydamlık ayrı tutulur). WCAG 2.x kontrast hesabı için.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// "#RGB", "#RRGGBB" ya da "#AARRGGBB" metnini çözer; alfa yok sayılır.
    pub fn try_parse(hex: Option<&str>) -> Option<Rgb> {
        let hex = hex?;
        if text::is_null_or_white_space(Some(hex)) {
            return None;
        }

        // C# dizgesi UTF-16'dır: uzunluk denetimleri kod birimiyle yapılır.
        let mut s: Vec<u16> = hex.trim().trim_start_matches('#').encode_utf16().collect();
        if s.len() == 3 {
            s = s.iter().flat_map(|&c| [c, c]).collect();
        }
        if s.len() == 8 {
            s.drain(..2);
        }
        if s.len() != 6 {
            return None;
        }
        let v = parse_hex_number(&s)?;
        Some(Rgb::new((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    /// Geçersizse `FormatException` yerine panik.
    pub fn parse(hex: &str) -> Rgb {
        match Rgb::try_parse(Some(hex)) {
            Some(c) => c,
            None => panic!("Geçersiz renk: {hex}"),
        }
    }
}

/// `int.TryParse(s, NumberStyles.HexNumber, InvariantCulture)`: baştaki ve sondaki boşluğa izin verir
/// (.NET'te boşluk: U+0009..U+000D ve U+0020), arası yalnız onaltılık rakam olmalı.
// C# ile aynı: altı kod birimlik metinde baştaki/sondaki boşluk geçerli sayılır ("#  12AB" çözülür).
fn parse_hex_number(units: &[u16]) -> Option<i32> {
    let is_white = |u: u16| (0x09..=0x0D).contains(&u) || u == 0x20;
    let mut start = 0;
    let mut end = units.len();
    while start < end && is_white(units[start]) {
        start += 1;
    }
    while end > start && is_white(units[end - 1]) {
        end -= 1;
    }
    if start == end {
        return None;
    }
    let mut value: i64 = 0;
    for &u in &units[start..end] {
        let digit = char::from_u32(u as u32)?.to_digit(16)?;
        value = value * 16 + digit as i64;
    }
    Some(value as i32)
}

/// HSL renk: H derece (0 ile 360 arası), S ve L 0 ile 1 arası.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Hsl {
    pub h: f64,
    pub s: f64,
    pub l: f64,
}

impl Hsl {
    pub const fn new(h: f64, s: f64, l: f64) -> Self {
        Hsl { h, s, l }
    }
}

pub struct ColorMath;

impl ColorMath {
    /// WCAG göreli parlaklık (0 siyah, 1 beyaz).
    pub fn luminance(c: Rgb) -> f64 {
        fn channel(v: u8) -> f64 {
            let s = v as f64 / 255.0;
            if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        }

        0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
    }

    /// WCAG kontrast oranı (1 ile 21 arası).
    pub fn contrast(a: Rgb, b: Rgb) -> f64 {
        let la = Self::luminance(a);
        let lb = Self::luminance(b);
        let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// `Contrast(string, string)` aşırı yüklemesi.
    pub fn contrast_hex(hex_a: &str, hex_b: &str) -> f64 {
        Self::contrast(Rgb::parse(hex_a), Rgb::parse(hex_b))
    }

    /// a'dan b'ye t oranında karışım (0 = a, 1 = b).
    pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
        let t = text::clamp(t, 0.0, 1.0);
        let m = |x: u8, y: u8| text::round(x as f64 + (y as f64 - x as f64) * t) as u8;
        Rgb::new(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b))
    }

    /// sRGB'yi HSL'e çevirir. Gri tonlarda H 0 ve S 0 döner.
    pub fn to_hsl(c: Rgb) -> Hsl {
        Self::to_hsl_bytes(c.r, c.g, c.b)
    }

    /// Bayt kanallarından doğrudan HSL (ayırma yapmaz; piksel döngüleri için).
    pub fn to_hsl_bytes(r8: u8, g8: u8, b8: u8) -> Hsl {
        let (r, g, b) = (r8 as f64 / 255.0, g8 as f64 / 255.0, b8 as f64 / 255.0);
        let max = r.max(g.max(b));
        let min = r.min(g.min(b));
        let l = (max + min) / 2.0;
        let d = max - min;
        if d <= 0.0 {
            return Hsl::new(0.0, 0.0, l);
        }

        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        Hsl::new(h * 60.0, s, l)
    }

    /// HSL'i sRGB'ye çevirir; H 360'a sarılır, S ve L 0..1'e kıskaçlanır.
    pub fn from_hsl(h: f64, s: f64, l: f64) -> Rgb {
        let h = ((h % 360.0) + 360.0) % 360.0;
        let s = text::clamp(s, 0.0, 1.0);
        let l = text::clamp(l, 0.0, 1.0);
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - (h / 60.0 % 2.0 - 1.0).abs());
        let m = l - c / 2.0;
        let (r, g, b) = match (h / 60.0) as i32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let to_byte = |v: f64| text::clamp(text::round(v * 255.0), 0.0, 255.0) as u8;
        Rgb::new(to_byte(r + m), to_byte(g + m), to_byte(b + m))
    }

    /// `FromHsl(Hsl)` aşırı yüklemesi.
    pub fn from_hsl_value(hsl: Hsl) -> Rgb {
        Self::from_hsl(hsl.h, hsl.s, hsl.l)
    }
}

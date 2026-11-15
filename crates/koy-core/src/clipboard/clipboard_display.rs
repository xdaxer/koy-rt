use crate::text;
use crate::time::{span_between, DateTimeOffset, TimeSpan};
use regex::Regex;
use std::sync::OnceLock;

/// Öğenin ne kadar önce kopyalandığı (arayüz birimi Strings.resx'ten biçimlendirir).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AgeUnit {
    #[default]
    Now,
    Minutes,
    Hours,
    Days,
}

fn rgb_parts() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)^rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*(?:,\s*(\d{1,3}%?|\d*\.\d+)\s*)?\)$").unwrap()
    })
}

/// Pano listesinin saf gösterim yardımcıları: önizleme, yaş, renk çözümleme. WPF yok.
pub struct ClipboardDisplay;

impl ClipboardDisplay {
    /// 1 dakikadan azı "şimdi", 1 saatten azı dakika, 1 günden azı saat, gerisi gün.
    pub fn age(copied_at: DateTimeOffset, now: DateTimeOffset) -> (AgeUnit, i32) {
        let span = span_between(now, copied_at);
        if span < TimeSpan::from_mins(1) {
            return (AgeUnit::Now, 0);
        }
        if span < TimeSpan::from_hours(1) {
            return (AgeUnit::Minutes, span.total_minutes() as i32);
        }
        if span < TimeSpan::from_days(1) {
            return (AgeUnit::Hours, span.total_hours() as i32);
        }
        (AgeUnit::Days, span.total_days() as i32)
    }

    /// Kartta gösterilecek ilk satırlar: boş satırlar atılır, her satırın başındaki girinti ve sondaki boşluk kırpılır,
    /// uzun satır `max_line_chars` karakterde "…" ile kesilir; en çok `max_lines` satır.
    /// C# varsayılanları: `max_lines = 3`, `max_line_chars = 90`. Uzunluk UTF-16 kod birimiyle sayılır.
    pub fn preview(text: Option<&str>, max_lines: i32, max_line_chars: i32) -> String {
        if text::is_null_or_white_space(text) {
            return String::new();
        }

        let max_lines = max_lines.max(0) as usize;
        let lines: Vec<String> = text
            .unwrap_or("")
            .split('\n')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .take(max_lines)
            .map(|l| {
                if l.encode_utf16().count() > max_line_chars as usize {
                    format!("{}…", take_utf16(l, max_line_chars as usize).trim_end())
                } else {
                    l.to_string()
                }
            })
            .collect();
        lines.join("\n")
    }

    /// "#RGB", "#RRGGBB", "#RRGGBBAA" (web sırası) ve "rgb()/rgba()" metnini bileşenlere çevirir.
    /// Başarılıysa `Some((r, g, b, a))`.
    pub fn try_parse_color(text: Option<&str>) -> Option<(u8, u8, u8, u8)> {
        let t = text?.trim();
        if t.is_empty() {
            return None;
        }

        if let Some(hex) = t.strip_prefix('#') {
            if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let h: Vec<char> = hex.chars().collect();
            return match h.len() {
                3 => Some((nibble(h[0]), nibble(h[1]), nibble(h[2]), 255)),
                6 | 8 => {
                    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
                    let a = if h.len() == 8 { byte(6) } else { 255 };
                    Some((byte(0), byte(2), byte(4), a))
                }
                _ => None,
            };
        }

        let m = rgb_parts().captures(t)?;
        let rv: i32 = m[1].parse().ok()?;
        let gv: i32 = m[2].parse().ok()?;
        let bv: i32 = m[3].parse().ok()?;
        if rv > 255 || gv > 255 || bv > 255 {
            return None;
        }
        let mut a = 255;
        if let Some(g) = m.get(4) {
            if let Some(alpha) = try_alpha(g.as_str()) {
                a = alpha;
            }
        }
        Some((rv as u8, gv as u8, bv as u8, a))
    }
}

/// `s[..n]` (UTF-16 kod birimi sayısıyla); vekil çiftin ortasına denk gelirse yarım çift atılır.
pub(super) fn take_utf16(s: &str, n: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let len = c.len_utf16();
        if used + len > n {
            break;
        }
        used += len;
        out.push(c);
    }
    out
}

fn try_alpha(text: &str) -> Option<u8> {
    let percent = text.ends_with('%');
    let raw = if percent { &text[..text.len() - 1] } else { text };
    let v: f64 = raw.parse().ok()?;
    // CSS: 0-1 arası kesir ya da yüzde.
    let fraction = if percent { v / 100.0 } else if v <= 1.0 { v } else { v / 255.0 };
    Some(text::round(text::clamp(fraction, 0.0, 1.0) * 255.0) as u8)
}

fn nibble(c: char) -> u8 {
    (c.to_digit(16).unwrap() * 17) as u8
}

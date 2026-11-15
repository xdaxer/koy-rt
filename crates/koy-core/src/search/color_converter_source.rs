use super::search_util::utf16_len;
use super::{
    ISearchShell, ISearchSource, SearchDragData, SearchDragKind, SearchGlyph, SearchQuery, SearchRelevance, SearchResult,
    SearchSourceBase, Translate,
};
use crate::appearance::{ColorMath, Hsl, Rgb};
use crate::text;
use regex::Regex;
use std::sync::{Arc, OnceLock};

/// Çözülmüş renk. `alpha` 0..255. `input_hsl`: HSL girdisinde kullanıcının yazdığı değer;
/// sekizli kanala yuvarlanınca kayan S/L geri hesaplanıp gösterilmesin diye saklanır.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedColor {
    pub color: Rgb,
    pub alpha: u8,
    pub input_hsl: Option<Hsl>,
}

impl ParsedColor {
    /// C# `ParsedColor(Color, Alpha, InputHsl = null)`: `input_hsl` zorunlu parametre.
    pub fn new(color: Rgb, alpha: u8, input_hsl: Option<Hsl>) -> Self {
        ParsedColor { color, alpha, input_hsl }
    }

    pub fn has_alpha(&self) -> bool {
        self.alpha != 255
    }

    pub fn to_hex(&self) -> String {
        if self.has_alpha() {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.alpha, self.color.r, self.color.g, self.color.b)
        } else {
            self.color.to_hex()
        }
    }

    pub fn to_rgb_text(&self) -> String {
        if self.has_alpha() {
            format!("rgba({}, {}, {}, {})", self.color.r, self.color.g, self.color.b, self.alpha_text())
        } else {
            format!("rgb({}, {}, {})", self.color.r, self.color.g, self.color.b)
        }
    }

    pub fn to_hsl_text(&self) -> String {
        let hsl = self.input_hsl.unwrap_or_else(|| ColorMath::to_hsl(self.color));
        let mut h = Self::whole(((hsl.h % 360.0) + 360.0) % 360.0);
        if h == "360" {
            h = "0".to_string();
        }
        let s = Self::whole(hsl.s * 100.0);
        let l = Self::whole(hsl.l * 100.0);
        if self.has_alpha() {
            format!("hsla({h}, {s}%, {l}%, {})", self.alpha_text())
        } else {
            format!("hsl({h}, {s}%, {l}%)")
        }
    }

    /// `(int)Math.Round(value, MidpointRounding.AwayFromZero)` (değişmez kültürle metin).
    fn whole(value: f64) -> String {
        (value.round() as i32).to_string()
    }

    /// `Math.Round(Alpha / 255.0, 2).ToString("0.##", Invariant)`.
    fn alpha_text(&self) -> String {
        let v = text::round_digits(self.alpha as f64 / 255.0, 2);
        let s = format!("{v:.2}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}

fn hex_pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)^#(?P<hex>[0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$").unwrap())
}

fn rgb_pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)^rgb\(\s*(?P<r>\d{1,3})\s*[,\s]\s*(?P<g>\d{1,3})\s*[,\s]\s*(?P<b>\d{1,3})\s*\)$").unwrap()
    })
}

fn hsl_pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)^hsl\(\s*(?P<h>[+-]?\d+(?:\.\d+)?)\s*(?:°|deg)?\s*[,\s]\s*(?P<s>\d+(?:\.\d+)?)\s*%?\s*[,\s]\s*(?P<l>\d+(?:\.\d+)?)\s*%?\s*\)$",
        )
        .unwrap()
    })
}

/// Renk girdisini çözer: #RGB, #RRGGBB, #AARRGGBB (Windows/WPF sırası, alfa önde), rgb(r,g,b), hsl(h,s%,l%).
pub struct ColorParser;

const MAX_INPUT_LENGTH: usize = 48;

impl ColorParser {
    /// Girdi gerçekten renk gibi değilse (kanal 255'ten büyük, S/L yüzde 100'den büyük...) `None`.
    pub fn try_parse(text: Option<&str>) -> Option<ParsedColor> {
        let text = text?;
        if text::is_null_or_white_space(Some(text)) || utf16_len(text) > MAX_INPUT_LENGTH {
            return None;
        }

        let s = text.trim();
        Self::try_parse_hex(s).or_else(|| Self::try_parse_rgb(s)).or_else(|| Self::try_parse_hsl(s))
    }

    fn try_parse_hex(s: &str) -> Option<ParsedColor> {
        let m = hex_pattern().captures(s)?;
        let mut hex = m["hex"].to_string();
        let mut alpha = 255u8;
        if hex.len() == 8 {
            alpha = u8::from_str_radix(&hex[..2], 16).ok()?;
            hex = hex[2..].to_string();
        }

        let rgb = Rgb::try_parse(Some(&hex))?;
        Some(ParsedColor::new(rgb, alpha, None))
    }

    fn try_parse_rgb(s: &str) -> Option<ParsedColor> {
        let m = rgb_pattern().captures(s)?;
        let r: i32 = m["r"].parse().ok()?;
        let g: i32 = m["g"].parse().ok()?;
        let b: i32 = m["b"].parse().ok()?;
        if r > 255 || g > 255 || b > 255 {
            return None;
        }

        Some(ParsedColor::new(Rgb::new(r as u8, g as u8, b as u8), 255, None))
    }

    fn try_parse_hsl(s: &str) -> Option<ParsedColor> {
        let m = hsl_pattern().captures(s)?;
        let h: f64 = m["h"].parse().ok()?;
        let sat: f64 = m["s"].parse().ok()?;
        let light: f64 = m["l"].parse().ok()?;
        if !h.is_finite() || sat > 100.0 || light > 100.0 {
            return None;
        }

        let hsl = Hsl::new(((h % 360.0) + 360.0) % 360.0, sat / 100.0, light / 100.0);
        Some(ParsedColor::new(ColorMath::from_hsl_value(hsl), 255, Some(hsl)))
    }
}

/// Renk çevirici: "#FF8800", "rgb(255, 136, 0)", "hsl(32, 100%, 50%)" yazınca HEX, RGB ve HSL karşılıkları; her satır
/// Enter ile kopyalanır.
pub struct ColorConverterSource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
}

impl ColorConverterSource {
    pub fn new(shell: Arc<dyn ISearchShell>, t: Translate) -> Self {
        let name = t("SearchCatColor");
        ColorConverterSource { base: SearchSourceBase::new("color", name), shell, t }
    }
}

impl ISearchSource for ColorConverterSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let Some(color) = ColorParser::try_parse(Some(&query.raw)) else {
            return Vec::new();
        };

        let rows = [("HEX", color.to_hex()), ("RGB", color.to_rgb_text()), ("HSL", color.to_hsl_text())];

        rows.into_iter()
            .enumerate()
            .map(|(index, (format, text))| {
                let shell = Arc::clone(&self.shell);
                let copy = text.clone();
                let mut result = SearchResult::new(
                    format!("color:{}:{}", format, query.raw),
                    self.base.name(),
                    text.clone(),
                    move || shell.copy_text(&copy),
                );
                result.subtitle = Some(format!("{} · {}", format, (self.t)("SearchHintCopy")));
                result.glyph = SearchGlyph::Clipboard;
                result.category_relevance = SearchRelevance::HIGHEST;
                result.relevance = 1.0 - index as f64 * 0.1;
                result.restores_focus = true;
                result.drag = Some(SearchDragData::new(SearchDragKind::Text, text));
                result
            })
            .collect()
    }
}

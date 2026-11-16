//! Dock simgelerinin gösterge ve rozet görünümü (ayardan bir kez türetilir, öğeler paylaşır). Karşılığı:
//! `DockPresentationStyle.cs`. Kararlar Core'dadır (`DockPresentationMath`); burada yalnız renkler çözülür. Renk `None`
//! ise görünüm tema vurgusunu kullanır (tema değişince canlı güncellenir).

use koy_core::dock::{DockBadgeCorner, DockBadgeStyle, DockColorSource, DockIndicatorStyle, DockLabelMode, DockPresentationMath};
use koy_core::settings::DockSettings;

// Nötr renk: bugünkü "çalışıyor" noktasıyla aynı gri.
const NEUTRAL: (u8, u8, u8) = (0xA1, 0xA1, 0xAA);

#[derive(Debug, Clone)]
pub struct DockPresentationStyle {
    pub indicator_style: DockIndicatorStyle,
    pub one_per_window: bool,
    /// Gösterge rengi; vurgu seçiliyse `None` (tema vurgusu).
    pub indicator_color: Option<slint::Color>,
    /// Arka plan vurgusu (gösterge renginin %25 opak hâli); vurgu seçiliyse `None`.
    pub highlight_color: Option<slint::Color>,
    pub show_badges: bool,
    pub badge_style: DockBadgeStyle,
    pub badge_color: Option<slint::Color>,
    pub badge_right: bool,
    pub badge_top: bool,
    pub label_mode: DockLabelMode,
}

impl Default for DockPresentationStyle {
    fn default() -> Self {
        Self::from_settings(&DockSettings::default())
    }
}

/// "#RRGGBB" ya da "#AARRGGBB" (WPF ColorConverter'ın yaygın biçimleri) → RGB.
fn parse_hex(text: &str) -> Option<(u8, u8, u8)> {
    let h = text.trim().strip_prefix('#')?;
    let v = u32::from_str_radix(h, 16).ok()?;
    match h.len() {
        6 => Some(((v >> 16) as u8, (v >> 8) as u8, v as u8)),
        8 => Some(((v >> 16) as u8, (v >> 8) as u8, v as u8)),
        3 => {
            let (r, g, b) = ((v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF);
            Some(((r * 17) as u8, (g * 17) as u8, (b * 17) as u8))
        }
        _ => None,
    }
}

/// Etkin kaynak Core'da çözülür (geçersiz özel renk → vurgu). Vurgu = `None`.
fn solid_or_none(source: DockColorSource, custom_hex: &str, alpha: u8) -> Option<slint::Color> {
    let effective = DockPresentationMath::effective_color_source(source, Some(custom_hex));
    let (r, g, b) = match effective {
        DockColorSource::Accent => return None,
        DockColorSource::Neutral => NEUTRAL,
        DockColorSource::Custom => parse_hex(custom_hex)?,
    };
    Some(slint::Color::from_argb_u8(alpha, r, g, b))
}

impl DockPresentationStyle {
    pub fn from_settings(dock: &DockSettings) -> Self {
        let right = matches!(dock.badge_corner, DockBadgeCorner::TopRight | DockBadgeCorner::BottomRight);
        let top = matches!(dock.badge_corner, DockBadgeCorner::TopRight | DockBadgeCorner::TopLeft);
        DockPresentationStyle {
            indicator_style: dock.indicator_style,
            one_per_window: dock.indicator_one_per_window,
            indicator_color: solid_or_none(dock.indicator_color, &dock.indicator_custom_color, 0xFF),
            highlight_color: solid_or_none(dock.indicator_color, &dock.indicator_custom_color, 0x40),
            show_badges: dock.show_badges,
            badge_style: dock.badge_style,
            badge_color: solid_or_none(dock.badge_color, &dock.badge_custom_color, 0xFF),
            badge_right: right,
            badge_top: top,
            label_mode: dock.label_mode,
        }
    }

    pub fn use_line(&self) -> bool {
        self.indicator_style == DockIndicatorStyle::Line
    }

    pub fn is_highlight(&self) -> bool {
        self.indicator_style == DockIndicatorStyle::Highlight
    }

    pub fn draws_mark(&self) -> bool {
        DockPresentationMath::draws_indicator_mark(self.indicator_style)
    }

    pub fn badge_dot_only(&self) -> bool {
        self.badge_style == DockBadgeStyle::DotOnly
    }

    /// Rozet kenar boşluğu (sol, üst, sağ, alt): alt köşede gösterge noktasının üstüne binmesin diye biraz yukarıda.
    pub fn badge_margin(&self) -> (f64, f64, f64, f64) {
        (
            if self.badge_right { 0.0 } else { 2.0 },
            if self.badge_top { 2.0 } else { 0.0 },
            if self.badge_right { 2.0 } else { 0.0 },
            if self.badge_top { 0.0 } else { 6.0 },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colors() {
        assert_eq!(parse_hex("#FF8800"), Some((0xFF, 0x88, 0x00)));
        assert_eq!(parse_hex("#80FF8800"), Some((0xFF, 0x88, 0x00)));
        assert_eq!(parse_hex("#F80"), Some((0xFF, 0x88, 0x00)));
        assert_eq!(parse_hex("FF8800"), None);
    }

    #[test]
    fn badge_margin_keeps_bottom_badges_above_indicator() {
        let mut d = DockSettings::default();
        d.badge_corner = DockBadgeCorner::BottomLeft;
        let p = DockPresentationStyle::from_settings(&d);
        assert_eq!(p.badge_margin(), (2.0, 0.0, 0.0, 6.0));
    }
}

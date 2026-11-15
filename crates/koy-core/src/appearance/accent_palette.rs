use super::{ColorMath, ResolvedTheme, Rgb};

/// Bir vurgu renginden türetilen, arayüzde kullanılan tonlar (hepsi "#RRGGBB").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccentPalette {
    /// Seçilen vurgu: anahtar izi, seçili gösterge, birincil düğme zemini.
    pub base: String,
    /// Üzerine gelince.
    pub hover: String,
    /// Basılıyken.
    pub pressed: String,
    /// Vurgu zemininin üstündeki yazı ve simge rengi (siyaha yakın ya da beyaz).
    pub on_accent: String,
    /// Vurgu renginde YAZI gerektiğinde kullanılan ton: yüzey üstünde en az 4,5:1 kontrast.
    pub text: String,
    /// Klavye odak halkası: yüzey üstünde en az 3:1 kontrast.
    pub focus: String,
}

const INK: Rgb = Rgb::new(0x0A, 0x0A, 0x0B);
const WHITE: Rgb = Rgb::new(0xFF, 0xFF, 0xFF);

impl AccentPalette {
    pub const DEFAULT_HEX: &'static str = "#3B82F6";

    /// `accent_hex`: seçilen vurgu; geçersizse Koy mavisi kullanılır. `theme`: koyu temada tonlar açılır, açık temada
    /// koyulaşır. `surface_hex`: yazının üstünde duracağı yüzey (Text ve Focus buna göre ayarlanır).
    pub fn derive(accent_hex: Option<&str>, theme: ResolvedTheme, surface_hex: &str) -> AccentPalette {
        let accent = Rgb::try_parse(accent_hex).unwrap_or_else(|| Rgb::parse(Self::DEFAULT_HEX));
        let surface = Rgb::parse(surface_hex);
        let dark = theme == ResolvedTheme::Dark;

        // Hover bir ton açılır (koyuda) ya da koyulaşır (açıkta); basılıyken ters yönde bir ton daha.
        let hover = ColorMath::mix(accent, if dark { WHITE } else { INK }, 0.18);
        // C# ile aynı: `dark ? Ink : Ink` — iki dal da aynı renk (koyu temada da siyaha doğru karışır).
        let pressed = ColorMath::mix(accent, INK, if dark { 0.22 } else { 0.30 });

        // Yazı ve simge için iki aday: siyaha yakın ya da beyaz; kontrastı yüksek olan seçilir.
        let on_accent = if ColorMath::contrast(accent, INK) >= ColorMath::contrast(accent, WHITE) { INK } else { WHITE };

        AccentPalette {
            base: accent.to_hex(),
            hover: hover.to_hex(),
            pressed: pressed.to_hex(),
            on_accent: on_accent.to_hex(),
            text: Self::readable(accent, surface, dark, 4.5).to_hex(),
            focus: Self::readable(accent, surface, dark, 3.0).to_hex(),
        }
    }

    /// Vurgu rengini, yüzey üstünde istenen kontrasta ulaşana dek koyuda beyaza, açıkta siyaha doğru çeker.
    fn readable(accent: Rgb, surface: Rgb, dark: bool, min_contrast: f64) -> Rgb {
        let target = if dark { WHITE } else { INK };
        for i in 0..=20 {
            let candidate = ColorMath::mix(accent, target, i as f64 / 20.0);
            if ColorMath::contrast(candidate, surface) >= min_contrast {
                return candidate;
            }
        }

        target
    }
}

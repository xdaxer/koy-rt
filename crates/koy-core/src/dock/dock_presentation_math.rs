use super::dock_enums::DockIndicatorStyle;
use super::dock_presentation_enums::{DockBadgeStyle, DockColorSource, DockRevealTrigger};
use crate::text;
use crate::time::TimeSpan;

/// Dock göstergesi, rozet, gizlenme/çıkma zamanlaması ve tetikleyici kararları (saf mantık, çizim yok).
/// App katmanı bu sonuçları XAML'e bağlar; kural burada test edilir.
pub struct DockPresentationMath;

impl DockPresentationMath {
    /// Gecikmeler için alt ve üst sınır (ms). Bunun dışındaki dosya değeri kıstırılır.
    pub const MIN_DELAY_MS: i32 = 0;
    pub const MAX_DELAY_MS: i32 = 2000;

    /// Tek simgede en çok kaç gösterge noktası çizilir (pencere başına kipte).
    pub const MAX_INDICATOR_DOTS: i32 = 3;

    /// Rozetteki en büyük sayı; bunun üstü "9+" olarak gösterilir.
    pub const MAX_BADGE_COUNT: i32 = 9;

    pub fn clamp_delay(ms: i32) -> i32 {
        text::clamp(ms, Self::MIN_DELAY_MS, Self::MAX_DELAY_MS)
    }

    /// Bir simgede kaç gösterge çizilir. Yalnız nokta kipinde ve "pencere başına" açıkken her pencere için bir nokta
    /// (en çok `MAX_INDICATOR_DOTS`); diğer durumlarda pencere varsa tek gösterge.
    pub fn indicator_count(window_count: i32, style: DockIndicatorStyle, one_per_window: bool) -> i32 {
        if window_count <= 0 {
            return 0;
        }

        if style != DockIndicatorStyle::Dot || !one_per_window {
            return 1;
        }

        window_count.min(Self::MAX_INDICATOR_DOTS)
    }

    /// Rozet metni. Nokta kipinde ya da sayı sıfırsa boş döner.
    pub fn badge_text(count: i32, style: DockBadgeStyle) -> String {
        if count <= 0 || style == DockBadgeStyle::DotOnly {
            return String::new();
        }

        if count > Self::MAX_BADGE_COUNT { format!("{}+", Self::MAX_BADGE_COUNT) } else { count.to_string() }
    }

    /// Rozet görünür mü: sayı kipinde sayı varsa, nokta kipinde en az bir bildirim varsa.
    pub fn shows_badge(count: i32, style: DockBadgeStyle) -> bool {
        if style == DockBadgeStyle::DotOnly { count > 0 } else { !Self::badge_text(count, style).is_empty() }
    }

    /// Dock'un çıkıp çıkmayacağı. `edge_held_for` fare alt kenarda ne kadar süredir kesintisiz durduğu.
    /// Fare tuşu basılıyken (sürükleme, seçim) hiçbir tetikleyici dock'u açmaz.
    pub fn should_reveal(
        trigger: DockRevealTrigger,
        at_edge: bool,
        button_pressed: bool,
        shortcut_active: bool,
        edge_held_for: TimeSpan,
        reveal_delay_ms: i32,
    ) -> bool {
        if button_pressed {
            return false;
        }

        // Tanımsız değer (elle düzenlenmiş dosya) kenarda bekleme gibi davranır: "hiç açılmaz" sonucu gizli dock'u ve
        // görev çubuğunu erişilmez bırakırdı. (Rust'ta enum tanımsız değer taşıyamaz; EdgeHold kolu bunu karşılar.)
        match trigger {
            DockRevealTrigger::EdgeTouch => at_edge,
            DockRevealTrigger::ShortcutOnly => shortcut_active,
            DockRevealTrigger::EdgeHold => at_edge && edge_held_for >= TimeSpan::from_millis(Self::clamp_delay(reveal_delay_ms) as i64),
        }
    }

    /// Kenar izleyicisinin bekleme süresi. Kenara dokunmak beklemesizdir; diğer tetikleyiciler (kısayol dahil: kısayol
    /// yolu henüz yok, kenar yine açabilmeli) ayardaki süreyi bekler.
    pub fn edge_reveal_delay(trigger: DockRevealTrigger, reveal_delay_ms: i32) -> TimeSpan {
        if trigger == DockRevealTrigger::EdgeTouch {
            TimeSpan::ZERO
        } else {
            TimeSpan::from_millis(Self::clamp_delay(reveal_delay_ms) as i64)
        }
    }

    /// Çalışma göstergesi simgenin altına nokta/çizgi çizer mi (arka plan vurgusu çizmez).
    pub fn draws_indicator_mark(style: DockIndicatorStyle) -> bool {
        style != DockIndicatorStyle::Highlight
    }

    /// Gizli dock'un altındaki ince çizgi. Ayar açıksa, dock gizliyse ve tam ekran bir uygulama yoksa görünür;
    /// tam ekranda çizgi her zaman gizlenir (oyun ve video kenarda bir şerit görmesin).
    pub fn shows_hidden_edge_line(line_enabled: bool, dock_hidden: bool, fullscreen_active: bool) -> bool {
        line_enabled && dock_hidden && !fullscreen_active
    }

    /// Özel renk metni geçerli mi: "#RRGGBB" biçimi (büyük/küçük harf fark etmez).
    pub fn is_valid_hex_color(text: Option<&str>) -> bool {
        let Some(text) = text else { return false };
        let chars: Vec<char> = text.chars().collect();
        if chars.len() != 7 || chars[0] != '#' {
            return false;
        }

        chars[1..].iter().all(|c| c.is_ascii_hexdigit())
    }

    /// Uygulanacak renk kaynağı. Özel renk seçili ama metin geçersizse vurgu rengine düşer (bozuk dosya dock'u boyamaz).
    pub fn effective_color_source(source: DockColorSource, custom_hex: Option<&str>) -> DockColorSource {
        if source == DockColorSource::Custom && !Self::is_valid_hex_color(custom_hex) { DockColorSource::Accent } else { source }
    }
}

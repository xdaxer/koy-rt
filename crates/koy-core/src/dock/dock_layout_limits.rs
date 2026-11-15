use crate::settings::DockSettings;
use crate::text;

/// Dock geometri ayarlarının geçerli aralıkları. Ayar dosyası elle düzenlenmiş olabileceği için değerler
/// kullanılmadan önce burada kenetlenir (`sanitize`, AppSettings normalizasyonundan çağrılır).
pub struct DockLayoutLimits;

impl DockLayoutLimits {
    /// Uzunluk yüzdesi: çok kısa bir dock okunmaz, en az %10.
    pub const MIN_LENGTH_PERCENT: i32 = 10;
    pub const MAX_LENGTH_PERCENT: i32 = 100;
    pub const DEFAULT_LENGTH_PERCENT: i32 = 100;

    /// Ekran kenarından uzaklık (DIP).
    pub const MIN_EDGE_OFFSET_DIP: f64 = 0.0;
    pub const MAX_EDGE_OFFSET_DIP: f64 = 96.0;
    pub const DEFAULT_EDGE_OFFSET_DIP: f64 = 0.0;

    /// Dock içi dolgu (DIP), her kenardan.
    pub const MIN_INNER_PADDING_DIP: f64 = 0.0;
    pub const MAX_INNER_PADDING_DIP: f64 = 24.0;
    pub const DEFAULT_INNER_PADDING_DIP: f64 = 0.0;

    /// Simgeler arası boşluk (DIP), yuva başına toplam. Varsayılan `DockFitMath::SLOT_MARGIN_DIP` ile aynıdır.
    pub const MIN_ICON_SPACING_DIP: f64 = 0.0;
    pub const MAX_ICON_SPACING_DIP: f64 = 16.0;
    pub const DEFAULT_ICON_SPACING_DIP: f64 = 2.0;

    pub fn clamp_length_percent(value: i32) -> i32 {
        text::clamp(value, Self::MIN_LENGTH_PERCENT, Self::MAX_LENGTH_PERCENT)
    }

    pub fn clamp_edge_offset_dip(value: f64) -> f64 {
        Self::clamp_finite(value, Self::MIN_EDGE_OFFSET_DIP, Self::MAX_EDGE_OFFSET_DIP, Self::DEFAULT_EDGE_OFFSET_DIP)
    }

    pub fn clamp_inner_padding_dip(value: f64) -> f64 {
        Self::clamp_finite(value, Self::MIN_INNER_PADDING_DIP, Self::MAX_INNER_PADDING_DIP, Self::DEFAULT_INNER_PADDING_DIP)
    }

    pub fn clamp_icon_spacing_dip(value: f64) -> f64 {
        Self::clamp_finite(value, Self::MIN_ICON_SPACING_DIP, Self::MAX_ICON_SPACING_DIP, Self::DEFAULT_ICON_SPACING_DIP)
    }

    /// Dock ayarlarındaki geometri alanlarını aralığa kenetler. NaN ve sonsuz değerler varsayılana döner
    /// (Math.Clamp NaN'ı korur, bu yüzden ayrı ele alınır).
    ///
    /// C#'taki "tanımsız enum sayısı (elle düzenlenmiş `"Edge": 7`) alt kenara döner" adımı Rust'ta gerekmez:
    /// `DockEdge` yalnız tanımlı değerleri taşıyabilir (serde bilinmeyen adı zaten reddeder).
    pub fn sanitize(dock: &mut DockSettings) {
        dock.length_percent = Self::clamp_length_percent(dock.length_percent);
        dock.edge_offset_dip = Self::clamp_edge_offset_dip(dock.edge_offset_dip);
        dock.inner_padding_dip = Self::clamp_inner_padding_dip(dock.inner_padding_dip);
        dock.icon_spacing_dip = Self::clamp_icon_spacing_dip(dock.icon_spacing_dip);
    }

    fn clamp_finite(value: f64, min: f64, max: f64, fallback: f64) -> f64 {
        if value.is_finite() { text::clamp(value, min, max) } else { fallback }
    }
}

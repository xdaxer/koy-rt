use super::dock_layout_limits::DockLayoutLimits;
use crate::text;

/// Çok uygulama açıkken dock'un ekrana sığması: simgeler tercih edilen boyuttan en küçük boyuta kadar küçülür,
/// o zaman da sığmıyorsa küçülme durur (taşan öğeler kırpılır). Saf hesap; dock ölçüleri DockViewModel'de uygulanır.
pub struct DockFitMath;

impl DockFitMath {
    /// Simgenin inebileceği en küçük boyut (DIP).
    pub const MIN_ICON_SIZE_DIP: f64 = 24.0;

    /// Öğe kutusu (ItemSize) simgeden bu kadar büyüktür.
    pub const ITEM_CHROME_DIP: f64 = 10.0;

    /// Her öğenin sağında ve solunda 1 DIP boşluk (Margin="1,0").
    pub const SLOT_MARGIN_DIP: f64 = 2.0;

    pub fn item_size_for(icon_size: f64) -> f64 {
        icon_size + Self::ITEM_CHROME_DIP
    }

    /// Bir yuvanın kapladığı genişlik veya yükseklik. `icon_spacing_dip` C# varsayılanı: `SLOT_MARGIN_DIP`.
    pub fn slot_width(icon_size: f64, icon_spacing_dip: f64) -> f64 {
        let spacing = DockLayoutLimits::clamp_icon_spacing_dip(icon_spacing_dip);
        icon_size + Self::ITEM_CHROME_DIP + spacing
    }

    /// Verilen genişliğe sığan simge boyutu (tam sayı DIP). Tercih edilenden büyümez, en küçük boyutun altına inmez;
    /// tercih edilen zaten en küçükten küçükse olduğu gibi kalır.
    ///
    /// * `preferred_icon_size`: Ayardaki simge boyutu.
    /// * `slot_count`: Simge boyutuyla ölçeklenen yuva sayısı.
    /// * `fixed_width`: Boyuta bağlı olmayan genişlik (dolgu, ayırıcı, sabit düğmeler).
    /// * `available_width`: Kullanılabilir toplam genişlik.
    /// * `icon_spacing_dip`: Yuva başına simge aralığı (C# varsayılanı: `SLOT_MARGIN_DIP`).
    pub fn fit_icon_size(preferred_icon_size: f64, slot_count: i32, fixed_width: f64, available_width: f64, icon_spacing_dip: f64) -> f64 {
        if preferred_icon_size <= Self::MIN_ICON_SIZE_DIP || slot_count <= 0 || !available_width.is_finite() || available_width <= 0.0 {
            return preferred_icon_size;
        }

        let spacing = DockLayoutLimits::clamp_icon_spacing_dip(icon_spacing_dip);
        let per_slot = (available_width - fixed_width) / slot_count as f64;
        let fit = (per_slot - Self::ITEM_CHROME_DIP - spacing).floor();
        text::clamp(fit, Self::MIN_ICON_SIZE_DIP, preferred_icon_size)
    }

    /// Fare üstünde öğenin en çok büyüyeceği ölçek: çubuk yüksekliğine (1 px üst çizgi + 1 px pay) sığacak kadar,
    /// en fazla `max_scale` (C# varsayılanı 1.30). 48 px çubukta 38 px'lik kutu ~1,21 kata çıkar, 60 px kapsülde 1,30.
    pub fn hover_peak_scale(bar_height: f64, item_size: f64, max_scale: f64) -> f64 {
        if item_size <= 0.0 || bar_height <= 0.0 {
            return 1.0;
        }

        text::clamp((bar_height - 2.0) / item_size, 1.0, max_scale)
    }

    /// Uygulama şeridine kalan en çok genişlik; 0'ın altına inmez.
    pub fn strip_max_width(available_width: f64, other_width: f64) -> f64 {
        0.0_f64.max(available_width - other_width)
    }
}

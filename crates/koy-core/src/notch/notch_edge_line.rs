use serde::{Deserialize, Serialize};

use crate::display::DisplayMonitor;
use crate::shell::WindowRect;
use crate::text;

/// Üst kenar çizgisinin rengi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NotchEdgeLineColor {
    /// Kapalı çentikle aynı zemin (KoyNotchSurface, varsayılan siyah): çentik çizginin devamı gibi görünür.
    #[default]
    NotchSurface,
    /// Seçili vurgu rengi (KoyAccent).
    Accent,
}

/// Çentiğin iki yanından ekranın üst kenarı boyunca uzanan çizgi. Çizgi ayrı, tıklama geçiren ince bir penceredir ve
/// çentiğin hemen altında durur: çentik kendi genişliği kadarını örter, gizlenince çizginin içine çekilir.
pub struct NotchEdgeLine;

impl NotchEdgeLine {
    /// En ince çizgi (DIP).
    pub const MIN_THICKNESS: i32 = 1;

    /// En kalın çizgi (DIP). Kapalı çentik 34 DIP; çizgi onun yarısından az kalır ki çentik çizgiden ayrı okunsun.
    pub const MAX_THICKNESS: i32 = 20;

    /// Varsayılan kalınlık (DIP).
    pub const DEFAULT_THICKNESS: i32 = 2;

    /// Ayar dosyasından ya da profilden gelen kalınlığı geçerli aralığa çeker.
    pub fn clamp_thickness(thickness: i32) -> i32 {
        text::clamp(thickness, Self::MIN_THICKNESS, Self::MAX_THICKNESS)
    }

    /// Rengin okunacağı tema kaynağı.
    pub fn resource_key(color: NotchEdgeLineColor) -> &'static str {
        if color == NotchEdgeLineColor::Accent { "KoyAccent" } else { "KoyNotchSurface" }
    }

    /// Çizgi penceresinin piksel sınırı: monitörün tam genişliği, üst kenara yapışık, en az 1 px.
    pub fn bounds(monitor: &DisplayMonitor, thickness_dip: i32) -> WindowRect {
        let height = i32::max(1, text::round(Self::clamp_thickness(thickness_dip) as f64 * monitor.dpi_scale_y) as i32);
        WindowRect::new(monitor.bounds.left, monitor.bounds.top, monitor.bounds.right, monitor.bounds.top + height)
    }

    /// Çizgi görünsün mü: tam ekranda çentik gizleniyorsa çizgi de gizlenir.
    pub fn is_visible(is_fullscreen: bool, hide_in_fullscreen: bool) -> bool {
        !(is_fullscreen && hide_in_fullscreen)
    }
}

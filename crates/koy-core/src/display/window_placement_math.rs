use super::DisplayMonitor;
use crate::dock::{DockEdge, DockLayoutLimits};
use crate::shell::WindowRect;

/// Çentik ve dock penceresinin hedef monitördeki fiziksel piksel konumu. WPF'in Left/Top (DIP) alanları pencerenin
/// o anki DPI'ıyla yorumlanır; farklı ölçekli ekranlar arasında yanlış yere düşer. Bu yüzden konum, hedef
/// monitörün kendi ölçeğiyle pikselde hesaplanır ve SetWindowPos ile uygulanır.
pub struct WindowPlacementMath;

impl WindowPlacementMath {
    /// Monitörün üst kenarında yatay ortalı pencere (çentik).
    pub fn top_center(monitor: &DisplayMonitor, width_dip: f64, height_dip: f64) -> WindowRect {
        Self::top_center_with_offset(monitor, width_dip, height_dip, 0.0)
    }

    /// Üst kenardan `top_offset_dip` aşağıda yatay ortalı pencere (yüzen ada). Ofset hedef monitörün
    /// kendi dikey ölçeğiyle piksele çevrilir; pencere boyutu değişmez, yalnız Top kayar.
    pub fn top_center_with_offset(monitor: &DisplayMonitor, width_dip: f64, height_dip: f64, top_offset_dip: f64) -> WindowRect {
        let width = Self::to_pixels(width_dip, monitor.dpi_scale_x);
        let height = Self::to_pixels(height_dip, monitor.dpi_scale_y);
        let left = monitor.bounds.left + (monitor.bounds.width() - width) / 2;
        let top = monitor.bounds.top + Self::to_pixels(f64::max(0.0, top_offset_dip), monitor.dpi_scale_y);
        WindowRect::new(left, top, left + width, top + height)
    }

    /// Monitörün alt kenarında yatay ortalı, alttan boşluklu pencere (yüzen kapsül dock).
    pub fn bottom_center(monitor: &DisplayMonitor, width_dip: f64, height_dip: f64, bottom_margin_dip: f64) -> WindowRect {
        let width = i32::min(Self::to_pixels(width_dip, monitor.dpi_scale_x), monitor.bounds.width());
        let height = Self::to_pixels(height_dip, monitor.dpi_scale_y);
        let margin = Self::to_pixels(bottom_margin_dip, monitor.dpi_scale_y);
        let left = monitor.bounds.left + (monitor.bounds.width() - width) / 2;
        let bottom = monitor.bounds.bottom - margin;
        WindowRect::new(left, bottom - height, left + width, bottom)
    }

    /// Monitörün tam ortasında pencere ("Ekranları tanımla" numarası).
    pub fn center(monitor: &DisplayMonitor, width_dip: f64, height_dip: f64) -> WindowRect {
        let width = Self::to_pixels(width_dip, monitor.dpi_scale_x);
        let height = Self::to_pixels(height_dip, monitor.dpi_scale_y);
        let left = monitor.bounds.left + (monitor.bounds.width() - width) / 2;
        let top = monitor.bounds.top + (monitor.bounds.height() - height) / 2;
        WindowRect::new(left, top, left + width, top + height)
    }

    /// Monitörün tam genişliğinde, alt kenara yaslı pencere (tam genişlik dock).
    pub fn bottom_full_width(monitor: &DisplayMonitor, height_dip: f64) -> WindowRect {
        let height = Self::to_pixels(height_dip, monitor.dpi_scale_y);
        WindowRect::new(monitor.bounds.left, monitor.bounds.bottom - height, monitor.bounds.right, monitor.bounds.bottom)
    }

    /// Verilen kenarda tam boy (veya uzunluk yüzdeli) dock penceresi yerleşimi (piksel).
    /// C# varsayılanları: `length_percent = DockLayoutLimits::DEFAULT_LENGTH_PERCENT`,
    /// `edge_offset_dip = DockLayoutLimits::DEFAULT_EDGE_OFFSET_DIP`.
    pub fn edge_bar(monitor: &DisplayMonitor, edge: DockEdge, thickness_dip: f64, length_percent: i32, edge_offset_dip: f64) -> WindowRect {
        // `Enum.IsDefined(edge)` Rust'ta her zaman doğru.
        let horizontal = matches!(edge, DockEdge::Bottom | DockEdge::Top);
        let thickness = Self::to_pixels(thickness_dip, if horizontal { monitor.dpi_scale_y } else { monitor.dpi_scale_x });
        let offset = Self::to_pixels(f64::max(0.0, edge_offset_dip), if horizontal { monitor.dpi_scale_y } else { monitor.dpi_scale_x });
        let length_fraction = DockLayoutLimits::clamp_length_percent(length_percent) as f64 / 100.0;

        if horizontal {
            let width = crate::text::round(monitor.bounds.width() as f64 * length_fraction) as i32;
            let left = monitor.bounds.left + (monitor.bounds.width() - width) / 2;
            let top = if edge == DockEdge::Bottom { monitor.bounds.bottom - offset - thickness } else { monitor.bounds.top + offset };
            WindowRect::new(left, top, left + width, top + thickness)
        } else {
            let height = crate::text::round(monitor.bounds.height() as f64 * length_fraction) as i32;
            let top = monitor.bounds.top + (monitor.bounds.height() - height) / 2;
            let left = if edge == DockEdge::Left { monitor.bounds.left + offset } else { monitor.bounds.right - offset - thickness };
            WindowRect::new(left, top, left + thickness, top + height)
        }
    }

    /// Verilen kenarda kapsül dock penceresi yerleşimi (piksel).
    pub fn edge_capsule(monitor: &DisplayMonitor, edge: DockEdge, length_dip: f64, thickness_dip: f64, edge_margin_dip: f64) -> WindowRect {
        let horizontal = matches!(edge, DockEdge::Bottom | DockEdge::Top);
        let thickness = Self::to_pixels(thickness_dip, if horizontal { monitor.dpi_scale_y } else { monitor.dpi_scale_x });
        let margin = Self::to_pixels(f64::max(0.0, edge_margin_dip), if horizontal { monitor.dpi_scale_y } else { monitor.dpi_scale_x });

        if horizontal {
            let width = i32::min(Self::to_pixels(length_dip, monitor.dpi_scale_x), monitor.bounds.width());
            let left = monitor.bounds.left + (monitor.bounds.width() - width) / 2;
            let top = if edge == DockEdge::Bottom { monitor.bounds.bottom - margin - thickness } else { monitor.bounds.top + margin };
            WindowRect::new(left, top, left + width, top + thickness)
        } else {
            let height = i32::min(Self::to_pixels(length_dip, monitor.dpi_scale_y), monitor.bounds.height());
            let top = monitor.bounds.top + (monitor.bounds.height() - height) / 2;
            let left = if edge == DockEdge::Left { monitor.bounds.left + margin } else { monitor.bounds.right - margin - thickness };
            WindowRect::new(left, top, left + thickness, top + height)
        }
    }

    /// DIP'ten fiziksel piksele TEK dönüşüm kuralı: yarım değerler yukarı yuvarlanır. Dock penceresinin yüksekliği ve
    /// AppBar'ın ayırdığı pay aynı fonksiyondan geçer; böylece büyütülmüş pencere ile çubuk arasında 1 piksellik boşluk
    /// ya da çakışma kalmaz (125 %, 150 % gibi kesirli ölçeklerde kesme/yuvarlama farkı bunu üretiyordu).
    pub fn to_pixels(dip: f64, scale: f64) -> i32 {
        // MidpointRounding.AwayFromZero
        (dip * if scale > 0.0 { scale } else { 1.0 }).round() as i32
    }
}

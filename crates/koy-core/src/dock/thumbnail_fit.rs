use crate::shell::WindowRect;
use crate::text;

/// Pencere önizlemesini kutuya en-boy oranını koruyarak sığdırır ve ortalar.
/// DWM küçük resmi verilen dikdörtgene gerilir; oran korunmazsa geniş pencere ezik görünür.
pub struct ThumbnailFit;

impl ThumbnailFit {
    /// `source_width` × `source_height` boyutlu kaynağı `box_rect` içine sığdırır.
    /// Kaynak kutudan küçükse büyütülmez (bulanıklaşmasın). Kaynak boyutu bilinmiyorsa kutunun kendisi döner.
    pub fn fit(source_width: i32, source_height: i32, box_rect: WindowRect) -> WindowRect {
        if box_rect.is_empty() {
            return WindowRect::EMPTY;
        }

        if source_width <= 0 || source_height <= 0 {
            return box_rect;
        }

        let scale = 1.0_f64.min((box_rect.width() as f64 / source_width as f64).min(box_rect.height() as f64 / source_height as f64));
        // Math.Round: bankacı yuvarlaması.
        let width = 1.max(text::round(source_width as f64 * scale) as i32);
        let height = 1.max(text::round(source_height as f64 * scale) as i32);

        let left = box_rect.left + (box_rect.width() - width) / 2;
        let top = box_rect.top + (box_rect.height() - height) / 2;
        WindowRect::new(left, top, left + width, top + height)
    }
}

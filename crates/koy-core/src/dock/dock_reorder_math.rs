use crate::text;

/// Dock'ta simge sürüklerken sıra ve kayma hesapları (saf mantık; yay animasyonu hedefleri bunlardan gelir).
pub struct DockReorderMath;

impl DockReorderMath {
    /// Sürüklenen simgenin merkezi, başlangıç yuvasından `drag_offset` kadar kaydıysa hangi yuvaya düşer.
    /// Yuvalar eşit genişliktedir (`slot_width`); sonuç [0, count) aralığına kırpılır.
    pub fn target_index(count: i32, original_index: i32, drag_offset: f64, slot_width: f64) -> i32 {
        if count <= 0 || slot_width <= 0.0 {
            return 0.max(original_index);
        }

        // MidpointRounding.AwayFromZero
        let shifted = (drag_offset / slot_width).round() as i32;
        text::clamp(original_index + shifted, 0, count - 1)
    }

    /// Sürükleme sürerken bir öğenin (sürüklenen hariç) yatay kayması: sürüklenen sağa geçtikçe aradakiler sola,
    /// sola geçtikçe sağa birer yuva kayar.
    pub fn shift_for(index: i32, dragged_index: i32, target_index: i32, slot_width: f64) -> f64 {
        if index == dragged_index {
            return 0.0;
        }

        if dragged_index < target_index && index > dragged_index && index <= target_index {
            return -slot_width;
        }

        if target_index < dragged_index && index >= target_index && index < dragged_index {
            return slot_width;
        }

        0.0
    }

    /// Sürükleme eşiği aşıldı mı (tıklama ile sürüklemeyi ayırır).
    pub fn exceeds_threshold(dx: f64, dy: f64, threshold: f64) -> bool {
        dx.abs() >= threshold || dy.abs() >= threshold
    }
}

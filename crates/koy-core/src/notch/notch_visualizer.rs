use crate::text;

/// Görselleştirici çubuk sayısı sınırları. Ayar dosyası elle bozulsa bile çizim bu aralıkta kalır.
pub struct NotchVisualizer;

impl NotchVisualizer {
    pub const MIN_BAR_COUNT: i32 = 3;
    pub const MAX_BAR_COUNT: i32 = 16;

    pub fn clamp_bar_count(count: i32) -> i32 {
        text::clamp(count, Self::MIN_BAR_COUNT, Self::MAX_BAR_COUNT)
    }
}

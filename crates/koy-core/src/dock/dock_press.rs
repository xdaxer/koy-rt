use crate::text;

/// Dock öğesine basınca simgenin geri bildirimi. Tek bir ilerleme değeri (0 = bırakılmış, 1 = tam basılı) yayla sürülür;
/// ölçek ve koyulaşma bundan türetilir. Ölçek, fare üstü büyütmesinden AYRI bir çarpandır (ikisi birbirini ezmez).
/// Hareket azaltmada ölçek hiç değişmez, yalnız renk (koyulaşma) değişir.
pub struct DockPress;

impl DockPress {
    /// Tam basılıyken simgenin ölçeği.
    pub const PRESSED_SCALE: f64 = 0.92;

    /// Tam basılıyken simgenin opaklığı (dock koyu yüzey olduğundan düşük opaklık koyulaşma verir).
    pub const PRESSED_OPACITY: f64 = 0.72;

    /// Basma çarpanı: 1 (basılı değil) ile `PRESSED_SCALE` arası; yayın aşımı doğal olarak çevreye taşar.
    pub fn scale(progress: f64, motion_enabled: bool) -> f64 {
        if motion_enabled { 1.0 - (1.0 - Self::PRESSED_SCALE) * progress } else { 1.0 }
    }

    /// Simge opaklığı: ilerleme 0..1 aralığına kırpılır (aşım koyulaşmayı geçersiz kılmaz).
    pub fn opacity(progress: f64) -> f64 {
        1.0 - (1.0 - Self::PRESSED_OPACITY) * text::clamp(progress, 0.0, 1.0)
    }
}

use super::countdown_timer::CountdownTimer;
use crate::text;
use crate::time::TimeSpan;

/// Zamanlayıcı süre seçimi: hazır süreler ve artı/eksi düğmesinin adım kuralı. Küçük sürelerde ince, büyüklerde kaba
/// adım atar ki 1 dakikadan 2 saate az tıkla gidilebilsin.
pub struct CountdownPresets;

impl CountdownPresets {
    /// Hazır süreler (dakika).
    pub const MINUTES: [i32; 4] = [1, 5, 10, 25];

    /// Başlangıç süresi (dakika).
    pub const DEFAULT_MINUTES: i32 = 5;

    /// Artı/eksi düğmesinin yeni süresi. Adım: 10 dakikanın altında 1, bir saatin altında 5, üstünde 15 dakika.
    /// Sonuç her zaman adımın katına oturur (12 → 15 ya da 10). Sonuç 1 dakika - 99 saat.
    pub fn step(current: TimeSpan, increase: bool) -> TimeSpan {
        let minutes = text::round(current.total_minutes()) as i32;
        let step = Self::step_for(minutes, increase);

        let next = if increase {
            (minutes / step + 1) * step
        } else {
            (f64::from(minutes) / f64::from(step) - 1.0).ceil() as i32 * step
        };

        let clamped = text::clamp(next, 1, CountdownTimer::MAX_DURATION.total_minutes() as i32);
        TimeSpan::from_mins(i64::from(clamped))
    }

    fn step_for(minutes: i32, increase: bool) -> i32 {
        // Eşikte yön önemli: 10'dan aşağı inerken 1'er, 10'dan yukarı çıkarken 5'er.
        let reference = if increase { minutes } else { minutes - 1 };
        if reference < 10 {
            1
        } else if reference < 60 {
            5
        } else {
            15
        }
    }
}

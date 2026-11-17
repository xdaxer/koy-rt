//! Kapalı çentiğin statik arka plan renklerini çözer (saf hesap). Ambiyans kipinde video ya da zamanlayıcı
//! çalıştırılmaz; seçili önayarın renkleri okunabilir koyuluğa çekilerek statik bir renk listesi üretilir.
//! Karşılığı: `Core/Notch/CollapsedBackgroundPolicy.cs`.

use crate::appearance::{ColorMath, Rgb};

use super::LiveBackgroundPresets;

const DARK_FALLBACK: Rgb = Rgb::new(0x0A, 0x0A, 0x0B);
const TITLE_WHITE: Rgb = Rgb::new(0xF4, 0xF4, 0xF5);
const BASE_DARK_MIX: f64 = 0.25;

pub struct CollapsedBackgroundPolicy;

impl CollapsedBackgroundPolicy {
    pub const MIN_TEXT_CONTRAST: f64 = 7.0;

    /// Önayarın renk duraklarını, üstündeki beyaz saat/metin en az 7:1 kontrast alacak biçimde koyulaştırılmış sRGB
    /// listesi olarak döner. En az 1 renk garantilenir.
    pub fn resolve_ambient_stops(preset_id: Option<&str>) -> Vec<Rgb> {
        let preset = LiveBackgroundPresets::find(preset_id);
        let mut stops: Vec<Rgb> = preset.colors.iter().filter_map(|hex| Rgb::try_parse(Some(hex))).map(Self::ensure_readable).collect();
        if stops.is_empty() {
            stops.push(DARK_FALLBACK);
        }
        stops
    }

    /// Rengi kapalı çentik zeminine uygun koyuluğa çeker; parlak önayar duraklarında beyaz saat ve simgeler okunaksız
    /// kalmasın diye kontrast en az `MIN_TEXT_CONTRAST` olana dek koyu tabana karıştırır.
    pub fn ensure_readable(color: Rgb) -> Rgb {
        let candidate = ColorMath::mix(color, DARK_FALLBACK, BASE_DARK_MIX);
        if ColorMath::contrast(TITLE_WHITE, candidate) >= Self::MIN_TEXT_CONTRAST {
            return candidate;
        }
        // İkili arama ile gereken en küçük koyulaştırma oranı; renk tonu olabildiğince korunur.
        let (mut lo, mut hi) = (BASE_DARK_MIX, 1.0);
        for _ in 0..10 {
            let mid = (lo + hi) * 0.5;
            let mixed = ColorMath::mix(color, DARK_FALLBACK, mid);
            if ColorMath::contrast(TITLE_WHITE, mixed) >= Self::MIN_TEXT_CONTRAST {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        ColorMath::mix(color, DARK_FALLBACK, hi)
    }
}

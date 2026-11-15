use super::AnimationSpeed;
use crate::text;

/// Animasyon başına açma/kapama ile ayrılan animasyon türleri (issue #79).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionKind {
    NotchOpenClose,
    DockMagnify,
    FolderPanel,
    Osd,
    BadgeBounce,
    TabTransitions,
}

impl MotionKind {
    /// `Enum.GetValues<MotionPolicy.MotionKind>()`: bildirim sırasıyla bütün türler.
    pub const ALL: [MotionKind; 6] = [
        MotionKind::NotchOpenClose,
        MotionKind::DockMagnify,
        MotionKind::FolderPanel,
        MotionKind::Osd,
        MotionKind::BadgeBounce,
        MotionKind::TabTransitions,
    ];
}

/// `Koy.Core.Settings.MotionSettings` karşılığı olarak `MotionPolicy::scale_for`'un okuduğu altı anahtar.
/// Ayarlar modülü `MotionSettings` için bu trait'i uygular (Appearance modülü Settings'e bağlanmasın diye
/// anahtarlar trait üzerinden okunur).
pub trait MotionToggles {
    fn notch_open_close(&self) -> bool;
    fn dock_magnify(&self) -> bool;
    fn folder_panel(&self) -> bool;
    fn osd(&self) -> bool;
    fn badge_bounce(&self) -> bool;
    fn tab_transitions(&self) -> bool;
}

/// Animasyon hızı ve azaltılmış hareket kararları. Tek bir çarpan üretir: süreler bununla çarpılır,
/// 0 ise geçişler anında yapılır (Corvus'taki "motion scale" mantığı).
pub struct MotionPolicy;

impl MotionPolicy {
    /// Hız ayarının kendi çarpanı (azaltılmış hareket hesaba katılmadan).
    pub fn speed_factor(value: AnimationSpeed) -> f64 {
        match value {
            AnimationSpeed::Calm => 1.5,
            AnimationSpeed::Fast => 0.65,
            AnimationSpeed::Off => 0.0,
            AnimationSpeed::Normal => 1.0,
        }
    }

    /// Son çarpan: 0 ise animasyon yok.
    /// - `speed`: animasyon ayarı (Sakin / Normal / Hızlı / Kapalı).
    /// - `respect_reduce_motion`: "Windows'un animasyonları azalt ayarına uy" açık mı.
    /// - `system_animations_enabled`: Windows istemci alanı animasyonları açık mı (SystemParameters.ClientAreaAnimation).
    /// - `battery_saver_reduce_motion`: pil tasarrufu politikası şu an hareketi azaltmayı istiyor mu (BatterySaverPolicy).
    pub fn scale(
        speed: AnimationSpeed,
        respect_reduce_motion: bool,
        system_animations_enabled: bool,
        battery_saver_reduce_motion: bool,
    ) -> f64 {
        if battery_saver_reduce_motion {
            return 0.0;
        }
        if respect_reduce_motion && !system_animations_enabled {
            return 0.0;
        }
        Self::speed_factor(speed)
    }

    /// Taban süreyi (ms) çarpanla ölçekler; 0 çarpan 0 ms verir.
    pub fn scaled_milliseconds(base_milliseconds: f64, scale: f64) -> i32 {
        if scale <= 0.0 { 0 } else { text::round(base_milliseconds * scale) as i32 }
    }

    /// Bir animasyon türünün son çarpanı. Kendi anahtarı kapalıysa 0 (son durum anında); açıksa genel çarpan.
    /// Genel çarpan (azaltılmış hareket, pil tasarrufu, hız) her zaman kendi anahtarının üstündedir: anahtar açık
    /// olsa da genel çarpan 0 ise animasyon yoktur. Sayı olmayan çarpan (NaN) da 0 sayılır: aksi halde
    /// `scaled_milliseconds` int.MinValue ms üretirdi.
    pub fn scale_for(kind: MotionKind, toggles: &dyn MotionToggles, global_scale: f64) -> f64 {
        if !(global_scale > 0.0) || !Self::is_enabled(kind, toggles) {
            return 0.0;
        }
        global_scale
    }

    fn is_enabled(kind: MotionKind, t: &dyn MotionToggles) -> bool {
        match kind {
            MotionKind::NotchOpenClose => t.notch_open_close(),
            MotionKind::DockMagnify => t.dock_magnify(),
            MotionKind::FolderPanel => t.folder_panel(),
            MotionKind::Osd => t.osd(),
            MotionKind::BadgeBounce => t.badge_bounce(),
            MotionKind::TabTransitions => t.tab_transitions(),
        }
    }
}

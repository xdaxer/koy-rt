use super::MaterialLevel;
use crate::text;

/// Malzeme ayarı (`MaterialLevel`): Katı (opak), Buzlu (%86) ya da Cam (%72). Bulanıklık yoktur: katmanlı pencerede
/// (AllowsTransparency) DWM akrilik uygulanmadığından saydamlık saf alfadır. Onaylı kaynak: docs/design/source/CamKoy.dc.html
/// ve CamCentik.dc.html. Eski dosyalardaki Türkçe adlar ("Katı", "Yarı saydam"…) ayar göçünde enum adına çevrilir.
pub struct MaterialSetting;

impl MaterialSetting {
    pub const FROSTED_OPACITY: f64 = 0.86;
    pub const GLASS_OPACITY: f64 = 0.72;

    /// Okunabilirlik korumasının denediği sıra: en saydamdan opağa.
    pub const LEVELS: [MaterialLevel; 3] = [MaterialLevel::Glass, MaterialLevel::Frosted, MaterialLevel::Solid];

    /// Katı dışındaki her seviye (yüzeyler saydam).
    pub fn is_translucent(value: MaterialLevel) -> bool {
        value != MaterialLevel::Solid
    }

    /// Yüzey opaklığı: Katı 1, Buzlu 0,86, Cam 0,72.
    pub fn surface_opacity(value: MaterialLevel) -> f64 {
        match value {
            MaterialLevel::Glass => Self::GLASS_OPACITY,
            MaterialLevel::Frosted => Self::FROSTED_OPACITY,
            MaterialLevel::Solid => 1.0,
        }
    }

    /// Opaklığın 0–255 alfa karşılığı.
    pub fn surface_alpha(value: MaterialLevel) -> u8 {
        text::round(255.0 * Self::surface_opacity(value)) as u8
    }

    // Bulanıklıksız yüzeyler (çentik, dock): katmanlı pencere oldukları için DWM bulanıklığı alamazlar. Demodaki %86 / %72
    // gerçek masaüstünde arkadaki metni okunur bırakıyordu (proje sahibi elle sınaması, 2026-11-11: "fazla saydam");
    // bu yüzden aynı seviyeler burada daha opaktır. Koy penceresi gerçek akrilik aldığı için surface_opacity'de kalır.
    pub const LAYERED_FROSTED_OPACITY: f64 = 0.94;
    pub const LAYERED_GLASS_OPACITY: f64 = 0.88;

    /// Bulanıklıksız yüzeyin (çentik, dock) opaklığı: Katı 1, Buzlu 0,94, Cam 0,88.
    pub fn layered_opacity(value: MaterialLevel) -> f64 {
        match value {
            MaterialLevel::Glass => Self::LAYERED_GLASS_OPACITY,
            MaterialLevel::Frosted => Self::LAYERED_FROSTED_OPACITY,
            MaterialLevel::Solid => 1.0,
        }
    }

    /// Bulanıklıksız yüzey opaklığının 0–255 alfa karşılığı.
    pub fn layered_alpha(value: MaterialLevel) -> u8 {
        text::round(255.0 * Self::layered_opacity(value)) as u8
    }

    /// Sistem arka planı (DWM akrilik/mika) desteklenmiyorsa (Windows 10) pencere yüzeyi opak `Solid` olur.
    pub fn resolve_window_material(requested: MaterialLevel, supports_backdrop: bool) -> MaterialLevel {
        if supports_backdrop { requested } else { MaterialLevel::Solid }
    }
}

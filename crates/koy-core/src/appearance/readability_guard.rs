use super::{ColorMath, MaterialLevel, MaterialSetting, Rgb};

/// Okunabilirlik koruması: duvar kağıdı açık renkliyse saydam yüzeyde ikincil metin 4,5:1 altına inmesin diye seçili
/// malzeme kendiliğinden daha opak seviyeye çıkar (Cam → Buzlu → Katı). Hesap onaylı demodakiyle aynıdır:
/// görünen yüzey = taban rengi, opaklığıyla duvar kağıdı ortalamasının üstünde; ikincil metin = metin rengi, alfasıyla
/// görünen yüzeyin üstünde. Bulanıklık olmadığından duvar kağıdının ortalaması yeterli bir tahmindir.
pub struct ReadabilityGuard;

/// Bir yüzeyin taban rengi, birincil metin rengi, ikincil metnin alfası ve bulanıklıksız (katmanlı) olup olmadığı.
/// Bulanıklıksız yüzey `MaterialSetting::layered_opacity`, diğerleri `MaterialSetting::surface_opacity` kullanır.
/// (C#'ta `ReadabilityGuard.Surface` iç içe kayıttı.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub base: Rgb,
    pub text: Rgb,
    pub secondary_alpha: f64,
    pub layered: bool,
}

impl Surface {
    /// `layered` varsayılanı (C#'ta `Layered = false`) kapalı.
    pub const fn new(base: Rgb, text: Rgb, secondary_alpha: f64) -> Self {
        Surface { base, text, secondary_alpha, layered: false }
    }

    pub fn opacity_of(&self, level: MaterialLevel) -> f64 {
        if self.layered {
            MaterialSetting::layered_opacity(level)
        } else {
            MaterialSetting::surface_opacity(level)
        }
    }
}

impl ReadabilityGuard {
    /// WCAG AA normal metin eşiği.
    pub const MIN_SECONDARY_CONTRAST: f64 = 4.5;

    /// Çentik ve dock: #0A0A0B taban, %62 beyaz ikincil metin, bulanıklıksız.
    pub const NOTCH: Surface =
        Surface { base: Rgb::new(10, 10, 11), text: Rgb::new(244, 244, 245), secondary_alpha: 0.62, layered: true };

    /// Koy penceresi, koyu tema.
    pub const WINDOW_DARK: Surface = Surface::new(Rgb::new(22, 22, 25), Rgb::new(244, 244, 245), 0.60);

    /// Koy penceresi, açık tema.
    pub const WINDOW_LIGHT: Surface = Surface::new(Rgb::new(243, 243, 245), Rgb::new(24, 24, 27), 0.62);

    /// Verilen opaklıkta ikincil metnin kontrastı.
    pub fn secondary_contrast(opacity: f64, wallpaper: Rgb, surface: Surface) -> f64 {
        let shown = ColorMath::mix(wallpaper, surface.base, opacity);
        let secondary = ColorMath::mix(shown, surface.text, surface.secondary_alpha);
        ColorMath::contrast(secondary, shown)
    }

    /// Etkin malzeme. Koruma kapalıysa ya da duvar kağıdı bilinmiyorsa (okunamadı) seçilen değer aynen döner.
    /// Aksi halde seçilenden başlayıp opağa doğru eşiği geçen ilk seviye; hiçbiri geçmezse Katı. Opaklık hiç düşmez.
    pub fn effective(level: MaterialLevel, enabled: bool, wallpaper: Option<Rgb>, surface: Surface) -> MaterialLevel {
        let wall = match wallpaper {
            Some(w) if enabled && level != MaterialLevel::Solid => w,
            _ => return level,
        };

        let levels = MaterialSetting::LEVELS;
        let mut start = 0;
        while start < levels.len() && levels[start] != level {
            start += 1;
        }

        for candidate in levels.iter().skip(start) {
            if Self::secondary_contrast(surface.opacity_of(*candidate), wall, surface) >= Self::MIN_SECONDARY_CONTRAST {
                return *candidate;
            }
        }

        MaterialLevel::Solid
    }

    /// BGRA piksel dizisinin (WinRT çözücü çıktısı) ortalama rengi; alfa yok sayılır. Tam piksel yoksa `None`.
    pub fn average_bgra(pixels: &[u8]) -> Option<Rgb> {
        let count = pixels.len() / 4;
        if count == 0 {
            return None;
        }

        let (mut b, mut g, mut r) = (0i64, 0i64, 0i64);
        for px in pixels.chunks_exact(4).take(count) {
            b += px[0] as i64;
            g += px[1] as i64;
            r += px[2] as i64;
        }

        let count = count as i64;
        Some(Rgb::new((r / count) as u8, (g / count) as u8, (b / count) as u8))
    }
}

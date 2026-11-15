use super::utf16_len;
use crate::text;

/// Yazı tipi ve Koy ölçeği kararları (saf mantık, WPF yok). Değerler ayar dosyasından gelir; burada sınırlanır ki
/// bozuk ya da elle düzenlenmiş bir ayar notch tuvalini taşıran bir değer üretmesin (AGENTS #13). Yüklü yazı tipi
/// listesi çağıran tarafından verilir; yazı tipini gerçekten yükleme ve WPF'e bağlama Core dışındadır.
pub struct FontScalePolicy;

impl FontScalePolicy {
    pub const MIN_UI_SCALE: i32 = 80;
    pub const MAX_UI_SCALE: i32 = 150;
    pub const DEFAULT_UI_SCALE: i32 = 100;

    pub const MIN_BASE_FONT_SIZE: f64 = 10.0;
    pub const MAX_BASE_FONT_SIZE: f64 = 18.0;
    pub const DEFAULT_BASE_FONT_SIZE: f64 = 13.0;

    pub const MIN_FONT_WEIGHT: i32 = 100;
    pub const MAX_FONT_WEIGHT: i32 = 900;
    pub const DEFAULT_FONT_WEIGHT: i32 = 400;

    /// Windows'ta yazı tipi ailesi adı en çok 31 karakterdir (LF_FACESIZE - 1); pay bırakılarak 64'te kesilir.
    pub const MAX_FONT_FAMILY_LENGTH: i32 = 64;

    /// Ayarda boş ya da yüklü olmayan yazı tipi adı için kullanılan Koy varsayılanı.
    pub const DEFAULT_FONT_FAMILY: &'static str = "Segoe UI";

    /// Koy ölçeği yüzde olarak 80-150 aralığına sıkıştırılır.
    pub fn clamp_ui_scale(percent: i32) -> i32 {
        text::clamp(percent, Self::MIN_UI_SCALE, Self::MAX_UI_SCALE)
    }

    /// Taban yazı boyu (DIP) sınırlanır. NaN ya da sonsuz değer varsayılana döner.
    pub fn clamp_base_font_size(size: f64) -> f64 {
        if size.is_finite() {
            text::clamp(size, Self::MIN_BASE_FONT_SIZE, Self::MAX_BASE_FONT_SIZE)
        } else {
            Self::DEFAULT_BASE_FONT_SIZE
        }
    }

    /// Kalınlık 100-900 aralığında ve yüzün 100'lük adımlarına yuvarlanır (WPF yalnız bu değerlerle anlamlı).
    pub fn clamp_font_weight(weight: i32) -> i32 {
        let clamped = text::clamp(weight, Self::MIN_FONT_WEIGHT, Self::MAX_FONT_WEIGHT);
        // MidpointRounding.AwayFromZero: f64::round.
        ((clamped as f64 / 100.0).round() * 100.0) as i32
    }

    /// İstenen yazı tipini yüklü listede arar (büyük küçük harf duyarsız) ve listedeki yazımını döndürür.
    /// Boş ya da yüklü olmayan ad (yazı tipi kaldırılmış) varsayılana ([`FontScalePolicy::DEFAULT_FONT_FAMILY`]) düşer.
    pub fn resolve_font_family<I, S>(requested: Option<&str>, installed: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::resolve_font_family_with_fallback(requested, installed, Self::DEFAULT_FONT_FAMILY)
    }

    /// `ResolveFontFamily(requested, installed, fallback)`: boş ya da yüklü olmayan ad `fallback` değerine düşer.
    pub fn resolve_font_family_with_fallback<I, S>(requested: Option<&str>, installed: I, fallback: &str) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let name = requested.map(str::trim);
        // Yazı tipi adları kısadır; elle düzenlenmiş dosyadaki dev bir metni yüklü listeyle karşılaştırmaya gerek yok.
        let name = match name {
            Some(n) if !n.is_empty() && utf16_len(n) <= Self::MAX_FONT_FAMILY_LENGTH as usize => n,
            _ => return fallback.to_string(),
        };

        for font in installed {
            if text::eq_ignore_case(font.as_ref(), name) {
                return font.as_ref().to_string();
            }
        }

        fallback.to_string()
    }

    /// Saat ve sayı yazı tipi. Boşsa ya da yüklü değilse arayüz yazı tipine düşer (Koy varsayılanına değil), böylece
    /// kaldırılmış bir saat yazı tipi yüzünden iki yazı tipi birbirinden kopmaz.
    pub fn resolve_clock_font_family<I, S>(requested: Option<&str>, resolved_ui_family: &str, installed: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if text::is_null_or_white_space(requested) {
            resolved_ui_family.to_string()
        } else {
            Self::resolve_font_family_with_fallback(requested, installed, resolved_ui_family)
        }
    }

    /// Koy ölçeğini bir DIP değerine uygular (yazı boyu, ikon, boşluk). Ölçek sınırlandıktan sonra çarpılır;
    /// monitör DPI'si WPF tarafından ayrıca uygulandığı için burada yalnız Koy'un kendi yüzdesi vardır.
    /// Sayı olmayan DIP 0 döner: NaN bir genişlik WPF yerleşimini bozar.
    pub fn scale_dip(dip: f64, ui_scale_percent: i32) -> f64 {
        if dip.is_finite() { dip * Self::clamp_ui_scale(ui_scale_percent) as f64 / 100.0 } else { 0.0 }
    }

    /// Pencereye uygulanacak son ölçek (1 = %100): istenen yüzde sınırlanır, sonra pencerenin taban boyu çalışma alanına
    /// sığmıyorsa sığacak kadar küçültülür (en az `MIN_UI_SCALE`). Çalışma alanı bilinmiyorsa (0 ya da NaN)
    /// yalnız sınırlanmış yüzde döner.
    pub fn fit_scale(ui_scale_percent: i32, base_width: f64, base_height: f64, work_width: f64, work_height: f64) -> f64 {
        let scale = Self::clamp_ui_scale(ui_scale_percent) as f64 / 100.0;
        if !(base_width > 0.0)
            || !(base_height > 0.0)
            || !(work_width > 0.0)
            || !(work_height > 0.0)
            || !work_width.is_finite()
            || !work_height.is_finite()
        {
            return scale;
        }

        let fit = (work_width / base_width).min(work_height / base_height);
        (Self::MIN_UI_SCALE as f64 / 100.0).max(scale.min(fit))
    }

    /// Taban yazı boyuna göre bir yazı ölçüsünün yeni değeri (oran korunur, 0,5 DIP'e yuvarlanır).
    pub fn scale_font_token(token_at_default: f64, base_font_size: f64) -> f64 {
        let size = Self::clamp_base_font_size(base_font_size);
        // MidpointRounding.AwayFromZero: f64::round.
        (token_at_default * size / Self::DEFAULT_BASE_FONT_SIZE * 2.0).round() / 2.0
    }
}

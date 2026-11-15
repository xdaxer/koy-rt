use super::NotchStyleMode;
use crate::display::WindowPlacementMath;

/// Çentiğin hangi yüzey olarak çizildiği (yüzen adada köşe yarıçapı bunlara göre seçilir).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NotchSurface {
    /// Kapalı, Peek ve ses/parlaklık göstergesi: küçük hap.
    #[default]
    Closed,
    /// Açık çentik.
    Expanded,
    /// Güncelleme ve bildirim kartı.
    Card,
}

/// Çentik biçimi (`NotchStyleMode`) kararları: ekrana yapışık (kulaklı) ya da yüzen ada (kulaksız kapsül, üstten boşluklu).
pub struct NotchStyle;

impl NotchStyle {
    /// Yapışık çentiğin üst köşelerindeki içbükey kulak yarıçapı (DIP).
    pub const PINNED_EAR_RADIUS: f64 = 12.0;

    /// Yüzen adanın ekranın üst kenarından uzaklığı (DIP).
    pub const FLOATING_TOP_OFFSET_DIP: f64 = 9.0;

    /// Açık çentiğin yüzen adadaki köşe yarıçapı.
    pub const FLOATING_EXPANDED_RADIUS: f64 = 30.0;

    /// Kartın yüzen adadaki köşe yarıçapı.
    pub const FLOATING_CARD_RADIUS: f64 = 26.0;

    /// Fare üst kenar bölgesi: ekranın üst kenarından bu kadar piksel (yüzen adada ofsete eklenir).
    pub const EDGE_TRIGGER_PIXELS: i32 = 2;

    pub fn is_floating(value: NotchStyleMode) -> bool {
        value == NotchStyleMode::Floating
    }

    /// Üst köşelerdeki kulak yarıçapı: yapışıkta 12, yüzende 0 (üst köşeler kapsül gibi yuvarlanır).
    pub fn ear_radius(style: NotchStyleMode) -> f64 {
        if Self::is_floating(style) { 0.0 } else { Self::PINNED_EAR_RADIUS }
    }

    /// Pencerenin ekranın üstünden uzaklığı (DIP): yapışıkta 0, yüzende 9.
    pub fn top_offset_dip(style: NotchStyleMode) -> f64 {
        if Self::is_floating(style) { Self::FLOATING_TOP_OFFSET_DIP } else { 0.0 }
    }

    /// Köşe yarıçapı. Yapışıkta mevcut değer aynen; yüzen adada kapalı hâllerde yükseklik/2 (kapsül),
    /// açık çentikte 30, kartta 26.
    pub fn corner_radius(style: NotchStyleMode, surface: NotchSurface, height: f64, pinned_radius: f64) -> f64 {
        if !Self::is_floating(style) {
            return pinned_radius;
        }

        match surface {
            NotchSurface::Expanded => Self::FLOATING_EXPANDED_RADIUS,
            NotchSurface::Card => Self::FLOATING_CARD_RADIUS,
            NotchSurface::Closed => height / 2.0,
        }
    }

    /// Fare kenar bölgesinin alt sınırı (piksel, ekranın üstünden ölçülür): çentiği açan bölge ekranın üst kenarından
    /// başlar ve yüzen adadaki boşluğu da kapsar.
    pub fn edge_trigger_bottom_pixels(style: NotchStyleMode, dpi_scale_y: f64) -> i32 {
        Self::EDGE_TRIGGER_PIXELS + WindowPlacementMath::to_pixels(Self::top_offset_dip(style), dpi_scale_y)
    }
}

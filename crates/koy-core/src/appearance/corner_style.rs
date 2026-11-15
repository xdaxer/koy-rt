#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CornerStyle {
    Sharp,
    Standard,
    Round,
}

/// Koy penceresinin köşe yarıçapları (DIP). Pencere dışındaki yüzeyler (çentik, dock) bundan etkilenmez.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CornerRadii {
    pub window: f64,
    pub row: f64,
    pub control: f64,
    pub small: f64,
}

impl CornerRadii {
    pub const fn new(window: f64, row: f64, control: f64, small: f64) -> Self {
        CornerRadii { window, row, control, small }
    }

    pub fn r#for(style: CornerStyle) -> CornerRadii {
        match style {
            CornerStyle::Sharp => CornerRadii::new(4.0, 4.0, 3.0, 2.0),
            CornerStyle::Round => CornerRadii::new(18.0, 12.0, 10.0, 6.0),
            CornerStyle::Standard => CornerRadii::new(12.0, 8.0, 6.0, 4.0),
        }
    }
}

pub struct CornerSetting;

impl CornerSetting {
    pub const SHARP_VALUE: f64 = 6.0;
    pub const STANDARD_VALUE: f64 = 12.0;
    pub const ROUND_VALUE: f64 = 18.0;

    /// Ayardaki sayıyı seçeneğe çevirir. Eski sürümlerin varsayılanı 18 idi ve hiçbir yerde kullanılmıyordu;
    /// bu yüzden 18 "Yuvarlak" seçimi sayılmaz, Standart olarak okunur. Yuvarlak 17 olarak saklanır.
    pub fn resolve(stored: f64) -> CornerStyle {
        if stored <= 0.0 {
            return CornerStyle::Standard;
        }
        if stored <= 8.0 {
            return CornerStyle::Sharp;
        }
        if (stored > 14.0 && stored < 18.0) || stored > 18.0 {
            return CornerStyle::Round;
        }
        CornerStyle::Standard
    }

    /// Seçeneğin ayar dosyasına yazılan değeri.
    pub fn to_stored(style: CornerStyle) -> f64 {
        match style {
            CornerStyle::Sharp => Self::SHARP_VALUE,
            CornerStyle::Round => 17.0,
            CornerStyle::Standard => Self::STANDARD_VALUE,
        }
    }
}

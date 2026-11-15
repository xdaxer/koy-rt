use crate::text;

/// Parlaklığı hangi yolla ayarlayacağımız.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BrightnessSourceKind {
    /// Hiçbir yol yok: kaydırıcı devre dışı kalır.
    #[default]
    None,
    /// Dahili ekran: WMI (WmiMonitorBrightness / WmiSetBrightness).
    Wmi,
    /// Harici monitör: DDC/CI (Get/SetMonitorBrightness).
    Ddc,
}

/// Çentiğin bulunduğu ekranın türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BrightnessTarget {
    /// Ekran bilinmiyor (sağlayıcı yok ya da eşleşmedi).
    #[default]
    Unknown,
    /// Dizüstü paneli (DisplayConfig çıkış teknolojisi dahili / gömülü).
    Internal,
    /// Harici monitör.
    External,
}

/// Kaynak seçimi: karar ekran düzeni her değiştiğinde yeniden verilir (servis bir kez kurulup ekran
/// kapanınca ölü monitör tanıtıcısında takılmasın). WMI yalnız dahili paneli ayarlar; harici ekranda
/// kullanılırsa başka ekranın parlaklığını değiştirirdi, bu yüzden harici hedefte yalnız DDC denenir.
pub struct BrightnessSourcePicker;

impl BrightnessSourcePicker {
    pub fn pick(target: BrightnessTarget, wmi_available: bool, ddc_available: bool) -> BrightnessSourceKind {
        match target {
            BrightnessTarget::External => {
                if ddc_available {
                    BrightnessSourceKind::Ddc
                } else {
                    BrightnessSourceKind::None
                }
            }
            // Internal ve Unknown (C#'taki `_` kolu) aynı sırayı izler.
            BrightnessTarget::Internal | BrightnessTarget::Unknown => {
                if wmi_available {
                    BrightnessSourceKind::Wmi
                } else if ddc_available {
                    BrightnessSourceKind::Ddc
                } else {
                    BrightnessSourceKind::None
                }
            }
        }
    }

    /// DDC/CI değerini (min..max) yüzdeye çevirir. Aralık bozuksa değeri 0-100'e kıstırır.
    pub fn to_percent(value: i32, min: i32, max: i32) -> i32 {
        if max > min {
            text::round((text::clamp(value, min, max) - min) as f64 * 100.0 / (max - min) as f64) as i32
        } else {
            text::clamp(value, 0, 100)
        }
    }

    /// Yüzdeyi DDC/CI aralığına (min..max) çevirir.
    pub fn from_percent(percent: i32, min: i32, max: i32) -> i32 {
        if max > min {
            min + text::round((text::clamp(percent, 0, 100) * (max - min)) as f64 / 100.0) as i32
        } else {
            text::clamp(percent, 0, 100)
        }
    }
}

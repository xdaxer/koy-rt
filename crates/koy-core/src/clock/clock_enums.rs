use serde::{Deserialize, Serialize};

// Bu dosyadaki enum adları settings.json'a yazılır: yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Saat biçimi. Auto kültürü izler; Hour12 ve Hour24 zorlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ClockHourFormat {
    #[default]
    Auto,
    Hour12,
    Hour24,
}

/// Haftanın ilk günü. System kültürün (Windows bölge ayarının) ilk gününü kullanır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WeekStartSetting {
    #[default]
    System,
    Saturday,
    Sunday,
    Monday,
}

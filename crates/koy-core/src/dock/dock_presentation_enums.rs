use serde::{Deserialize, Serialize};

// Bu dosyadaki enum adları settings.json'a yazılır: yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Gösterge ya da rozet rengi: Windows vurgu rengi, nötr gri ya da kullanıcının seçtiği özel renk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockColorSource {
    #[default]
    Accent,
    Neutral,
    Custom,
}

/// Rozetin simgenin hangi köşesinde durduğu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockBadgeCorner {
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

/// Rozet biçimi: sayı (1–9, 9'dan büyükse 9+) ya da yalnız nokta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockBadgeStyle {
    #[default]
    Number,
    DotOnly,
}

/// Dock'un gizliyken yeniden çıkma tetikleyicisi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockRevealTrigger {
    /// Fare alt kenara değer değmez (gecikme yok).
    #[default]
    EdgeTouch,

    /// Fare alt kenarda `RevealDelayMs` kadar durunca (bugünkü davranış, varsayılan).
    EdgeHold,

    /// Yalnız kısayol; kenar hareketi dock'u açmaz.
    ShortcutOnly,
}

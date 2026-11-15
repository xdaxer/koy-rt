use serde::{Deserialize, Serialize};

// Bu dosyadaki enum adları settings.json'a yazılır: bir kez yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Tema seçimi. "System" Windows uygulama temasını izler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppTheme {
    System,
    Dark,
    Light,
}

/// Malzeme seviyesi: Katı (opak), Buzlu (%86) ya da Cam (%72).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialLevel {
    Solid,
    Frosted,
    Glass,
}

/// Animasyon hızı. `Off` geçişleri anında yapar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnimationSpeed {
    Calm,
    Normal,
    Fast,
    Off,
}

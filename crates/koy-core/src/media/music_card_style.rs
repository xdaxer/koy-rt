use serde::{Deserialize, Serialize};

// Enum adları settings.json'a yazılır: bir kez yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Müzik kartı stili: Düz (sabit nötr kart), Albüm rengi (kapaktan türeyen ton, varsayılan) ya da Albüm kapağı
/// (küçültülmüş kapak arka plan, üstünde koyu perde).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MusicCardStyle {
    Neutral,
    Tinted,
    Artwork,
}

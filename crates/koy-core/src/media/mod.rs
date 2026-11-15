//! Karşılığı: `koy/src/Core/Media/`.
//!
//! Medya servisi sözleşmesi, oynatma saati, uygulama adı, kapak parmak izi ve kapak paleti çıkarımı.
//! Bu modül `appearance` modülüne bağlıdır (`Rgb`, `ColorMath`).

mod cover_fingerprint;
mod cover_palette_extractor;
mod i_media_service;
mod media_app_name;
mod music_card_style;
mod playback_clock;

pub use cover_fingerprint::*;
pub use cover_palette_extractor::*;
pub use i_media_service::*;
pub use media_app_name::*;
pub use music_card_style::*;
pub use playback_clock::*;

#[cfg(test)]
mod tests;

//! Karşılığı: `koy/src/Core/Audio/`.
//!
//! Sistem ses servisi sözleşmesi ve müzik kartı için spektrum çözümleyici (FFT).

pub mod i_audio_service;
pub mod spectrum_analyzer;

pub use i_audio_service::*;
pub use spectrum_analyzer::*;

#[cfg(test)]
mod tests;

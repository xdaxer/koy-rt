//! Karşılığı: `koy/src/Core/Radio/`.
//!
//! Wi-Fi ve Bluetooth radyo durumu sözleşmesi ve Kontrol Merkezi döşemesinin saf mantığı.

pub mod i_radio_service;
pub mod radio_tile;

pub use i_radio_service::*;
pub use radio_tile::*;

#[cfg(test)]
mod tests;

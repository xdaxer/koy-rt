//! Karşılığı: `koy/src/Core/Notch/`.
//!
//! Çentik durum makinesi, panel sırası, boşta gösterim kararları, canlı arka plan politikası ve yerleşim hesapları.
//!
//! Not: `koy/docs/MAP.md` `src/Core/Notch/Effects/` klasörünü (BackgroundEffect, EffectMath…) anar ama bu kaynak
//! ağacında o klasör yok (ne `Notch/Effects/` ne `BackgroundEffectsTests.cs`); bu yüzden `effects` alt modülü yoktur.

mod clock_tick;
mod collapsed_background_policy;
mod control_center_tiles;
mod greeting;
mod home_card_layout;
mod idle_display;
mod idle_display_format;
mod live_background_policy;
mod notch_card;
mod notch_controller;
mod notch_edge_line;
mod notch_enums;
mod notch_home_page;
mod notch_layout;
mod notch_open_policy;
mod notch_panel;
mod notch_state;
mod notch_style;
mod notch_visualizer;

pub use clock_tick::*;
pub use collapsed_background_policy::*;
pub use control_center_tiles::*;
pub use greeting::*;
pub use home_card_layout::*;
pub use idle_display::*;
pub use idle_display_format::*;
pub use live_background_policy::*;
pub use notch_card::*;
pub use notch_controller::*;
pub use notch_edge_line::*;
pub use notch_enums::*;
pub use notch_home_page::*;
pub use notch_layout::*;
pub use notch_open_policy::*;
pub use notch_panel::*;
pub use notch_state::*;
pub use notch_style::*;
pub use notch_visualizer::*;

#[cfg(test)]
mod tests;

//! Karşılığı: `koy/src/Core/StartMenu/`.
//!
//! Başlat düğmesi sağ tık (Win+X) menüsünün modeli ve tıklama işleri; Windows tarafı `IStartMenuHost` arkasında.

mod start_menu_actions;
mod start_menu_model;

pub use start_menu_actions::*;
pub use start_menu_model::*;

#[cfg(test)]
mod tests;

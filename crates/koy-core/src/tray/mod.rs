//! Karşılığı: `koy/src/Core/Tray/`.
//!
//! `TrayMessageParser` SHELLTRAYDATA ikili bloğunu bayt düzeyinde çözer (u8 dilimleri, `from_le_bytes`);
//! düzen C#'takiyle birebirdir.

mod i_tray_service;
mod taskbar_created_policy;
mod tray_balloon_filter;
mod tray_callback;
mod tray_host_lease;
mod tray_host_policy;
mod tray_host_recovery;
mod tray_icon_store;
mod tray_icon_visibility;
mod tray_icon_visibility_policy;
mod tray_menu_anchor;
mod tray_message_parser;
mod tray_notify_data;
mod tray_popup_dismiss_policy;

pub use i_tray_service::*;
pub use taskbar_created_policy::*;
pub use tray_balloon_filter::*;
pub use tray_callback::*;
pub use tray_host_lease::*;
pub use tray_host_policy::*;
pub use tray_host_recovery::*;
pub use tray_icon_store::*;
pub use tray_icon_visibility::*;
pub use tray_icon_visibility_policy::*;
pub use tray_menu_anchor::*;
pub use tray_message_parser::*;
pub use tray_notify_data::*;
pub use tray_popup_dismiss_policy::*;

#[cfg(test)]
mod tests;

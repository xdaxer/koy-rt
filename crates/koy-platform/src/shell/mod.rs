//! Karşılığı: `koy/src/Platform/Shell/`.

mod app_user_model_id_reader;
mod icon_helper;
mod png_encoder;
mod shell_action_service;
pub mod shell_tray_locator;
pub mod taskbar_service;
#[cfg(feature = "watchdog-launcher")]
pub mod watchdog_launcher;

pub use app_user_model_id_reader::*;
pub use icon_helper::*;
pub use shell_action_service::*;

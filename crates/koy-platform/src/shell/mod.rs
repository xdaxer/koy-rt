//! Karşılığı: `koy/src/Platform/Shell/`.

mod app_user_model_id_reader;
mod corner_hook_service;
mod dwm;
mod hotkey_service;
mod hook_thread;
mod icon_helper;
mod keyboard_hook_service;
mod outside_click_hook;
mod tray_icon_service;
mod png_encoder;
mod small;
mod window_styles;
mod shell_action_service;
pub mod shell_tray_locator;
pub mod taskbar_service;
#[cfg(feature = "watchdog-launcher")]
pub mod watchdog_launcher;

pub use app_user_model_id_reader::*;
pub use corner_hook_service::*;
pub use dwm::*;
pub use hotkey_service::*;
pub use icon_helper::*;
pub use keyboard_hook_service::*;
pub use outside_click_hook::*;
pub use tray_icon_service::*;
pub use small::*;
pub use window_styles::WindowStyles;
pub use shell_action_service::*;

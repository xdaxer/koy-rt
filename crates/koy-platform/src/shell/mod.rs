//! Karşılığı: `koy/src/Platform/Shell/`.

mod shell_action_service;
pub mod shell_tray_locator;
pub mod taskbar_service;
#[cfg(feature = "watchdog-launcher")]
pub mod watchdog_launcher;

pub use shell_action_service::*;

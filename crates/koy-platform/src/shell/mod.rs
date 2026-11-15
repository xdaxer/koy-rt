//! Karşılığı: `koy/src/Platform/Shell/`.

pub mod shell_tray_locator;
pub mod taskbar_service;
#[cfg(feature = "watchdog-launcher")]
pub mod watchdog_launcher;

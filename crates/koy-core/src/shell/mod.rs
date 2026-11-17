//! Karşılığı: `koy/src/Core/Shell/`.
//!
//! Görev çubuğu güvenliği ve gözcü (watchdog) karar mantığı burada: `TaskbarStatePolicy` ve `WatchdogPolicy`
//! birebir korunur (AGENTS.md "görev çubuğu güvenliği").

mod fullscreen_rule;
mod i_focus_assist_service;
mod i_fullscreen_detector;
mod i_shell_action_service;
mod i_window_tracker;
mod process_termination_policy;
mod shell_flyouts;
mod startup_path_policy;
mod taskbar_state_policy;
mod taskbar_window_rule;
mod watchdog_policy;
mod window_rect;

pub use fullscreen_rule::*;
pub use i_focus_assist_service::*;
pub use i_fullscreen_detector::*;
pub use i_shell_action_service::*;
pub use i_window_tracker::*;
pub use process_termination_policy::*;
pub use shell_flyouts::*;
pub use startup_path_policy::*;
pub use taskbar_state_policy::*;
pub use taskbar_window_rule::*;
pub use watchdog_policy::*;
pub use window_rect::*;

#[cfg(test)]
mod tests;

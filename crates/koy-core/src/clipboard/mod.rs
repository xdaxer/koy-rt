//! Karşılığı: `koy/src/Core/Clipboard/`.

mod clipboard_classifier;
mod clipboard_display;
mod clipboard_history;
mod clipboard_models;
mod clipboard_pinned_store;
mod guid;
mod i_clipboard_history_service;

pub use clipboard_classifier::*;
pub use clipboard_display::*;
pub use clipboard_history::*;
pub use clipboard_models::*;
pub use clipboard_pinned_store::*;
pub use guid::*;
pub use i_clipboard_history_service::*;

#[cfg(test)]
mod tests;

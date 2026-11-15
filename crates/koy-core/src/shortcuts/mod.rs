//! Karşılığı: `koy/src/Core/Shortcuts/`.
//!
//! Genel kısayollar: `Hotkey` metin biçimi ("Ctrl+Alt+N"), eylem eşlemesi, `ShortcutManager` (çakışma/kayıt kararları) ve
//! düşük düzey klavye kancasının karar mantığı (`KeyboardHookEngine`). Gerçek kayıt/kanca platform katmanındadır.

pub mod hotkey;
pub mod i_hotkey_service;
pub mod i_keyboard_hook_service;
pub mod keyboard_hook_engine;
pub mod shortcut_bindings;
pub mod shortcut_manager;

pub use hotkey::*;
pub use i_hotkey_service::*;
pub use i_keyboard_hook_service::*;
pub use keyboard_hook_engine::*;
pub use shortcut_bindings::*;
pub use shortcut_manager::*;

#[cfg(test)]
mod tests;

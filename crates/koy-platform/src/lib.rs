//! Koy platform katmanı: `koy/src/Platform` karşılığı. Win32/WinRT çağrıları yalnız bu crate'te yapılır;
//! Core'a `HWND` gibi tipler sızmaz (pencere kimliği `isize`).

#![cfg(windows)]

pub mod audio;
pub mod bluetooth;
pub mod calendar;
pub mod clipboard;
pub mod diagnostics;
pub mod display;
pub mod extensions;
pub mod jump_list;
pub mod lyrics;
pub mod media;
pub mod net;
pub mod network;
pub mod notifications;
pub mod power;
pub mod privacy;
pub mod radio;
pub mod search;
pub mod security;
pub mod shelf;
pub mod shell;
pub mod start_menu;
pub mod system_stats;
pub mod voice;
pub mod weather;
pub mod wireless;

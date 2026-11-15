//! Koy platform katmanı: `koy/src/Platform` karşılığı. Win32 çağrıları yalnız bu crate'te yapılır;
//! Core'a `HWND` gibi tipler sızmaz (pencere kimliği `isize`).

#![cfg(windows)]

pub mod security;
pub mod shell;

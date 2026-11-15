//! Karşılığı: `koy/src/Core/Diagnostics/`.

pub mod crash_log;

pub use crash_log::{CrashError, CrashLog};

#[cfg(test)]
mod tests;

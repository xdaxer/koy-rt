//! Karşılığı: `koy/src/Core/Voice/`.
//!
//! Sesli komut: komut çözümleme (kabuksuz, güvenli), harness yürütülebilir dosya çözümleme ve
//! çentikteki oturum durum makinesi.

pub mod harness_executable_resolver;
pub mod i_harness_runner_service;
pub(crate) use crate::win_path as path_util;
pub mod voice_command_parser;
pub mod voice_models;
pub mod voice_session_controller;

pub use harness_executable_resolver::*;
pub use i_harness_runner_service::*;
pub use voice_command_parser::*;
pub use voice_models::*;
pub use voice_session_controller::*;

#[cfg(test)]
mod tests;

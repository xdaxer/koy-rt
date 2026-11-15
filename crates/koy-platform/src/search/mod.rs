//! Karşılığı: `koy/src/Platform/Search/`.

pub(crate) mod com_apartment;
mod default_browser_locator;
mod http_dictionary_service;
pub(crate) mod process_start;
pub(crate) mod win_env;

pub use default_browser_locator::*;
pub use http_dictionary_service::*;

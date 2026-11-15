//! Karşılığı: `koy/src/Platform/Search/`.

pub(crate) mod com_apartment;
mod default_browser_locator;
mod firefox_bookmarks;
mod http_dictionary_service;
pub(crate) mod process_start;
mod search_system_services;
pub(crate) mod win_env;

pub use default_browser_locator::*;
pub use firefox_bookmarks::*;
pub use http_dictionary_service::*;
pub use search_system_services::*;

//! Karşılığı: `koy/src/Platform/Search/`.

pub(crate) mod com_apartment;
mod default_browser_locator;
mod file_search_service;
mod firefox_bookmarks;
mod http_dictionary_service;
mod installed_apps_service;
pub(crate) mod process_start;
mod search_system_services;
pub(crate) mod win_env;
mod windows_spell_checker_service;

pub use default_browser_locator::*;
pub use firefox_bookmarks::*;
pub use http_dictionary_service::*;
pub use search_system_services::*;
pub use file_search_service::*;
pub use installed_apps_service::*;
pub use windows_spell_checker_service::*;

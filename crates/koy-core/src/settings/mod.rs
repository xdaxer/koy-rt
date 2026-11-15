//! Karşılığı: `koy/src/Core/Settings/`.
//!
//! settings.json (şema v4) modeli, okuma/yazma, göç, profiller, arama ve gelişmiş sayfa mantığı.
//! JSON biçimi C# `System.Text.Json` çıktısıyla birebir uyumludur (PascalCase adlar, metin enum'lar, girintili yazım);
//! bkz. `json_tree` ve `setting_value`.

mod advanced_settings;
mod app_language;
mod app_settings;
pub(crate) mod json_list_file;
pub(crate) mod json_tree;
mod legacy_value_translator;
mod policies;
pub mod profiles;
pub(crate) mod setting_value;
mod settings_changes;
mod settings_key_map;
mod settings_migration;
mod settings_search;
mod settings_sections;
mod settings_store;
mod update_channel;
mod value_collections;

pub use advanced_settings::*;
pub use app_language::*;
pub use app_settings::*;
pub use json_list_file::{JsonListFile, in_settings_folder};
pub use policies::*;
pub use profiles::*;
pub use settings_changes::*;
pub use settings_key_map::*;
pub use settings_migration::*;
pub use settings_search::*;
pub use settings_sections::*;
pub use settings_store::*;
pub use update_channel::*;
pub use value_collections::*;

pub use json_tree::JsonError;

#[cfg(test)]
mod tests;

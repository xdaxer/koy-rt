//! Karşılığı: `koy/src/Core/Search/`.
//!
//! Arama paneli mantığı: sorgu/puanlama, kaynaklar, düzen, geçmiş, hesap makinesi, birim ve renk çeviricileri.
//!
//! Rust'a uyarlama notları (C# ile farklar):
//! - `Task`/`async` yerine arka plan iş parçacığı + [`crate::cancel::CancellationToken`] kullanılır; servis
//!   arayüzleri eşzamanlıdır ve arka plan iş parçacığında çağrılır.
//! - `IEnumerable<SearchResult>` dönüşleri hevesli `Vec`'tir.
//! - `decimal` için [`Decimal`] (96 bit mantisli elle yazım); `CultureInfo` için [`crate::clock::Culture`].

mod color_converter_source;
mod decimal;
mod default_browser_search;
mod expression_calculator;
mod file_stamp_cache;
mod jump_list_search_source;
mod language_search_sources;
mod microsoft_store_search_source;
mod search_command_safety;
mod search_history;
mod search_layout;
mod search_matcher;
mod search_models;
mod search_panel_controller;
mod search_sources;
mod search_text;
mod search_util;
mod special_characters_source;
mod system_search_sources;
mod unit_converter;
mod unit_converter_source;
mod uri_util;
mod web_search_sources;

pub use color_converter_source::*;
pub use decimal::*;
pub use default_browser_search::*;
pub use expression_calculator::*;
pub use file_stamp_cache::*;
pub use jump_list_search_source::*;
pub use language_search_sources::*;
pub use microsoft_store_search_source::*;
pub use search_command_safety::*;
pub use search_history::*;
pub use search_layout::*;
pub use search_matcher::*;
pub use search_models::*;
pub use search_panel_controller::*;
pub use search_sources::*;
pub use search_text::*;
pub use special_characters_source::*;
pub use system_search_sources::*;
pub use unit_converter::*;
pub use unit_converter_source::*;
pub use web_search_sources::*;

#[cfg(test)]
mod tests;

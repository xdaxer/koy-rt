//! Karşılığı: `koy/src/Core/Updates/`.
//!
//! Sürüm karşılaştırması, "Neler Geldi" kararları, güncelleme kartı politikası ve CHANGELOG ayrıştırıcısı.
//! `CHANGELOG.md` / `CHANGELOG.tr.md` derlemeye `include_str!` ile gömülür (C#'taki `EmbeddedResource` karşılığı).

mod changelog_parser;
mod update_card_policy;
mod update_flow_policy;

pub use changelog_parser::*;
pub use update_card_policy::*;
pub use update_flow_policy::*;

#[cfg(test)]
mod tests;

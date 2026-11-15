//! Karşılığı: `koy/src/Core/Rules/`.
//!
//! Bağlam kuralları: ham kural (`RuleDraft`) → doğrulama → değerlendirme (ilk eşleşen kazanır) → titreşimsiz profil
//! değiştirme kararı. `rules.json` dosyası bilerek settings.json'ın dışındadır.

pub mod auto_rule;
pub mod context_profile_switcher;
pub mod rule_condition;
pub mod rule_draft;
pub mod rule_evaluator;
pub mod rule_list_editor;
pub mod rule_set_validator;
pub mod rule_sources;
pub mod rule_store;

pub use auto_rule::*;
pub use context_profile_switcher::*;
pub use rule_condition::*;
pub use rule_draft::*;
pub use rule_evaluator::*;
pub use rule_list_editor::*;
pub use rule_set_validator::*;
pub use rule_sources::*;
pub use rule_store::*;

#[cfg(test)]
mod tests;

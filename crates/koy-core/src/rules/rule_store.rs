//! Karşılığı: `Rules/RuleStore.cs`.

use std::path::{Path, PathBuf};

use super::auto_rule::AutoRule;
use super::rule_draft::RuleDraft;
use super::rule_set_validator::RuleSetValidator;
use crate::settings::JsonListFile;

/// Kural dosyası (ayar klasöründe `rules.json`). Bilerek settings.json'ın dışında: profil uygulamak ayar bölümlerini
/// değiştirir, kural listesini değiştiremez (kural → profil → kural döngüsü yapısal olarak yok). Dosya yok = kural yok.
/// Okuma hatasında boş liste, yazma hatasında `false` (bkz. `JsonListFile<T>`).
pub struct RuleStore {
    file: JsonListFile<RuleDraft>,
}

impl RuleStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        RuleStore { file: JsonListFile::new(path.into(), None) }
    }

    /// Ayar klasöründeki `rules.json` (C#: `RuleStore.DefaultPath`).
    pub fn default_path() -> PathBuf {
        crate::settings::in_settings_folder("rules.json")
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Ham kayıtlar (düzenleme için). Boş öğeler atılır.
    ///
    /// C# ile aynı değil (tür sistemi farkı): dosyadaki JSON `null` öğeler okunurken boş taslağa (`RuleDraft::default()`)
    /// dönüşür; C#'taki `Where(d => d is not null)` süzgeci burada bu boş taslakları atar.
    pub fn load_drafts(&self) -> Vec<RuleDraft> {
        self.file.load().into_iter().filter(|d| *d != RuleDraft::default()).collect()
    }

    pub fn save(&self, drafts: &[RuleDraft]) -> bool {
        self.file.save(drafts)
    }

    /// Doğrulanmış, kullanıma hazır kurallar (geçersizler atılır, sıra korunur).
    pub fn load_rules(&self) -> Vec<AutoRule> {
        RuleSetValidator::validate(&self.load_drafts()).rules
    }
}

//! Karşılığı: `Rules/RuleListEditor.cs`.

use super::auto_rule::RuleLimits;
use super::rule_draft::{RuleConditionDraft, RuleDraft};
use super::rule_set_validator::RuleSetValidator;
use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RuleEditError {
    #[default]
    None,
    InvalidProcess,
    NoProfile,
    LimitReached,
    Duplicate,
    NotFound,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleEditResult {
    pub error: RuleEditError,
    pub rules: Vec<RuleDraft>,
}

impl RuleEditResult {
    pub fn is_success(&self) -> bool {
        self.error == RuleEditError::None
    }
}

/// Kural listesi düzenlemesi (arayüz için): ekle, aç/kapat, sil. Liste sırası önceliktir, yeni kural sona eklenir.
/// Yeni kural KAPALI eklenir; kullanıcı açana kadar hiçbir şey uygulanmaz. Dosyada elle yazılmış başka koşullu
/// kurallara dokunulmaz, yalnız gösterilir / açılıp kapatılır / silinir.
pub struct RuleListEditor;

impl RuleListEditor {
    pub fn add_process_rule(rules: &[RuleDraft], process_name: Option<&str>, profile_id: Option<&str>) -> RuleEditResult {
        let Some(process) = RuleSetValidator::normalize_process_name(process_name) else {
            return Self::fail(RuleEditError::InvalidProcess, rules);
        };
        let profile = profile_id.map(text::trim);
        let profile = match profile {
            Some(p) if !p.is_empty() && p.encode_utf16().count() <= RuleLimits::MAX_PROFILE_ID_LENGTH => p,
            _ => return Self::fail(RuleEditError::NoProfile, rules),
        };
        if rules.len() >= RuleLimits::MAX_RULES {
            return Self::fail(RuleEditError::LimitReached, rules);
        }
        // Aynı uygulama için ikinci kural hiç tetiklenmezdi (ilk eşleşen kazanır).
        if rules.iter().any(|r| Self::process_of(r).as_deref() == Some(process.as_str())) {
            return Self::fail(RuleEditError::Duplicate, rules);
        }

        let draft = RuleDraft {
            name: Some(process.clone()),
            profile_id: Some(profile.to_string()),
            enabled: false,
            conditions: vec![RuleConditionDraft::process(Some(&process))],
        };
        let mut all = rules.to_vec();
        all.push(draft);
        RuleEditResult { error: RuleEditError::None, rules: all }
    }

    pub fn set_enabled(rules: &[RuleDraft], index: i32, enabled: bool) -> RuleEditResult {
        if index < 0 || index as usize >= rules.len() {
            return Self::fail(RuleEditError::NotFound, rules);
        }
        let mut copy = rules.to_vec();
        copy[index as usize] = RuleDraft { enabled, ..copy[index as usize].clone() };
        RuleEditResult { error: RuleEditError::None, rules: copy }
    }

    pub fn delete(rules: &[RuleDraft], index: i32) -> RuleEditResult {
        if index < 0 || index as usize >= rules.len() {
            return Self::fail(RuleEditError::NotFound, rules);
        }
        let mut copy = rules.to_vec();
        copy.remove(index as usize);
        RuleEditResult { error: RuleEditError::None, rules: copy }
    }

    /// Kuralın tek süreç koşulu varsa normalleştirilmiş süreç adı; aksi hâlde `None` (gelişmiş kural).
    pub fn process_of(rule: &RuleDraft) -> Option<String> {
        if rule.conditions.len() != 1 {
            return None;
        }
        let c = &rule.conditions[0];
        if !c.kind.as_deref().is_some_and(|k| text::eq_ignore_case(text::trim(k), "process")) {
            return None;
        }
        RuleSetValidator::normalize_process_name(c.process_name.as_deref())
    }

    fn fail(error: RuleEditError, rules: &[RuleDraft]) -> RuleEditResult {
        RuleEditResult { error, rules: rules.to_vec() }
    }
}

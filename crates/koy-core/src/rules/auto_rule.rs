//! Karşılığı: `Rules/AutoRule.cs`.

use super::rule_condition::RuleCondition;

/// Doğrulanmış bir kural. Listedeki sıra önceliktir: değerlendirmede ilk eşleşen kazanır.
/// `profile_id` yalnızca metin kimliğidir; profilin varlığı burada denetlenmez.
/// Koşulsuz kural hiçbir zaman eşleşmez (`RuleEvaluator`); doğrulayıcı da böylelerini üretmez.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoRule {
    pub name: String,
    pub profile_id: String,
    pub enabled: bool,
    pub conditions: Vec<RuleCondition>,
}

impl AutoRule {
    pub fn new(name: impl Into<String>, profile_id: impl Into<String>, enabled: bool, conditions: Vec<RuleCondition>) -> Self {
        AutoRule { name: name.into(), profile_id: profile_id.into(), enabled, conditions }
    }
}

/// Kural kümesi sınırları.
pub struct RuleLimits;

impl RuleLimits {
    pub const MAX_RULES: usize = 50;
    pub const MAX_NAME_LENGTH: usize = 64;
    pub const MAX_DISPLAY_COUNT: i32 = 16;
    // El ile düzenlenmiş dosya binlerce koşul ya da megabaytlık dize taşıyabilir; ayrıştırma bunlara göre büyümesin.
    pub const MAX_CONDITIONS_PER_RULE: usize = 8;
    pub const MAX_PROFILE_ID_LENGTH: usize = 64;
    pub const MAX_PROCESS_NAME_LENGTH: usize = 260;
}

//! Karşılığı: `Rules/RuleSources.cs`.

use std::ops::{BitAnd, BitOr, BitOrAssign};

use super::auto_rule::AutoRule;
use super::rule_condition::RuleCondition;

/// Kuralların hangi bağlam ölçümlerine ihtiyaç duyduğu (C# `[Flags] enum`). Ölçüm yalnız gerekince yapılır (AGENTS #5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RuleSource(pub i32);

#[allow(non_upper_case_globals)]
impl RuleSource {
    pub const None: RuleSource = RuleSource(0);
    pub const Process: RuleSource = RuleSource(1);
    pub const Time: RuleSource = RuleSource(2);
    pub const DisplayCount: RuleSource = RuleSource(4);
    pub const Fullscreen: RuleSource = RuleSource(8);

    /// `HasFlag`: verilen bayrakların hepsi açık mı.
    pub fn has_flag(self, flag: RuleSource) -> bool {
        self.0 & flag.0 == flag.0
    }
}

impl BitOr for RuleSource {
    type Output = RuleSource;
    fn bitor(self, rhs: RuleSource) -> RuleSource {
        RuleSource(self.0 | rhs.0)
    }
}
impl BitOrAssign for RuleSource {
    fn bitor_assign(&mut self, rhs: RuleSource) {
        self.0 |= rhs.0;
    }
}
impl BitAnd for RuleSource {
    type Output = RuleSource;
    fn bitand(self, rhs: RuleSource) -> RuleSource {
        RuleSource(self.0 & rhs.0)
    }
}

pub struct RuleSources;

impl RuleSources {
    /// Etkin kuralların koşullarının birleşimi. Etkin kural yoksa `RuleSource::None` (hiçbir şey izlenmez).
    pub fn needed(rules: &[AutoRule]) -> RuleSource {
        let mut needed = RuleSource::None;
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            for condition in &rule.conditions {
                needed |= match condition {
                    RuleCondition::Process(_) => RuleSource::Process,
                    RuleCondition::TimeWindow(_) => RuleSource::Time,
                    RuleCondition::DisplayCount(_) => RuleSource::DisplayCount,
                    RuleCondition::Fullscreen(_) => RuleSource::Fullscreen,
                    RuleCondition::Unknown => RuleSource::None,
                };
            }
        }
        needed
    }
}

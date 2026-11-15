//! Karşılığı: `Rules/RuleEvaluator.cs`.

use chrono::NaiveTime;

use super::auto_rule::AutoRule;
use super::rule_condition::RuleCondition;
use super::rule_set_validator::RuleSetValidator;

/// Bir andaki bağlam görüntüsü. Ölçüm (süreç izleme, saat, ekran sayımı) Core dışında yapılır.
/// `process_name` yalnız dosya adı olmalı (yol değil; yol içeren ad hiçbir süreç koşuluyla eşleşmez).
/// `local_time` UTC değil, makinenin yerel saatidir; saat dilimi ve yaz saati dönüşümü çağıranın işi.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleContext {
    pub process_name: Option<String>,
    pub local_time: NaiveTime,
    pub display_count: i32,
    pub is_fullscreen: bool,
}

impl RuleContext {
    pub fn new(process_name: Option<&str>, local_time: NaiveTime, display_count: i32, is_fullscreen: bool) -> Self {
        RuleContext { process_name: process_name.map(String::from), local_time, display_count, is_fullscreen }
    }
}

/// Saf karar mantığı: kuralları listedeki sırayla dener. Etkin ve bütün koşulları sağlayan
/// ilk kuralın profil kimliğini döner; hiçbiri eşleşmezse `None`. Sıra belirlidir, aynı girdi
/// her zaman aynı sonucu verir. Çelişen kurallarda öncelik yalnız liste sırasıdır.
/// Koşulsuz kural ve tanınmayan koşul türü eşleşmez (fail-closed).
pub struct RuleEvaluator;

impl RuleEvaluator {
    pub fn evaluate(rules: &[AutoRule], context: &RuleContext) -> Option<String> {
        let process = RuleSetValidator::normalize_process_name(context.process_name.as_deref());
        for rule in rules {
            if !rule.enabled || rule.conditions.is_empty() {
                continue;
            }
            if rule.conditions.iter().all(|condition| Self::matches(condition, context, process.as_deref())) {
                return Some(rule.profile_id.clone());
            }
        }
        None
    }

    fn matches(condition: &RuleCondition, context: &RuleContext, process: Option<&str>) -> bool {
        match condition {
            RuleCondition::Process(p) => {
                process.is_some() && RuleSetValidator::normalize_process_name(Some(&p.process_name)).as_deref() == process
            }
            RuleCondition::TimeWindow(w) => Self::in_window(context.local_time, w.from, w.to),
            RuleCondition::DisplayCount(d) => context.display_count == d.count,
            RuleCondition::Fullscreen(f) => context.is_fullscreen == f.is_fullscreen,
            RuleCondition::Unknown => false,
        }
    }

    /// `from` dahil, `to` hariç. `from > to` ise aralık gece yarısını aşar (ör. 22:00-06:00). `from == to` belirsizdir, eşleşmez.
    fn in_window(now: NaiveTime, from: NaiveTime, to: NaiveTime) -> bool {
        if from == to {
            return false;
        }
        if from < to { now >= from && now < to } else { now >= from || now < to }
    }
}

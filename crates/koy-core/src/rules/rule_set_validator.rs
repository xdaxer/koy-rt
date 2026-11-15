//! Karşılığı: `Rules/RuleSetValidator.cs`.

use chrono::NaiveTime;

use super::auto_rule::{AutoRule, RuleLimits};
use super::rule_condition::*;
use super::rule_draft::{RuleConditionDraft, RuleDraft};
use crate::text;

/// Doğrulama sonucu: geçerli kurallar (sırası korunmuş) ve atılan öğelerin sayısı.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleSetValidation {
    pub rules: Vec<AutoRule>,
    pub dropped_rules: i32,
    pub dropped_conditions: i32,
}

/// C# `string.Length` (UTF-16 kod birimi sayısı).
fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Ham kuralları doğrular. Kurala fail-closed davranılır: bir kuralın koşullarından biri bile geçersiz
/// ya da bilinmeyen türdeyse kural tamamen atılır, kalan koşullarıyla tutulmaz. Geçerli koşullar
/// kalsa da kural ancak tam hâliyle anlamlıdır. Koşulsuz kural her şeyle eşleşeceği için de kabul
/// edilmez. Fazla kurallar (üst sınır) listenin sonundan atılır ve yine raporlanır.
/// El ile düzenlenmiş dosyadan gelen boş liste/öğe ve aşırı büyük girdi de kuralı atar, istisna fırlatmaz.
/// Saat aralığı "HH:mm" olmalı; "24:00" yoktur, gün sonuna kadar olan aralık için bitiş "00:00" yazılır.
pub struct RuleSetValidator;

impl RuleSetValidator {
    pub fn validate(drafts: &[RuleDraft]) -> RuleSetValidation {
        let mut rules: Vec<AutoRule> = Vec::new();
        let mut dropped_rules = 0;
        let mut dropped_conditions = 0;

        for (i, draft) in drafts.iter().enumerate() {
            // Üst sınıra ulaşıldıysa kalanların koşullarını ayrıştırmaya gerek yok.
            if rules.len() >= RuleLimits::MAX_RULES {
                dropped_rules += (drafts.len() - i) as i32;
                break;
            }

            let name = draft.name.as_deref().map(text::trim);
            let profile_id = draft.profile_id.as_deref().map(text::trim);
            let (name, profile_id) = match (name, profile_id) {
                (Some(n), Some(p))
                    if !n.is_empty()
                        && utf16_len(n) <= RuleLimits::MAX_NAME_LENGTH
                        && !p.is_empty()
                        && utf16_len(p) <= RuleLimits::MAX_PROFILE_ID_LENGTH
                        && draft.conditions.len() <= RuleLimits::MAX_CONDITIONS_PER_RULE =>
                {
                    (n, p)
                }
                _ => {
                    dropped_rules += 1;
                    continue;
                }
            };

            let mut conditions: Vec<RuleCondition> = Vec::new();
            let mut has_invalid_condition = false;
            for condition_draft in &draft.conditions {
                match Self::parse_condition(condition_draft) {
                    None => {
                        dropped_conditions += 1;
                        has_invalid_condition = true;
                    }
                    Some(c) => conditions.push(c),
                }
            }

            // Bir koşul bile geçersizse kalanlarla kurmak yanlış tetiklemeye yol açar:
            // "süreç=oyun VE saat=25:00-06:00" yalnız "süreç=oyun" olursa gün boyu uygulanır.
            if has_invalid_condition || conditions.is_empty() {
                dropped_rules += 1;
                continue;
            }

            rules.push(AutoRule { name: name.to_string(), profile_id: profile_id.to_string(), enabled: draft.enabled, conditions });
        }

        RuleSetValidation { rules, dropped_rules, dropped_conditions }
    }

    /// Süreç adını karşılaştırılabilir hâle getirir: kırpılmış, küçük harf, sonundaki .exe atılmış.
    /// Yol ayırıcı içeren ya da boş ad geçersizdir (süreç adı bir dosya adıdır).
    pub fn normalize_process_name(raw: Option<&str>) -> Option<String> {
        let mut name = text::to_lower_invariant(text::trim(raw?));
        if name.ends_with(".exe") {
            name.truncate(name.len() - 4);
        }
        if name.is_empty()
            || utf16_len(&name) > RuleLimits::MAX_PROCESS_NAME_LENGTH
            || name.contains(['\\', '/', ':'])
        {
            return None;
        }
        Some(name)
    }

    fn parse_condition(draft: &RuleConditionDraft) -> Option<RuleCondition> {
        let kind = draft.kind.as_deref().map(|k| text::to_lower_invariant(text::trim(k)));
        match kind.as_deref() {
            Some("process") => {
                let process = Self::normalize_process_name(draft.process_name.as_deref())?;
                Some(ProcessCondition::new(process).into())
            }
            Some("timewindow") => {
                // Başlangıç ve bitiş aynıysa aralık boştur ya da tüm gün olur; belirsiz olduğu için atılır.
                let from = Self::try_parse_time(draft.from.as_deref())?;
                let to = Self::try_parse_time(draft.to.as_deref())?;
                if from != to { Some(TimeWindowCondition::new(from, to).into()) } else { None }
            }
            Some("displaycount") => match draft.display_count {
                Some(c) if (1..=RuleLimits::MAX_DISPLAY_COUNT).contains(&c) => Some(DisplayCountCondition::new(c).into()),
                _ => None,
            },
            Some("fullscreen") => draft.is_fullscreen.map(|f| FullscreenCondition::new(f).into()),
            _ => None,
        }
    }

    /// `TimeOnly.TryParseExact(raw?.Trim(), "HH:mm", InvariantCulture)`: tam iki haneli saat ve dakika.
    fn try_parse_time(raw: Option<&str>) -> Option<NaiveTime> {
        let b = text::trim(raw?).as_bytes();
        if b.len() != 5 || b[2] != b':' {
            return None;
        }
        let digit = |c: u8| if c.is_ascii_digit() { Some((c - b'0') as u32) } else { None };
        let hour = digit(b[0])? * 10 + digit(b[1])?;
        let minute = digit(b[3])? * 10 + digit(b[4])?;
        NaiveTime::from_hms_opt(hour, minute, 0)
    }
}

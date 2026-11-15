//! Karşılığı: `Rules/RuleDraft.cs`.
//!
//! JSON: `rules.json` C# sürümüyle birebir uyumlu (PascalCase alan adları; kaynak üreticili `DataJsonContext`,
//! adlandırma ilkesi yok). Eksik alan C#'taki gibi varsayılan değeri alır; JSON `null` liste boş sayılır,
//! `null` öğe "boş" taslağa dönüşür (aşağıya bakın).

use serde::{Deserialize, Deserializer, Serialize};

/// JSON `null` → `T::default()`.
fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// Ham kural: arayüzden ya da dosyadan gelir, doğrulanmadan kullanılmaz.
/// Alanlar eksik ya da bozuk olabilir; `RuleSetValidator` bunları ayıklar.
///
/// C# ile aynı değil (tür sistemi farkı): C#'ta liste/öğe `null` olabilir ve doğrulayıcı bunları atar. Burada JSON `null`
/// liste boş listeye, `null` öğe (`RuleDraft`) tamamen boş taslağa (`RuleDraft::default()`) dönüşür; doğrulayıcının
/// sonucu (atılan kural/koşul sayıları dahil) aynıdır.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", from = "MaybeRuleDraft")]
pub struct RuleDraft {
    pub name: Option<String>,
    pub profile_id: Option<String>,
    pub enabled: bool,
    pub conditions: Vec<RuleConditionDraft>,
}

impl RuleDraft {
    pub fn new(name: Option<&str>, profile_id: Option<&str>, enabled: bool, conditions: Vec<RuleConditionDraft>) -> Self {
        RuleDraft { name: name.map(String::from), profile_id: profile_id.map(String::from), enabled, conditions }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct RuleDraftRaw {
    name: Option<String>,
    profile_id: Option<String>,
    enabled: bool,
    #[serde(deserialize_with = "null_as_default")]
    conditions: Vec<RuleConditionDraft>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MaybeRuleDraft {
    Value(RuleDraftRaw),
    Null(()),
}

impl From<MaybeRuleDraft> for RuleDraft {
    fn from(m: MaybeRuleDraft) -> Self {
        match m {
            MaybeRuleDraft::Value(r) => RuleDraft { name: r.name, profile_id: r.profile_id, enabled: r.enabled, conditions: r.conditions },
            MaybeRuleDraft::Null(()) => RuleDraft::default(),
        }
    }
}

/// Ham koşul. `kind` bilinmeyen bir metin olabilir (ileri sürümden kalan dosya);
/// tanınmayan tür sessizce atılır, uygulama açılışını bozmaz.
/// Tür adları: process, timeWindow, displayCount, fullscreen (büyük-küçük harf duyarsız).
///
/// C# ile aynı değil (tür sistemi farkı): JSON `null` öğe, `kind`'ı boş (`None`) bir koşula dönüşür; doğrulayıcı bunu
/// `null` koşul gibi atar ve kuralı da düşürür.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", from = "MaybeRuleConditionDraft")]
pub struct RuleConditionDraft {
    pub kind: Option<String>,
    pub process_name: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub display_count: Option<i32>,
    pub is_fullscreen: Option<bool>,
}

impl RuleConditionDraft {
    /// Yalnız `kind` dolu, diğer alanlar boş (C#'ta `new RuleConditionDraft("battery")`).
    pub fn new(kind: Option<&str>) -> Self {
        RuleConditionDraft { kind: kind.map(String::from), ..Default::default() }
    }
    pub fn process(process_name: Option<&str>) -> Self {
        RuleConditionDraft { kind: Some("process".into()), process_name: process_name.map(String::from), ..Default::default() }
    }
    pub fn time_window(from: Option<&str>, to: Option<&str>) -> Self {
        RuleConditionDraft {
            kind: Some("timeWindow".into()),
            from: from.map(String::from),
            to: to.map(String::from),
            ..Default::default()
        }
    }
    pub fn display_count(count: Option<i32>) -> Self {
        RuleConditionDraft { kind: Some("displayCount".into()), display_count: count, ..Default::default() }
    }
    pub fn fullscreen(is_fullscreen: Option<bool>) -> Self {
        RuleConditionDraft { kind: Some("fullscreen".into()), is_fullscreen, ..Default::default() }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct RuleConditionDraftRaw {
    kind: Option<String>,
    process_name: Option<String>,
    from: Option<String>,
    to: Option<String>,
    display_count: Option<i32>,
    is_fullscreen: Option<bool>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MaybeRuleConditionDraft {
    Value(RuleConditionDraftRaw),
    Null(()),
}

impl From<MaybeRuleConditionDraft> for RuleConditionDraft {
    fn from(m: MaybeRuleConditionDraft) -> Self {
        match m {
            MaybeRuleConditionDraft::Value(r) => RuleConditionDraft {
                kind: r.kind,
                process_name: r.process_name,
                from: r.from,
                to: r.to,
                display_count: r.display_count,
                is_fullscreen: r.is_fullscreen,
            },
            MaybeRuleConditionDraft::Null(()) => RuleConditionDraft::default(),
        }
    }
}

//! Karşılığı: `Rules/ContextProfileSwitcher.cs`.

use super::auto_rule::AutoRule;
use super::rule_evaluator::{RuleContext, RuleEvaluator};
use crate::time::{DateTimeOffset, TimeSpan, span_between};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SwitchActionKind {
    /// Yapılacak bir şey yok.
    #[default]
    None,

    /// Karar henüz kararlı değil; `SwitchDecision::delay` sonra `ContextProfileSwitcher::tick` çağrılmalı.
    Wait,

    /// Kuralın profili uygulanmalı.
    Apply,

    /// Kural artık eşleşmiyor; kuraldan önceki duruma dönülmeli.
    Revert,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SwitchDecision {
    pub kind: SwitchActionKind,
    pub profile_id: Option<String>,
    pub delay: TimeSpan,
}

impl SwitchDecision {
    /// C#: `new SwitchDecision(kind, profileId = null, delay = default)`.
    pub fn new(kind: SwitchActionKind, profile_id: Option<&str>, delay: TimeSpan) -> Self {
        SwitchDecision { kind, profile_id: profile_id.map(String::from), delay }
    }

    /// C#: `SwitchDecision.None`.
    pub fn none() -> Self {
        SwitchDecision::new(SwitchActionKind::None, None, TimeSpan::ZERO)
    }
}

/// Bağlam kurallarının "ne zaman profil değiştirilir" kararı. Saf ve durumlu; saat, zamanlayıcı ya da pencere yok,
/// zaman dışarıdan verilir. Üç güvence:
/// - Titreşim önleme: bir eylem ancak aynı eylem `stable_for` süre kesintisiz istenirse verilir; hızlı alt-tab
///   bekleyen eylemi iptal eder (ön plan eski duruma dönünce eylem kalmaz), çentik ve dock boşuna yeniden kurulmaz.
/// - Aynı profil iki kez uygulanmaz: kuralla uygulanmış profil yürürlükteyken aynı hedef için eylem üretilmez.
/// - Geri dönüş: hiçbir kural eşleşmiyorsa ve bir kural profili yürürlükteyse `Revert`;
///   kural yokken ya da kural profili yürürlükte değilken asla (kullanıcının kendi ayarına dokunulmaz).
///
/// Döngü yok: yalnız `observe` (dışarıdan gelen ön plan olayı) karar üretir, `note_*` çağrıları üretmez.
/// Bu sınıf kural listesini değiştirmez; profil uygulamak kuralları değiştiremez (kurallar ayrı dosyada).
/// "Neye dönülecek"in kendisi (ayar anlık görüntüsü) çağıranda tutulur; burada yalnız dönülüp dönülmeyeceği bilinir.
pub struct ContextProfileSwitcher {
    stable_for: TimeSpan,
    rules: Vec<AutoRule>,
    pending_kind: SwitchActionKind,
    pending_profile: Option<String>,
    pending_since: DateTimeOffset,
    active_rule_profile_id: Option<String>,
}

impl ContextProfileSwitcher {
    /// C#: `DefaultStableFor`.
    pub const DEFAULT_STABLE_FOR: TimeSpan = TimeSpan::from_secs(1);

    pub fn new(stable_for: TimeSpan) -> Self {
        ContextProfileSwitcher {
            stable_for,
            rules: Vec::new(),
            pending_kind: SwitchActionKind::None,
            pending_profile: None,
            pending_since: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset(),
            active_rule_profile_id: None,
        }
    }

    /// C#: parametresiz kurucu (`DefaultStableFor`).
    pub fn with_default_stable_for() -> Self {
        Self::new(Self::DEFAULT_STABLE_FOR)
    }

    /// Kural tarafından uygulanmış ve hâlâ yürürlükte olan profil; yoksa `None`.
    pub fn active_rule_profile_id(&self) -> Option<&str> {
        self.active_rule_profile_id.as_deref()
    }

    pub fn is_rule_active(&self) -> bool {
        self.active_rule_profile_id.is_some()
    }

    /// Kural listesi değişti. Bekleyen eylem düşer; yürürlükteki kural profili bilgisi korunur.
    pub fn set_rules(&mut self, rules: &[AutoRule]) {
        self.rules = rules.to_vec();
        self.clear_pending();
    }

    /// Yeni bir ön plan / bağlam görüntüsü. Eylem varsa ya hemen (kararlıysa) ya da bekleme olarak döner.
    pub fn observe(&mut self, context: &RuleContext, now: DateTimeOffset) -> SwitchDecision {
        let target = RuleEvaluator::evaluate(&self.rules, context);

        let kind = match &target {
            Some(t) => {
                if self.active_rule_profile_id.as_deref() == Some(t.as_str()) {
                    SwitchActionKind::None
                } else {
                    SwitchActionKind::Apply
                }
            }
            None => {
                if self.is_rule_active() {
                    SwitchActionKind::Revert
                } else {
                    SwitchActionKind::None
                }
            }
        };

        if kind == SwitchActionKind::None {
            self.clear_pending();
            return SwitchDecision::none();
        }

        let profile = if kind == SwitchActionKind::Apply { target } else { None };
        if self.pending_kind != kind || self.pending_profile != profile {
            self.pending_kind = kind;
            self.pending_profile = profile;
            self.pending_since = now;
        }

        self.resolve(now)
    }

    /// Bekleme süresi dolduğunda çağrılır. Hâlâ erkense yine bekleme, bekleyen yoksa `None` döner.
    pub fn tick(&mut self, now: DateTimeOffset) -> SwitchDecision {
        if self.pending_kind == SwitchActionKind::None { SwitchDecision::none() } else { self.resolve(now) }
    }

    /// Kuralın profili uygulandı: yürürlüğe girdi.
    pub fn note_applied(&mut self, profile_id: &str) {
        self.active_rule_profile_id = Some(profile_id.to_string());
        self.clear_pending();
    }

    /// Kuraldan önceki duruma dönüldü.
    pub fn note_reverted(&mut self) {
        self.active_rule_profile_id = None;
        self.clear_pending();
    }

    /// Kullanıcı profili kendisi uyguladı (ya da kurallar kapandı): kural yürürlükte sayılmaz, geri dönüş yapılmaz,
    /// kullanıcının seçimi ezilmez.
    pub fn reset(&mut self) {
        self.active_rule_profile_id = None;
        self.clear_pending();
    }

    fn resolve(&mut self, now: DateTimeOffset) -> SwitchDecision {
        let remaining = self.stable_for - span_between(now, self.pending_since);
        if remaining > TimeSpan::ZERO {
            return SwitchDecision::new(SwitchActionKind::Wait, None, remaining);
        }

        let decision = SwitchDecision { kind: self.pending_kind, profile_id: self.pending_profile.clone(), delay: TimeSpan::ZERO };
        self.clear_pending();
        decision
    }

    fn clear_pending(&mut self) {
        self.pending_kind = SwitchActionKind::None;
        self.pending_profile = None;
    }
}

impl Default for ContextProfileSwitcher {
    fn default() -> Self {
        Self::with_default_stable_for()
    }
}

//! Karşılığı: `koy/src/Core/Updates/UpdateCardPolicy.cs`.

use std::sync::Arc;

use crate::text::is_null_or_white_space;
use crate::time::{span_between, DateTimeOffset, SystemTimeProvider, TimeProvider, TimeSpan};

/// Çentikteki "yeni sürüm var" kartının ne zaman çıkacağına karar verir. Saf mantık; durum yalnız bu oturum için bellekte tutulur.
/// Kural: bir sürüm için oturumda en çok bir kez çıkar, × ile kapatılan sürüm bir daha çıkmaz, farklı bir sürüm yeniden çıkabilir,
/// çentik açıkken bekler (kapanınca gösterilir), gösterildikten 15 sn sonra kendiliğinden kapanır.
pub struct UpdateCardPolicy {
    clock: Arc<dyn TimeProvider>,
    shown_version: Option<String>,
    dismissed_version: Option<String>,
    shown_at: Option<DateTimeOffset>,
}

impl UpdateCardPolicy {
    /// Kartın kendiliğinden kapanma süresi.
    pub const AUTO_DISMISS_AFTER: TimeSpan = TimeSpan::from_secs(15);

    /// `clock` verilmezse (`null`) gerçek saat (`TimeProvider.System`).
    pub fn new(clock: Option<Arc<dyn TimeProvider>>) -> Self {
        UpdateCardPolicy {
            clock: clock.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
            shown_version: None,
            dismissed_version: None,
            shown_at: None,
        }
    }

    /// Kart şimdi gösterilmeli mi. Çentik açıkken false döner; çağıran çentik kapanınca yeniden sorar.
    pub fn should_show(&self, has_update: bool, version: Option<&str>, notch_expanded: bool) -> bool {
        let version = match version {
            Some(v) if has_update && !is_null_or_white_space(Some(v)) && !notch_expanded => v,
            _ => return false,
        };

        Some(version) != self.dismissed_version.as_deref() && Some(version) != self.shown_version.as_deref()
    }

    /// Kart gösterildi: bu sürüm için oturumda ikinci kez çıkmaz.
    pub fn mark_shown(&mut self, version: &str) {
        self.shown_version = Some(version.to_string());
        self.shown_at = Some(self.clock.utc_now());
    }

    /// Kullanıcı × ile kapattı: bu sürüm bu oturumda bir daha gösterilmez.
    pub fn dismiss(&mut self, version: &str) {
        self.dismissed_version = Some(version.to_string());
    }

    /// Gösterilen kartın süresi doldu mu.
    pub fn is_expired(&self) -> bool {
        match (&self.shown_version, self.shown_at) {
            (Some(_), Some(at)) => span_between(self.clock.utc_now(), at) >= Self::AUTO_DISMISS_AFTER,
            _ => false,
        }
    }
}

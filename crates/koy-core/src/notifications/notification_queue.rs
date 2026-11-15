use std::sync::Arc;

use super::notification_item::NotificationItem;
use crate::text;
use crate::time::{SystemTimeProvider, TimeProvider, TimeSpan, span_between};

/// Çentikte gösterilecek bildirimlerin kuyruğu. Aynı anda tek kart; art arda gelenler sırada bekler (en çok
/// [`NotificationQueue::MAX_PENDING`], dolunca en eskisi düşer); çentik açıkken kart alınmaz; Koy'un kendi bildirimleri ve
/// Rahatsız Etme açıkken gelenler hiç kuyruğa girmez. Saf mantık; yalnız UI iş parçacığında kullanılır.
pub struct NotificationQueue {
    clock: Arc<dyn TimeProvider>,
    own_app_name: String,
    pending: Vec<NotificationItem>,
    current: Option<NotificationItem>,
}

impl NotificationQueue {
    pub const DEFAULT_OWN_APP_NAME: &'static str = "Koy";

    /// Bir kartın ekranda kalma süresi; kapanışı kartın sahibi (OSD zamanlayıcısı) yürütür.
    pub const CARD_DURATION: TimeSpan = TimeSpan::from_secs(6);

    /// Bekleyen kart üst sınırı; kart gösterilirken sayılmaz.
    pub const MAX_PENDING: usize = 5;

    /// Bu süreden eski bildirim kartı artık anlamsızdır (çentik uzun süre açık kaldıysa): gösterilmeden düşer.
    pub const MAX_AGE: TimeSpan = TimeSpan::from_mins(2);

    /// `clock` verilmezse `TimeProvider.System`; `ownAppName` varsayılanı [`Self::DEFAULT_OWN_APP_NAME`].
    pub fn new(clock: Option<Arc<dyn TimeProvider>>) -> Self {
        Self::with_own_app_name(clock, Self::DEFAULT_OWN_APP_NAME)
    }

    pub fn with_own_app_name(clock: Option<Arc<dyn TimeProvider>>, own_app_name: &str) -> Self {
        NotificationQueue {
            clock: clock.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
            own_app_name: own_app_name.to_owned(),
            pending: Vec::new(),
            current: None,
        }
    }

    /// Şu an kartı gösterilen bildirim; yoksa `None`.
    pub fn current(&self) -> Option<&NotificationItem> {
        self.current.as_ref()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Süzgeçten geçerse kuyruğa ekler. Süzülen: Koy'un kendi bildirimi, Rahatsız Etme açıkken gelen, boş içerikli,
    /// aynı kimlikle zaten kuyrukta ya da gösterilen. Dönüş: eklendi mi.
    pub fn enqueue(&mut self, item: NotificationItem, do_not_disturb: bool) -> bool {
        if do_not_disturb || self.is_own(&item) || Self::is_empty(&item) {
            return false;
        }

        // Aynı kimlik ama ilerlemiş zaman = yerine konan yeni bildirim (kuyruğa girer); aynı damga = yinelenen okuma.
        if Self::is_same(self.current.as_ref(), &item) || self.pending.iter().any(|p| Self::is_same(Some(p), &item)) {
            return false;
        }

        self.pending.push(item);
        if self.pending.len() > Self::MAX_PENDING {
            self.pending.remove(0);
        }
        true
    }

    /// Sıradaki kartı gösterilmek üzere alır; yoksa `None`. Zaten bir kart gösteriliyorsa ya da çentik açıksa
    /// (çağıran kapanınca yeniden sorar) `None` döner. Çok eskimiş öğeler atlanır.
    pub fn try_take_next(&mut self, notch_expanded: bool) -> Option<NotificationItem> {
        if self.current.is_some() || notch_expanded {
            return None;
        }

        let now = self.clock.utc_now();
        self.pending.retain(|p| !(span_between(now, p.time) > Self::MAX_AGE));
        if self.pending.is_empty() {
            return None;
        }

        let next = self.pending.remove(0);
        self.current = Some(next.clone());
        Some(next)
    }

    /// Gösterilen kart bitti (süre doldu, × ile kapandı ya da başka gösterge yerini aldı).
    pub fn complete(&mut self) {
        self.current = None;
    }

    /// Bekleyenleri ve gösterileni bırakır (dinleyici durduğunda).
    pub fn clear(&mut self) {
        self.pending.clear();
        self.current = None;
    }

    fn is_same(a: Option<&NotificationItem>, b: &NotificationItem) -> bool {
        a.is_some_and(|a| a.id == b.id && a.time == b.time)
    }

    fn is_own(&self, item: &NotificationItem) -> bool {
        text::eq_ignore_case(text::trim(&item.app_name), &self.own_app_name)
    }

    fn is_empty(item: &NotificationItem) -> bool {
        text::is_null_or_white_space(Some(&item.title)) && text::is_null_or_white_space(Some(&item.body))
    }
}

use std::sync::Arc;

use super::tray_notify_data::{TrayIconFields, TrayIconKey, TrayMessage, TrayNotifyData};
use crate::text;
use crate::time::{DateTimeOffset, SystemTimeProvider, TimeProvider, TimeSpan, span_between};

struct RecentEntry {
    title: String,
    body: String,
    seen_at: DateTimeOffset,
}

/// Tepsi balon bildirimlerini süzer: boş metinleri (balon kapatma çağrıları dahil) ve kısa aralıkla
/// yinelenen aynı balonları eler. Saf mantık, test edilir.
pub struct TrayBalloonFilter {
    clock: Arc<dyn TimeProvider>,
    // C# `Dictionary`: ekleme sırası korunur (PORTING.md #13); anahtar varsa yerinde güncellenir.
    recent: Vec<(TrayIconKey, RecentEntry)>,
}

impl TrayBalloonFilter {
    pub const DUPLICATE_COOLDOWN: TimeSpan = TimeSpan::from_secs(5);
    pub const MAX_TRACKED_KEYS: usize = 64;

    /// `clock` verilmezse `TimeProvider.System`.
    pub fn new(clock: Option<Arc<dyn TimeProvider>>) -> Self {
        TrayBalloonFilter { clock: clock.unwrap_or_else(|| Arc::new(SystemTimeProvider)), recent: Vec::new() }
    }

    /// Mesaj gösterilebilir bir balon bildirimi taşıyorsa `Some((başlık, gövde, zaman damgası))` döner.
    pub fn try_accept(&mut self, data: &TrayNotifyData) -> Option<(String, String, DateTimeOffset)> {
        if !matches!(data.message, TrayMessage::Add | TrayMessage::Modify) {
            return None;
        }

        if !data.fields.has(TrayIconFields::INFO) {
            return None;
        }

        let title = text::trim(&data.info_title).to_owned();
        let body = text::trim(&data.info).to_owned();
        if title.is_empty() && body.is_empty() {
            return None;
        }

        let now = self.clock.utc_now();
        let key = TrayIconKey::from_data(data);

        if let Some((_, prev)) = self.recent.iter().find(|(k, _)| *k == key)
            && span_between(now, prev.seen_at) < Self::DUPLICATE_COOLDOWN
            && prev.title == title
            && prev.body == body
        {
            return None;
        }

        self.prune_expired(now);
        if !self.recent.iter().any(|(k, _)| *k == key) && self.recent.len() >= Self::MAX_TRACKED_KEYS {
            self.remove_oldest();
        }

        let entry = RecentEntry { title: title.clone(), body: body.clone(), seen_at: now };
        match self.recent.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = entry,
            None => self.recent.push((key, entry)),
        }
        Some((title, body, now))
    }

    pub fn clear(&mut self) {
        self.recent.clear();
    }

    fn prune_expired(&mut self, now: DateTimeOffset) {
        self.recent.retain(|(_, entry)| span_between(now, entry.seen_at) < Self::DUPLICATE_COOLDOWN);
    }

    fn remove_oldest(&mut self) {
        let mut oldest: Option<(usize, DateTimeOffset)> = None;

        for (i, (_, entry)) in self.recent.iter().enumerate() {
            if oldest.is_none_or(|(_, t)| entry.seen_at < t) {
                oldest = Some((i, entry.seen_at));
            }
        }

        if let Some((i, _)) = oldest {
            self.recent.remove(i);
        }
    }
}

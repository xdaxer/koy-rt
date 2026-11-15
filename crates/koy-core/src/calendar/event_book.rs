use chrono::{NaiveDate, NaiveTime};

use super::calendar_event::{CalendarEvent, CalendarEventSource};
use super::calendar_oauth::random_bytes;
use super::event_agenda::EventAgenda;
use crate::event::Event;
use crate::text;

/// Etkinlik defteri: yerel ekleme/silme ve harici (salt okunur) takvim etkinliklerini zamana göre birleştirir.
/// Saf bellek içi mantık; diske yazmayı `EventStore` yapar.
pub struct EventBook {
    local_events: Vec<CalendarEvent>,
    external_events: Vec<CalendarEvent>,
    merged: Vec<CalendarEvent>,
    changed: Event<()>,
    local_changed: Event<()>,
}

impl Default for EventBook {
    fn default() -> Self {
        Self::new(None)
    }
}

impl EventBook {
    /// Yerel defterdeki en çok etkinlik; sınırsız büyüyüp ayar dosyasını şişirmesin.
    pub const MAX_EVENTS: usize = 500;

    /// Her harici sağlayıcı için bellekte tutulacak en çok etkinlik.
    pub const MAX_EXTERNAL_PER_PROVIDER: usize = 250;

    pub fn new(initial: Option<Vec<CalendarEvent>>) -> Self {
        let local_events: Vec<CalendarEvent> = match initial {
            None => Vec::new(),
            Some(initial) => {
                let mut list: Vec<CalendarEvent> =
                    initial.into_iter().filter(|e| e.source == CalendarEventSource::Local).take(Self::MAX_EVENTS).collect();
                // LINQ OrderBy kararlıdır.
                list.sort_by_key(|e| e.start);
                list
            }
        };
        let merged = local_events.clone();
        EventBook { local_events, external_events: Vec::new(), merged, changed: Event::new(), local_changed: Event::new() }
    }

    /// Yerel ya da harici etkinlik listesi değişti.
    pub fn changed(&self) -> &Event<()> {
        &self.changed
    }

    /// Yerel etkinlikler değişti (yalnız bunlar `EventStore` ile diske yazılır).
    pub fn local_changed(&self) -> &Event<()> {
        &self.local_changed
    }

    /// Birleşik (yerel + harici) etkinlikler, başlangıç saatine göre sıralı.
    pub fn events(&self) -> &[CalendarEvent] {
        &self.merged
    }

    /// Yalnız yerel etkinlikler.
    pub fn local_events(&self) -> &[CalendarEvent] {
        &self.local_events
    }

    /// `Add(title, day, time)`: süre varsayılanı 60 dakika.
    pub fn add(&mut self, title: Option<&str>, day: NaiveDate, time: NaiveTime) -> Option<CalendarEvent> {
        self.add_with_duration(title, day, time, 60)
    }

    /// Yerel etkinlik ekler. Başlık boşsa ya da yerel defter doluysa eklenmez ve None döner. Süre 0 - 24 saate sıkıştırılır.
    pub fn add_with_duration(&mut self, title: Option<&str>, day: NaiveDate, time: NaiveTime, duration_minutes: i32) -> Option<CalendarEvent> {
        let normalized = EventAgenda::normalize_title(title)?;
        if self.local_events.len() >= Self::MAX_EVENTS {
            return None;
        }

        let created = CalendarEvent::with_source(
            new_guid_n(),
            normalized,
            day.and_time(time),
            EventAgenda::clamp_duration(duration_minutes),
            CalendarEventSource::Local,
        );

        let index = self.local_events.iter().position(|e| e.start > created.start);
        self.local_events.insert(index.unwrap_or(self.local_events.len()), created.clone());
        self.rebuild_merged();
        self.local_changed.invoke(&());
        self.changed.invoke(&());
        Some(created)
    }

    /// Yalnız yerel etkinlikleri siler; harici (salt okunur) etkinlikler silinmez.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.local_events.len();
        self.local_events.retain(|e| e.id != id);
        let removed = self.local_events.len() < before;
        if removed {
            self.rebuild_merged();
            self.local_changed.invoke(&());
            self.changed.invoke(&());
        }

        removed
    }

    /// Belirtilen harici kaynağın etkinliklerini yenileriyle değiştirir (salt okunur birleştirme).
    pub fn replace_external(&mut self, source: CalendarEventSource, events: &[CalendarEvent]) {
        if source == CalendarEventSource::Local {
            panic!("Yerel kaynak ReplaceExternal ile değiştirilemez.");
        }

        self.external_events.retain(|e| e.source != source);
        let mut valid: Vec<&CalendarEvent> = events
            .iter()
            .filter(|e| !text::is_null_or_white_space(Some(&e.id)) && !text::is_null_or_white_space(Some(&e.title)))
            .collect();
        valid.sort_by_key(|e| e.start);
        for e in valid.into_iter().take(Self::MAX_EXTERNAL_PER_PROVIDER) {
            self.external_events.push(if e.source == source { e.clone() } else { CalendarEvent { source, ..e.clone() } });
        }

        self.rebuild_merged();
        self.changed.invoke(&());
    }

    /// Harici etkinlikleri bellekten temizler (hesap bağlantısı kesilince). `None` hepsini temizler.
    pub fn clear_external(&mut self, source: Option<CalendarEventSource>) -> bool {
        let before = self.external_events.len();
        match source {
            None => self.external_events.clear(),
            Some(s) => self.external_events.retain(|e| e.source != s),
        }
        let removed = self.external_events.len() < before;

        if removed {
            self.rebuild_merged();
            self.changed.invoke(&());
        }

        removed
    }

    /// Bitişi `cutoff` gününden önce olan eski yerel etkinlikleri siler; silinen sayısını döner.
    pub fn prune_before(&mut self, cutoff: NaiveDate) -> i32 {
        let before = self.local_events.len();
        self.local_events.retain(|e| e.end().date() >= cutoff);
        let removed = (before - self.local_events.len()) as i32;
        if removed > 0 {
            self.rebuild_merged();
            self.local_changed.invoke(&());
            self.changed.invoke(&());
        }

        removed
    }

    fn rebuild_merged(&mut self) {
        if self.external_events.is_empty() {
            self.merged = self.local_events.clone();
            return;
        }

        let mut combined = Vec::with_capacity(self.local_events.len() + self.external_events.len());
        combined.extend(self.local_events.iter().cloned());
        combined.extend(self.external_events.iter().cloned());
        // C# ile aynı değil: `List.Sort` kararsızdır (16'dan çok öğede eşit başlangıçların sırası belirsiz);
        // burada kararlı sıralama kullanılır, yerel etkinlikler eşit başlangıçta önce gelir.
        combined.sort_by(|a, b| a.start.cmp(&b.start));
        self.merged = combined;
    }
}

/// `Guid.NewGuid().ToString("N")`: 32 küçük onaltılık hane (sürüm 4 / varyant 1 bitleriyle).
pub(crate) fn new_guid_n() -> String {
    let mut bytes = random_bytes(16);
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

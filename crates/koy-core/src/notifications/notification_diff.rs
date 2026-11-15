use std::collections::HashMap;

use super::notification_item::NotificationItem;
use crate::time::DateTimeOffset;

/// Listedeki bir bildirimin kimliği ve oluşturulma zamanı. Aynı kimlik güncellenirse zaman ilerler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NotificationStamp {
    pub id: u32,
    pub time: DateTimeOffset,
}

impl NotificationStamp {
    pub const fn new(id: u32, time: DateTimeOffset) -> Self {
        NotificationStamp { id, time }
    }
}

/// Bir yoklamanın sonucu: şu an listede olan bütün bildirimlerin damgası ve (en az) yeni ya da güncellenmiş olanların ayrıntısı.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationSnapshot {
    pub stamps: Vec<NotificationStamp>,
    pub items: Vec<NotificationItem>,
}

impl NotificationSnapshot {
    pub fn new(stamps: Vec<NotificationStamp>, items: Vec<NotificationItem>) -> Self {
        NotificationSnapshot { stamps, items }
    }
}

/// Yoklama sonuçlarından yeni gelen bildirimleri bulur. Yeni = bilinmeyen kimlik, ya da bilinen kimlik ama
/// `CreationTime` ilerlemiş (aynı etiket/grupla eski bildirimini değiştiren uygulamalar kimliği korur).
/// İlk okuma "temel çizgi" sayılır: açılışta aksiyon merkezinde duran eski bildirimler kart olarak yağmasın.
/// Saf mantık; tek iş parçacığında kullanılır.
#[derive(Debug, Default)]
pub struct NotificationDiff {
    known: HashMap<u32, DateTimeOffset>,
    has_baseline: bool,
}

impl NotificationDiff {
    pub fn new() -> Self {
        Self::default()
    }

    /// İlk okuma yapıldı mı. Yapılmadıysa kaynak ayrıntıları okumak zorunda değildir.
    pub fn has_baseline(&self) -> bool {
        self.has_baseline
    }

    /// Daha önce görülen kimlikler ve zamanları; kaynak, zamanı ilerlememiş olanlar için ayrıntı üretmeyebilir.
    pub fn known(&self) -> &HashMap<u32, DateTimeOffset> {
        &self.known
    }

    /// Yeni gelenleri zaman sırasıyla döner. Bilinen küme her seferinde güncel listeyle değişir (kapanan bildirimlerin
    /// kimlikleri düşer, küme sınırsız büyümez).
    pub fn apply(&mut self, snapshot: &NotificationSnapshot) -> Vec<NotificationItem> {
        let mut current: HashMap<u32, DateTimeOffset> = HashMap::new();
        for stamp in &snapshot.stamps {
            current.insert(stamp.id, stamp.time);
        }

        if !self.has_baseline {
            self.has_baseline = true;
            self.known = current;
            return Vec::new();
        }

        // Where → DistinctBy(Id) (ilk görülen kalır) → OrderBy(Time).ThenBy(Id); LINQ sıralaması kararlı.
        // Not: karşılaştırılan zaman damgadaki (`current`) zamandır, öğenin kendi `Time`'ı değil.
        let mut arrived: Vec<NotificationItem> = Vec::new();
        for item in &snapshot.items {
            let Some(&time) = current.get(&item.id) else { continue };
            let is_new = match self.known.get(&item.id) {
                None => true,
                Some(&known_time) => time > known_time,
            };
            if !is_new || arrived.iter().any(|a| a.id == item.id) {
                continue;
            }
            arrived.push(item.clone());
        }
        arrived.sort_by(|a, b| a.time.cmp(&b.time).then(a.id.cmp(&b.id)));

        self.known = current;
        arrived
    }
}

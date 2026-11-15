//! Karşılığı: `JumpList/JumpListCache.cs`.

use std::collections::VecDeque;
use std::sync::Mutex;

use super::jump_list_models::JumpListSection;
use crate::text;

/// Bir dosyanın boyutu ve son yazılma zamanı; dosya yoksa `missing()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileStamp {
    pub length: i64,
    pub last_write_utc_ticks: i64,
}

impl FileStamp {
    pub fn new(length: i64, last_write_utc_ticks: i64) -> Self {
        FileStamp { length, last_write_utc_ticks }
    }

    pub fn missing() -> Self {
        FileStamp { length: -1, last_write_utc_ticks: 0 }
    }
}

struct Entry {
    app_id: String,
    custom: FileStamp,
    automatic: FileStamp,
    sections: Vec<JumpListSection>,
}

/// Uygulama başına son okunan bölümleri tutan sınırlı (en az yakın zamanda kullanılan atılır) önbellek. Girdi, iki
/// kaynak dosyanın (özel hedefler, otomatik hedefler) damgası değiştiyse geçersizdir. Zamanlayıcısı yoktur; eski girdiler
/// yalnız yeni bir ekleme sırasında atılır. İş parçacığı güvenlidir.
pub struct JumpListCache {
    // Ön (front) = en yakın zamanda kullanılan (C#'ta LinkedList.First).
    lru: Mutex<VecDeque<Entry>>,
    capacity: i32,
}

impl JumpListCache {
    pub const DEFAULT_CAPACITY: i32 = 16;

    pub fn new(capacity: i32) -> Self {
        JumpListCache { lru: Mutex::new(VecDeque::new()), capacity: capacity.max(1) }
    }

    pub fn count(&self) -> usize {
        self.lru.lock().unwrap().len()
    }

    /// İsabette bölümlerin kopyası; ıskada `None`. Bayat girdi (damga değişmiş) silinir.
    pub fn try_get(&self, app_id: &str, custom: FileStamp, automatic: FileStamp) -> Option<Vec<JumpListSection>> {
        let mut lru = self.lru.lock().unwrap();
        let found = lru.iter().position(|e| text::eq_ignore_case(&e.app_id, app_id))?;
        let entry = lru.remove(found).unwrap();
        if entry.custom == custom && entry.automatic == automatic {
            let sections = entry.sections.clone();
            lru.push_front(entry);
            return Some(sections);
        }

        // bayat: girdi atılır
        None
    }

    pub fn set(&self, app_id: &str, custom: FileStamp, automatic: FileStamp, sections: Vec<JumpListSection>) {
        let mut lru = self.lru.lock().unwrap();
        if let Some(found) = lru.iter().position(|e| text::eq_ignore_case(&e.app_id, app_id)) {
            lru.remove(found);
        }

        lru.push_front(Entry { app_id: app_id.to_string(), custom, automatic, sections });
        while lru.len() as i32 > self.capacity {
            lru.pop_back();
        }
    }

    pub fn clear(&self) {
        self.lru.lock().unwrap().clear();
    }
}

impl Default for JumpListCache {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

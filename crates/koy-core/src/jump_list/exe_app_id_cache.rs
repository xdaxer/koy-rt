//! Karşılığı: `JumpList/ExeAppIdCache.cs`.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use crate::text;

struct Inner {
    // Anahtar büyük harfe çevrilir: C#'taki `StringComparer.OrdinalIgnoreCase` sözlüğünün karşılığı.
    map: HashMap<String, Option<String>>,
    order: VecDeque<String>,
}

/// Exe yolundan çözülen AppUserModelID'ler için küçük, sınırlı önbellek. Bulunamadı sonucu (`None`) da tutulur; böylece
/// aynı uygulamaya her sağ tıkta Başlat menüsü yeniden taranmaz. Kapasite dolunca en eski girdi atılır. İş parçacığı güvenli.
pub struct ExeAppIdCache {
    gate: Mutex<Inner>,
    capacity: i32,
}

impl ExeAppIdCache {
    pub const DEFAULT_CAPACITY: i32 = 64;

    pub fn new(capacity: i32) -> Self {
        ExeAppIdCache {
            gate: Mutex::new(Inner { map: HashMap::new(), order: VecDeque::new() }),
            capacity: capacity.max(1),
        }
    }

    pub fn count(&self) -> usize {
        self.gate.lock().unwrap().map.len()
    }

    /// Girdi varsa `Some(sonuç)` (negatif sonuçta `Some(None)`), yoksa `None`.
    pub fn try_get(&self, exe_path: &str) -> Option<Option<String>> {
        self.gate.lock().unwrap().map.get(&text::to_upper_invariant(exe_path)).cloned()
    }

    pub fn set(&self, exe_path: &str, app_id: Option<String>) {
        let key = text::to_upper_invariant(exe_path);
        let mut g = self.gate.lock().unwrap();
        if !g.map.contains_key(&key) {
            g.order.push_back(key.clone());
            while g.order.len() as i32 > self.capacity {
                if let Some(oldest) = g.order.pop_front() {
                    g.map.remove(&oldest);
                }
            }
        }

        g.map.insert(key, app_id);
    }

    pub fn clear(&self) {
        let mut g = self.gate.lock().unwrap();
        g.map.clear();
        g.order.clear();
    }
}

impl Default for ExeAppIdCache {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

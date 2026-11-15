use std::sync::Arc;

use crate::text;
use crate::time::{TimeProvider, TimeSpan};

/// Dock simgesinin üzerine sürükleme eylemi: Sol %25 = Before, Orta ~%50 = Merge, Sağ %25 = After.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DockDropZone {
    #[default]
    Before,
    Merge,
    After,
}

/// Geriye dönük uyumluluk veya alternatif adlandırma için eylem takma adı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DockDropAction {
    #[default]
    Before,
    Merge,
    After,
}

/// Dock öğeleri üzerinde sürükleme bölgelerini hesaplayan saf geometri motoru.
pub struct DockDropZones;

impl DockDropZones {
    /// Birleştirme (Merge) eyleminin silahlanması (armed) için gereken kesintisiz bekleme süresi: 400 ms.
    pub const MERGE_HOVER_DELAY: TimeSpan = TimeSpan::from_millis(400);

    pub fn merge_hover_delay() -> TimeSpan {
        Self::MERGE_HOVER_DELAY
    }

    /// Verilen genişlik ve bağıl X konumuna göre sürükleme bölgesini tespit eder.
    /// `merge_ratio` C# varsayılanı 0.5 (%50 orta bölge, %25 sol, %25 sağ).
    pub fn detect_zone(item_width: f64, relative_x: f64, merge_ratio: f64) -> DockDropZone {
        if item_width <= 0.0 {
            return DockDropZone::Merge;
        }

        if merge_ratio <= 0.0 {
            return if relative_x < item_width * 0.5 { DockDropZone::Before } else { DockDropZone::After };
        }

        if merge_ratio >= 1.0 {
            return DockDropZone::Merge;
        }

        let side_ratio = (1.0 - merge_ratio) / 2.0;
        let left_threshold = item_width * side_ratio;
        let right_threshold = item_width * (1.0 - side_ratio);

        if relative_x < left_threshold {
            return DockDropZone::Before;
        }

        if relative_x > right_threshold {
            return DockDropZone::After;
        }

        DockDropZone::Merge
    }

    /// İmlecin mutlak X koordinatı, yuvanın sol kenarı ve genişliğine göre bölgeyi hesaplar.
    pub fn resolve_zone(cursor_x: f64, slot_left: f64, slot_width: f64, merge_ratio: f64) -> DockDropZone {
        Self::detect_zone(slot_width, cursor_x - slot_left, merge_ratio)
    }
}

/// Bölge tespiti için ek statik sarmalayıcı sınıf.
pub struct DockDropZoneDetector;

impl DockDropZoneDetector {
    pub fn detect_zone(item_width: f64, relative_x: f64, merge_ratio: f64) -> DockDropZone {
        DockDropZones::detect_zone(item_width, relative_x, merge_ratio)
    }

    pub fn resolve_zone(cursor_x: f64, slot_left: f64, slot_width: f64, merge_ratio: f64) -> DockDropZone {
        DockDropZones::resolve_zone(cursor_x, slot_left, slot_width, merge_ratio)
    }
}

/// Birleştirme (Merge) bölgesinde 400 ms kesintisiz duraklamayı takip eden saf durum yöneticisi.
/// Platform zamanlayıcılarına bağımlı değildir; dışarıdan geçirilen zaman damgasıyla test edilebilir.
pub struct DockDropZoneTracker {
    target_slot: i32,
    target_key: Option<String>,
    hover_start_time: Option<TimeSpan>,
    time_provider: Option<Arc<dyn TimeProvider>>,
    is_merge_armed: bool,
}

impl Default for DockDropZoneTracker {
    fn default() -> Self {
        Self::new(None)
    }
}

impl DockDropZoneTracker {
    pub const MERGE_HOVER_DELAY: TimeSpan = DockDropZones::MERGE_HOVER_DELAY;

    pub fn merge_hover_delay() -> TimeSpan {
        Self::MERGE_HOVER_DELAY
    }

    pub fn detect_zone(item_width: f64, relative_x: f64, merge_ratio: f64) -> DockDropZone {
        DockDropZones::detect_zone(item_width, relative_x, merge_ratio)
    }

    pub fn resolve_zone(cursor_x: f64, slot_left: f64, slot_width: f64, merge_ratio: f64) -> DockDropZone {
        DockDropZones::resolve_zone(cursor_x, slot_left, slot_width, merge_ratio)
    }

    pub fn new(time_provider: Option<Arc<dyn TimeProvider>>) -> Self {
        DockDropZoneTracker { target_slot: -1, target_key: None, hover_start_time: None, time_provider, is_merge_armed: false }
    }

    pub fn is_merge_armed(&self) -> bool {
        self.is_merge_armed
    }

    pub fn target_slot(&self) -> i32 {
        self.target_slot
    }

    pub fn target_key(&self) -> Option<&str> {
        self.target_key.as_deref()
    }

    pub fn hover_start_time(&self) -> Option<TimeSpan> {
        self.hover_start_time
    }

    /// İndeks tabanlı yuva ve zaman damgasıyla durumu günceller.
    /// İmleç 400 ms boyunca Merge bölgesinde kalırsa IsMergeArmed true döner.
    pub fn update(&mut self, target_slot: i32, zone: DockDropZone, now: TimeSpan) -> bool {
        if zone != DockDropZone::Merge || target_slot < 0 {
            self.reset();
            return false;
        }

        if self.target_slot != target_slot {
            self.target_slot = target_slot;
            self.target_key = None;
            self.hover_start_time = Some(now);
            self.is_merge_armed = false;
            return false;
        }

        let start = *self.hover_start_time.get_or_insert(now);
        if now - start >= DockDropZones::MERGE_HOVER_DELAY {
            self.is_merge_armed = true;
        }

        self.is_merge_armed
    }

    /// Anahtar/kimlik tabanlı yuva ve zaman damgasıyla durumu günceller (C# `Update(string, zone, now)`).
    pub fn update_key(&mut self, target_key: &str, zone: DockDropZone, now: TimeSpan) -> bool {
        if zone != DockDropZone::Merge || target_key.is_empty() {
            self.reset();
            return false;
        }

        let same = self.target_key.as_deref().is_some_and(|k| text::eq_ignore_case(k, target_key));
        if !same {
            self.target_key = Some(target_key.to_string());
            self.target_slot = -1;
            self.hover_start_time = Some(now);
            self.is_merge_armed = false;
            return false;
        }

        let start = *self.hover_start_time.get_or_insert(now);
        if now - start >= DockDropZones::MERGE_HOVER_DELAY {
            self.is_merge_armed = true;
        }

        self.is_merge_armed
    }

    /// TimeProvider ile zaman damgasını hesaplayarak durumu günceller (C# `Update(int, zone, TimeProvider)`).
    pub fn update_with_time_provider(&mut self, target_slot: i32, zone: DockDropZone, time_provider: &dyn TimeProvider) -> bool {
        self.update(target_slot, zone, TimeSpan::from_ticks(time_provider.timestamp()))
    }

    /// Yapıcıda TimeProvider verilmişse parametresiz zaman güncellemesi yapar (C# `Update(int, zone)`).
    /// Verilmemişse C#'taki `InvalidOperationException` gibi panikler.
    pub fn update_with_ctor_time_provider(&mut self, target_slot: i32, zone: DockDropZone) -> bool {
        let provider = match &self.time_provider {
            Some(p) => Arc::clone(p),
            None => panic!("TimeProvider yapıcıda sağlanmadı. Update(targetSlot, zone, TimeSpan now) kullanın."),
        };
        self.update(target_slot, zone, TimeSpan::from_ticks(provider.timestamp()))
    }

    /// Durumu sıfırlar. İmleç Merge dışına çıktığında veya sürükleme bittiğinde çağrılır.
    pub fn reset(&mut self) {
        self.target_slot = -1;
        self.target_key = None;
        self.hover_start_time = None;
        self.is_merge_armed = false;
    }
}

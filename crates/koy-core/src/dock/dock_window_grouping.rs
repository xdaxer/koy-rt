use super::dock_enums::{DockGroupingMode, DockWindowMonitorScope};
use crate::text;

// Pencere gruplama ve monitör süzgeci saf karar mantığıdır: Windows'tan gelen düz değerlerle çalışır, WinRT / Win32 tipi yoktur.
// Pencere sayımı (EnumWindows, monitör ve sanal masaüstü bilgisi) Platform tarafının işidir ve bu dosyanın dışındadır.

/// Bir pencerenin dock kararı için gereken gerçekler.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DockWindowFact {
    pub process_name: String,
    pub monitor_id: i32,
    pub handle: isize,
}

impl DockWindowFact {
    pub fn new(process_name: impl Into<String>, monitor_id: i32, handle: isize) -> Self {
        DockWindowFact { process_name: process_name.into(), monitor_id, handle }
    }
}

/// Tek dock simgesine karşılık gelen pencereler. Tutamaçlar girdi sırasındadır.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DockWindowGroup {
    pub process_name: String,
    pub handles: Vec<isize>,
}

pub struct DockWindowGrouping;

impl DockWindowGrouping {
    /// Monitör süzgeci. `AllMonitors` listeyi olduğu gibi döndürür.
    pub fn filter_by_monitor(windows: &[DockWindowFact], scope: DockWindowMonitorScope, current_monitor_id: i32) -> Vec<DockWindowFact> {
        if scope == DockWindowMonitorScope::AllMonitors {
            return windows.to_vec();
        }

        // Girdi sırası korunur: aynı girdi her zaman aynı çıktıyı verir.
        windows.iter().filter(|w| w.monitor_id == current_monitor_id).cloned().collect()
    }

    /// Pencereleri dock simgelerine böler. Sıra kararlıdır: gruplar ilk göründükleri sırayla, bir gruptaki tutamaçlar
    /// girdi sırasıyla gelir. Süreç adı büyük/küçük harf duyarsız eşlenir.
    ///
    /// `available_slots`: Dock'un bu pencereler için ayırabildiği simge sayısı (yalnız WhenSpaceRunsOut kullanır).
    /// Negatifse C#'taki `ArgumentOutOfRangeException` gibi panikler.
    pub fn group(windows: &[DockWindowFact], mode: DockGroupingMode, available_slots: i32) -> Vec<DockWindowGroup> {
        assert!(available_slots >= 0, "availableSlots ('{available_slots}') must be a non-negative value.");

        // Yer varken gruplamak pencereleri gereksiz yere saklar; WhenSpaceRunsOut bu yüzden yalnız sığmayınca birleştirir.
        // Dosyadan gelen tanımsız sayı (örn. 7) bugünkü davranışa, yani gruplamasıza düşer (Rust'ta enum tanımsız olamaz).
        let group = match mode {
            DockGroupingMode::Always => true,
            DockGroupingMode::WhenSpaceRunsOut => windows.len() as i64 > available_slots as i64,
            DockGroupingMode::Never => false,
        };

        let mut groups = Vec::new();
        if !group {
            for w in windows {
                groups.push(DockWindowGroup { process_name: w.process_name.clone(), handles: vec![w.handle] });
            }
            return groups;
        }

        // Büyük/küçük harf duyarsız sözlük; anahtarın yazımı ilk görülenindir, ekleme sırası korunur.
        for w in windows {
            match groups.iter_mut().find(|g: &&mut DockWindowGroup| text::eq_ignore_case(&g.process_name, &w.process_name)) {
                Some(g) => g.handles.push(w.handle),
                None => groups.push(DockWindowGroup { process_name: w.process_name.clone(), handles: vec![w.handle] }),
            }
        }

        groups
    }
}

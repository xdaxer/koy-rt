//! Yakalama iş parçacığından gelen çubuk düzeylerini arayüz iş parçacığına taşır. Kareler birikmez: arayüz bir
//! öncekini çizmeden gelen yenisi yalnız son değeri günceller (kuyrukta en çok bir çağrı).
//! Karşılığı: `Features/Music/SpectrumBars.cs` (`SpectrumFeed`).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;

struct Pending {
    levels: Vec<f32>,
    queued: bool,
}

pub struct SpectrumFeed {
    pending: Arc<Mutex<Pending>>,
    levels: RefCell<Vec<f32>>,
    /// Yeni düzeyler arayüze geçti (arayüz iş parçacığında).
    pub updated: Notifier,
}

/// Herhangi bir iş parçacığından düzey itmek için tutamaç.
#[derive(Clone)]
pub struct SpectrumPusher {
    pending: Arc<Mutex<Pending>>,
    feed: UiRef<SpectrumFeed>,
}

impl SpectrumPusher {
    pub fn push(&self, levels: &[f32]) {
        {
            let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            let n = levels.len().min(p.levels.len());
            p.levels[..n].copy_from_slice(&levels[..n]);
            if p.queued {
                return;
            }
            p.queued = true;
        }
        let feed = self.feed;
        dispatch::post(move || {
            if let Some(f) = feed.upgrade() {
                f.apply();
            }
        });
    }

    /// Çubukları sıfırlar (yakalama durdu).
    pub fn clear(&self) {
        let n = self.pending.lock().unwrap_or_else(|e| e.into_inner()).levels.len();
        self.push(&vec![0.0; n]);
    }
}

impl SpectrumFeed {
    pub fn new(band_count: usize) -> Rc<Self> {
        Rc::new(SpectrumFeed {
            pending: Arc::new(Mutex::new(Pending { levels: vec![0.0; band_count], queued: false })),
            levels: RefCell::new(vec![0.0; band_count]),
            updated: Notifier::default(),
        })
    }

    pub fn pusher(self: &Rc<Self>) -> SpectrumPusher {
        SpectrumPusher { pending: Arc::clone(&self.pending), feed: UiRef::new(self) }
    }

    /// Arayüz iş parçacığında okunan son düzeyler (0..1).
    pub fn levels(&self) -> Vec<f32> {
        self.levels.borrow().clone()
    }

    fn apply(&self) {
        {
            let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            self.levels.borrow_mut().copy_from_slice(&p.levels);
            p.queued = false;
        }
        self.updated.notify();
    }
}

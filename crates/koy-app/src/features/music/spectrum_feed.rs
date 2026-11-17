//! Yakalama iş parçacığından gelen çubuk düzeylerini arayüz iş parçacığına taşır. Kareler birikmez: arayüz bir
//! öncekini çizmeden gelen yenisi yalnız son değeri günceller (kuyrukta en çok bir çağrı).
//! Karşılığı: `Features/Music/SpectrumBars.cs` (`SpectrumFeed`).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;

struct Pending {
    levels: Vec<f32>,
    queued: bool,
    last_dispatch: Option<Instant>,
}

pub struct SpectrumFeed {
    pending: Arc<Mutex<Pending>>,
    min_interval_ms: Arc<AtomicI64>,
    levels: RefCell<Vec<f32>>,
    /// Yeni düzeyler arayüze geçti (arayüz iş parçacığında).
    pub updated: Notifier,
}

/// Herhangi bir iş parçacığından düzey itmek için tutamaç.
#[derive(Clone)]
pub struct SpectrumPusher {
    pending: Arc<Mutex<Pending>>,
    min_interval_ms: Arc<AtomicI64>,
    feed: UiRef<SpectrumFeed>,
}

impl SpectrumPusher {
    pub fn push(&self, levels: &[f32]) {
        self.push_inner(levels, false);
    }

    fn push_inner(&self, levels: &[f32], force: bool) {
        {
            let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            let n = levels.len().min(p.levels.len());
            p.levels[..n].copy_from_slice(&levels[..n]);
            if p.queued {
                return;
            }
            let min_interval = self.min_interval_ms.load(Ordering::Relaxed);
            // Sessizlik karesi hiçbir zaman düşürülmez ki yakalama durduğunda çubuklar havada donmasın.
            if !force
                && min_interval > 0
                && !is_silent(levels)
                && p.last_dispatch.is_some_and(|t| (t.elapsed().as_millis() as i64) < min_interval)
            {
                return;
            }
            p.last_dispatch = Some(Instant::now());
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
        self.push_inner(&vec![0.0; n], true);
    }
}

impl SpectrumFeed {
    pub fn new(band_count: usize) -> Rc<Self> {
        Rc::new(SpectrumFeed {
            pending: Arc::new(Mutex::new(Pending { levels: vec![0.0; band_count], queued: false, last_dispatch: None })),
            min_interval_ms: Arc::new(AtomicI64::new(0)),
            levels: RefCell::new(vec![0.0; band_count]),
            updated: Notifier::default(),
        })
    }

    pub fn pusher(self: &Rc<Self>) -> SpectrumPusher {
        SpectrumPusher { pending: Arc::clone(&self.pending), min_interval_ms: Arc::clone(&self.min_interval_ms), feed: UiRef::new(self) }
    }

    /// En küçük arayüz yenileme aralığı (ms). Kapalı çentikte ~15 kare/sn için 60 ms gereksiz yazılımsal çizimi keser;
    /// açık kartta 0 (yakalama hızında).
    pub fn set_min_interval_ms(&self, interval_ms: i64) {
        self.min_interval_ms.store(interval_ms.max(0), Ordering::Relaxed);
    }

    /// `bar_count` çubuk için düzeyler: küçük çubuk sayısında yalnız en düşük bas bantları yerine bütün tayf örneklenir
    /// (her dilimin tepesi). C# `SpectrumBars.SampleLevel`.
    pub fn sampled(&self, bar_count: usize) -> Vec<f32> {
        let raw = self.levels.borrow();
        if bar_count == 0 || bar_count >= raw.len() {
            return raw.clone();
        }
        (0..bar_count)
            .map(|i| {
                let start = i * raw.len() / bar_count;
                let end = ((i + 1) * raw.len() / bar_count).max(start + 1).min(raw.len());
                raw[start..end].iter().copied().fold(0.0f32, f32::max)
            })
            .collect()
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

fn is_silent(levels: &[f32]) -> bool {
    levels.iter().all(|l| *l <= 0.01)
}

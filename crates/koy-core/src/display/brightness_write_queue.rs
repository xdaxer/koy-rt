use std::sync::Mutex;

use crate::text;

#[derive(Default)]
struct Inner {
    pending: Option<i32>,
    running: bool,
}

/// Parlaklık yazımlarını birleştirir: kaydırıcı saniyede onlarca değer üretir, ama DDC/CI çağrısı yüzlerce ms
/// sürebilir. Bekleyen değer tek yer tutar (yenisi eskisini ezer); yalnız bir çalışan işçi olur.
/// Çağıran [`BrightnessWriteQueue::post`] true dönerse işçiyi başlatır, işçi [`BrightnessWriteQueue::try_take`]
/// `None` dönene kadar döner.
#[derive(Default)]
pub struct BrightnessWriteQueue {
    gate: Mutex<Inner>,
}

impl BrightnessWriteQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Değeri bekleyen yere yazar (0-100'e kıstırılır). İşçi çalışmıyorsa true: çağıran başlatmalı.
    pub fn post(&self, value: i32) -> bool {
        let mut g = self.gate.lock().unwrap();
        g.pending = Some(text::clamp(value, 0, 100));
        if g.running {
            return false;
        }

        g.running = true;
        true
    }

    /// Bekleyen son değeri alır. Yoksa işçiyi "durdu" işaretler ve `None` döner
    /// (C#: `TryTake(out value)` false).
    pub fn try_take(&self) -> Option<i32> {
        let mut g = self.gate.lock().unwrap();
        if let Some(pending) = g.pending.take() {
            return Some(pending);
        }

        g.running = false;
        None
    }
}

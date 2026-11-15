//! C# `CancellationToken` karşılığı.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// C# `CancellationToken` + `CancellationTokenSource` yerine geçen en küçük karşılık: paylaşılan iptal bayrağı.
/// Klonlar aynı bayrağı gösterir; `cancel` hepsini iptal eder.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancellation_requested(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

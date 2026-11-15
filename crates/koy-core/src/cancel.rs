//! `System.Threading.CancellationToken` / `CancellationTokenSource` / `OperationCanceledException` karşılıkları.

use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// `OperationCanceledException`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OperationCanceled;

impl fmt::Display for OperationCanceled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("The operation was canceled.")
    }
}

impl Error for OperationCanceled {}

/// `CancellationToken`: klonlar aynı bayrağı gösterir. `default` (`none()`) hiç iptal edilmez.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// `default(CancellationToken)`.
    pub fn none() -> Self {
        Self::default()
    }

    /// Kaynağı ayrı tutmayan çağıranlar için kısayol (C#'ta `CancellationTokenSource.Cancel`).
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancellation_requested(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// `ThrowIfCancellationRequested`: iptal edildiyse `OperationCanceled` hatası.
    pub fn throw_if_cancellation_requested(&self) -> Result<(), OperationCanceled> {
        if self.is_cancellation_requested() { Err(OperationCanceled) } else { Ok(()) }
    }
}

/// `CancellationTokenSource`.
#[derive(Debug, Clone, Default)]
pub struct CancellationTokenSource {
    token: CancellationToken,
}

impl CancellationTokenSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }

    pub fn is_cancellation_requested(&self) -> bool {
        self.token.is_cancellation_requested()
    }
}

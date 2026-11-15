//! Lyrics'in eşzamansızlık modeli ortak iptal tiplerine dayanır (`crate::cancel`).

pub use crate::cancel::{CancellationToken, CancellationTokenSource, OperationCanceled};

/// Kaynakların döndürdüğü hata: C#'taki herhangi bir `Exception`. İptal, [`OperationCanceled`] değeridir.
pub type LyricsError = Box<dyn std::error::Error + Send + Sync>;

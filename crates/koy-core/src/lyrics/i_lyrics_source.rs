//! Karşılığı: `Lyrics/ILyricsSource.cs`.

use super::cancellation::{CancellationToken, LyricsError};
use super::lyrics_query::LyricsQuery;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LyricsLookupKind {
    Found,

    /// Kaynak yanıt verdi, bu parça için eşzamanlı söz yok.
    NotFound,

    /// Geçici sorun (ağ, 429, sunucu): sonuç önbelleğe yazılmaz, sonra yeniden denenir.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsLookup {
    pub kind: LyricsLookupKind,
    pub lrc: Option<String>,
}

impl LyricsLookup {
    /// `LyricsLookup.NotFound`.
    pub fn not_found() -> Self {
        LyricsLookup { kind: LyricsLookupKind::NotFound, lrc: None }
    }

    /// `LyricsLookup.Failed`.
    pub fn failed() -> Self {
        LyricsLookup { kind: LyricsLookupKind::Failed, lrc: None }
    }

    /// `LyricsLookup.Found(lrc)`.
    pub fn found(lrc: impl Into<String>) -> Self {
        LyricsLookup { kind: LyricsLookupKind::Found, lrc: Some(lrc.into()) }
    }
}

/// Bir söz kaynağı (yerel klasör, çevrim içi hizmet). Ham LRC metni döndürür.
///
/// C# `Task<LyricsLookup> FindAsync(...)` burada bloklayıcı; çağıran (`LyricsSession`) arka plan iş parçacığında çağırır.
/// Hata dönerse C#'taki istisna gibidir (iptal için `OperationCanceled`).
pub trait ILyricsSource: Send + Sync {
    fn find(&self, query: &LyricsQuery, cancellation_token: &CancellationToken) -> Result<LyricsLookup, LyricsError>;
}

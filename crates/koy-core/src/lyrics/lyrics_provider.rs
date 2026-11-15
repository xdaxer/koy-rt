//! Karşılığı: `Lyrics/LyricsProvider.cs`.

use std::sync::Arc;

use super::cancellation::{CancellationToken, LyricsError};
use super::i_lyrics_source::{ILyricsSource, LyricsLookupKind};
use super::lrc_parser::LrcParser;
use super::lyric_line::LyricLine;
use super::lyrics_cache::{LyricsCache, LyricsCacheStatus};
use super::lyrics_query::LyricsQuery;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LyricsLoadStatus {
    Found,
    NotFound,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsLoadResult {
    pub status: LyricsLoadStatus,
    pub lines: Vec<LyricLine>,
}

impl LyricsLoadResult {
    pub fn new(status: LyricsLoadStatus, lines: Vec<LyricLine>) -> Self {
        LyricsLoadResult { status, lines }
    }

    /// `LyricsLoadResult.NotFound`.
    pub fn not_found() -> Self {
        LyricsLoadResult { status: LyricsLoadStatus::NotFound, lines: Vec::new() }
    }

    /// `LyricsLoadResult.Failed`.
    pub fn failed() -> Self {
        LyricsLoadResult { status: LyricsLoadStatus::Failed, lines: Vec::new() }
    }
}

/// C# `Task<LyricsLoadResult> LoadAsync(...)` burada bloklayıcı `load`; `LyricsSession` arka plan iş parçacığında çağırır.
pub trait ILyricsProvider: Send + Sync {
    fn load(&self, query: &LyricsQuery, cancellation_token: &CancellationToken) -> Result<LyricsLoadResult, LyricsError>;
}

/// Kaynak sırası: yerel dosya, disk önbelleği, çevrim içi hizmet. Yerelde ya da önbellekte varsa ağa çıkılmaz.
pub struct LyricsProvider {
    local: Arc<dyn ILyricsSource>,
    cache: LyricsCache,
    online: Arc<dyn ILyricsSource>,
}

impl LyricsProvider {
    pub fn new(local: Arc<dyn ILyricsSource>, cache: LyricsCache, online: Arc<dyn ILyricsSource>) -> Self {
        LyricsProvider { local, cache, online }
    }
}

impl ILyricsProvider for LyricsProvider {
    fn load(&self, query: &LyricsQuery, cancellation_token: &CancellationToken) -> Result<LyricsLoadResult, LyricsError> {
        let local_result = self.local.find(query, cancellation_token)?;
        if local_result.kind == LyricsLookupKind::Found {
            if let Some(local_lrc) = local_result.lrc.as_deref() {
                let local_lines = LrcParser::parse(Some(local_lrc));
                if !local_lines.is_empty() {
                    return Ok(LyricsLoadResult::new(LyricsLoadStatus::Found, local_lines));
                }
            }
        }

        let (status, cached) = self.cache.read(query);
        if status == LyricsCacheStatus::KnownMissing {
            return Ok(LyricsLoadResult::not_found());
        }

        if status == LyricsCacheStatus::Hit {
            let cached_lines = LrcParser::parse(cached.as_deref());
            if !cached_lines.is_empty() {
                return Ok(LyricsLoadResult::new(LyricsLoadStatus::Found, cached_lines));
            }
        }

        let remote = self.online.find(query, cancellation_token)?;
        if remote.kind == LyricsLookupKind::Failed {
            return Ok(LyricsLoadResult::failed());
        }

        if remote.kind == LyricsLookupKind::Found {
            if let Some(lrc) = remote.lrc.as_deref() {
                let lines = LrcParser::parse(Some(lrc));
                if !lines.is_empty() {
                    self.cache.write(query, lrc);
                    return Ok(LyricsLoadResult::new(LyricsLoadStatus::Found, lines));
                }
            }
        }

        self.cache.write_missing(query);
        Ok(LyricsLoadResult::not_found())
    }
}

//! Karşılığı: `koy/src/Core/Lyrics/`.
//!
//! Şarkı sözü: LRC ayrıştırma, LRCLIB adres/yanıt çözümleme, yerel + disk önbelleği + çevrim içi kaynak sırası,
//! parça oturumu ve çalma konumundan etkin satır hesabı.
//!
//! **Eşzamansızlık modeli:** C# `Task` / `CancellationToken` burada eşzamanlı çağrı + `CancellationToken`
//! ([`cancellation`]). `ILyricsSource::find` ve `ILyricsProvider::load` bloklayıcıdır; `LyricsSession::load` yükleme işini
//! kendi iş parçacığına alır ve C#'taki `Task`'ın karşılığı olarak bir [`lyrics_session::LoadTask`] döner (`wait()` = `await`).

pub mod cancellation;
pub mod file_text;
pub mod i_lyrics_source;
pub mod local_lyrics_source;
pub mod lrc_parser;
pub mod lrclib_api;
pub mod lyric_line;
pub mod lyrics_cache;
pub mod lyrics_provider;
pub mod lyrics_query;
pub mod lyrics_session;
pub mod lyrics_timeline;

pub use cancellation::{CancellationToken, CancellationTokenSource, LyricsError, OperationCanceled};
pub use i_lyrics_source::{ILyricsSource, LyricsLookup, LyricsLookupKind};
pub use local_lyrics_source::LocalLyricsSource;
pub use lrc_parser::LrcParser;
pub use lrclib_api::LrclibApi;
pub use lyric_line::LyricLine;
pub use lyrics_cache::{LyricsCache, LyricsCacheStatus};
pub use lyrics_provider::{ILyricsProvider, LyricsLoadResult, LyricsLoadStatus, LyricsProvider};
pub use lyrics_query::LyricsQuery;
pub use lyrics_session::{LoadTask, LyricsSession, LyricsState};
pub use lyrics_timeline::LyricsTimeline;

#[cfg(test)]
mod tests;

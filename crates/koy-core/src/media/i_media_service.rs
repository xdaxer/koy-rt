use crate::event::Event;
use crate::time::{DateTimeOffset, TimeSpan};

/// Medya servisi sözleşmesi. Core'da yer alır, WinRT/Win32 tipleri içermez.
/// Uygulaması Platform projesindedir (GSMTC tabanlı).
///
/// C# farkları: `event EventHandler<T>` alanları trait'te erişimci metot olarak (`changed()` vb.) görünür,
/// `Task` dönen metotlar eşzamanlı metot olur (bekleme/arka plan işi uygulayanın sorumluluğudur) ve
/// `IDisposable` karşılığı `Drop`'tur (trait'e eklenmez).
pub trait IMediaService: Send + Sync {
    /// `Changed` olayı: geçerli medya değişti (`None`: oturum yok).
    fn changed(&self) -> &Event<Option<MediaInfo>>;
    /// `TimelineChanged` olayı.
    fn timeline_changed(&self) -> &Event<Option<PlaybackTimeline>>;
    /// `SessionsChanged` olayı.
    fn sessions_changed(&self) -> &Event<Vec<MediaSessionInfo>>;

    fn current(&self) -> Option<MediaInfo>;
    fn timeline(&self) -> Option<PlaybackTimeline>;
    fn sessions(&self) -> Vec<MediaSessionInfo>;

    fn start(&self);
    fn stop(&self);

    fn play_pause(&self);
    fn next(&self);
    fn previous(&self);
    fn seek(&self, position: TimeSpan);
    fn select_session(&self, session_id: &str);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub source_app_id: String,
    pub is_playing: bool,
    pub thumbnail: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackTimeline {
    pub position: TimeSpan,
    pub start_time: TimeSpan,
    pub end_time: TimeSpan,
    pub last_updated: DateTimeOffset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSessionInfo {
    pub id: String,
    pub source_app_id: String,
    pub title: String,
    pub is_current: bool,
}

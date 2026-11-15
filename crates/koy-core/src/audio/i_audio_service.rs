use crate::event::Event;

/// Sistem ses seviyesi ve sessize alma sözleşmesi.
/// Saf Rust: Platform uygulaması CoreAudio (IAudioEndpointVolume) ile çalışır.
/// C# `IDisposable` karşılığı uygulayan tipin `Drop`'udur.
pub trait IAudioService: Send + Sync {
    fn volume(&self) -> f32;
    fn is_muted(&self) -> bool;

    /// C# `event EventHandler<AudioStateChangedEventArgs>? StateChanged`.
    fn state_changed(&self) -> &Event<AudioStateChangedEventArgs>;

    fn set_volume(&self, volume: f32);
    fn set_mute(&self, is_muted: bool);
    fn toggle_mute(&self);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioStateChangedEventArgs {
    pub volume: f32,
    pub is_muted: bool,
}

impl AudioStateChangedEventArgs {
    pub fn new(volume: f32, is_muted: bool) -> Self {
        AudioStateChangedEventArgs { volume, is_muted }
    }
}

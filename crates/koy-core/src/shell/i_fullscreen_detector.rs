/// Tam ekran uygulama (oyun, sunum, video) algılama sözleşmesi.
pub trait IFullscreenDetector: Send + Sync {
    /// Sistemde tam ekran bir uygulama olup olmadığını denetler.
    fn is_fullscreen(&self) -> bool;

    /// Belirtilen monitörde tam ekran bir uygulama olup olmadığını denetler (C# `IsFullscreen(nint)` aşırı yüklemesi).
    /// Varsayılan: monitörden bağımsız sorgu.
    fn is_fullscreen_on(&self, _target_monitor_handle: isize) -> bool {
        self.is_fullscreen()
    }
}

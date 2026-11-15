use crate::shell::{WindowInfo, WindowRect};

/// Aday pencere bilgisi (çoklu ekranda Z-sırasındaki açık pencereleri filtrelemek için).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DockCandidateWindow {
    pub handle: isize,
    pub bounds: WindowRect,
    pub monitor_handle: isize,
    pub is_maximized: bool,
    pub is_minimized: bool,
}

impl DockCandidateWindow {
    /// C# `new DockCandidateWindow(handle, bounds, monitorHandle)`: `IsMaximized` ve `IsMinimized` false.
    pub const fn new(handle: isize, bounds: WindowRect, monitor_handle: isize) -> Self {
        DockCandidateWindow { handle, bounds, monitor_handle, is_maximized: false, is_minimized: false }
    }

    pub const fn with_state(handle: isize, bounds: WindowRect, monitor_handle: isize, is_maximized: bool, is_minimized: bool) -> Self {
        DockCandidateWindow { handle, bounds, monitor_handle, is_maximized, is_minimized }
    }
}

/// Ön plandaki pencerenin hangi ekranın dock'unu etkileyeceğine karar verir.
/// Pencere yalnız bulunduğu ekranın (Windows'un MonitorFromWindow ile seçtiği, alanın çoğunu kaplayan) dock'unu etkiler.
/// Büyütülmüş pencere kenarlardan birkaç piksel taşar (ör. 0–1920 ekranda -8…1928); sınırlar bütün dock'lara verilince
/// bu taşma yan ekranın kenarına değiyor ve yan ekrandaki dock "pencere üstüme bindi" sanıp yapışık moda geçiyordu.
pub struct DockForegroundFilter;

impl DockForegroundFilter {
    /// Bu dock'a verilecek pencere sınırları: pencere başka ekrandaysa boş (çakışma yok).
    /// Pencerenin ekranı bilinmiyorsa (0) eski davranış korunur, sınırlar olduğu gibi verilir.
    pub fn bounds_for_dock(window_bounds: WindowRect, window_monitor: isize, dock_monitor: isize) -> WindowRect {
        if Self::applies_to_dock(window_monitor, dock_monitor) { window_bounds } else { WindowRect::EMPTY }
    }

    /// Pencere bu dock'un ekranında mı (ekranı bilinmiyorsa evet sayılır).
    pub fn applies_to_dock(window_monitor: isize, dock_monitor: isize) -> bool {
        window_monitor == 0 || window_monitor == dock_monitor
    }

    /// Çoklu ekranda ön plan penceresi başka bir ekrandaysa, bu dock'un kendi ekranındaki en üst
    /// görünür ve küçültülmemiş penceresini seçer. Adaylar Z-sırasına (üstten alta) göre verilmelidir.
    /// Uygun pencere bulunamazsa boş sınır döner. Dönüş: `(Bounds, IsMaximized, Handle)`.
    pub fn resolve_monitor_window(
        dock_monitor: isize,
        foreground: WindowInfo,
        foreground_monitor: isize,
        foreground_maximized: bool,
        candidates: Option<&[DockCandidateWindow]>,
    ) -> (WindowRect, bool, isize) {
        if Self::applies_to_dock(foreground_monitor, dock_monitor) {
            return (foreground.bounds, foreground_maximized, foreground.handle);
        }

        if let Some(c) = Self::find_top_window_for_monitor(dock_monitor, candidates) {
            return (c.bounds, c.is_maximized, c.handle);
        }

        (WindowRect::EMPTY, false, 0)
    }

    /// Verilen aday pencereler arasından bu monitördeki en üst görünür/küçültülmemiş pencereyi bulur.
    /// Adaylar Z-sırasına göre sıralı olmalıdır.
    pub fn find_top_window_for_monitor(dock_monitor: isize, candidates: Option<&[DockCandidateWindow]>) -> Option<DockCandidateWindow> {
        let candidates = candidates?;

        candidates.iter().find(|c| !c.is_minimized && !c.bounds.is_empty() && c.monitor_handle == dock_monitor).copied()
    }
}

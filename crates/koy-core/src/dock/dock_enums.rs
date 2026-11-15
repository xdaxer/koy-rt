use serde::{Deserialize, Serialize};

// Bu dosyadaki enum adları settings.json'a yazılır: bir kez yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Çalışan uygulama göstergesi: nokta (varsayılan) ya da çizgi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockIndicatorStyle {
    #[default]
    Dot,
    Line,
    Highlight,
}

/// Dock simgesine orta tık: uygulamanın yeni penceresini aç (varsayılan) ya da pencereyi kapat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum MiddleClickAction {
    #[default]
    NewWindow,
    Close,
}

/// Aynı uygulamanın pencereleri dock'ta tek simgede toplansın mı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockGroupingMode {
    /// Her pencere ayrı simge (bugünkü davranış).
    #[default]
    Never,

    /// Aynı süreç adına sahip pencereler her zaman tek simgede.
    Always,

    /// Pencere sayısı dock'un yer sayısını aşınca gruplanır, aksi halde her pencere ayrı simge.
    WhenSpaceRunsOut,
}

/// Dock'ta hangi monitörün pencereleri gösterilsin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockWindowMonitorScope {
    /// Bütün monitörlerin pencereleri (varsayılan).
    #[default]
    AllMonitors,

    /// Yalnız dock'un bulunduğu monitördeki pencereler.
    CurrentMonitor,
}

/// Dock simgesine sol tık. Varsayılan `MinimizeIfActive` (bugünkü dock davranışı); BringToFront etkinken de yalnız öne getirir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockLeftClickAction {
    #[default]
    BringToFront,
    MinimizeIfActive,
    CycleWindows,
    ShowThumbnails,
}

/// Dock simgelerinin altındaki uygulama adı etiketi: hiç (varsayılan), her zaman ya da yalnız üzerine gelince.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockLabelMode {
    #[default]
    Never,
    Always,
    OnHover,
}

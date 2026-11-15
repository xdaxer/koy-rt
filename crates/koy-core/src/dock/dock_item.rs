use crate::shell::WindowRect;

/// Dock öğesine ait açık bir pencerenin başlık ve tanıtıcı bilgisi (DWM önizlemesi ve küçültülmüş pencereler için).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DockWindowEntry {
    pub handle: isize,
    pub title: String,
    pub bounds: WindowRect,
    pub is_minimized: bool,
}

impl DockWindowEntry {
    /// C# `new DockWindowEntry(handle, title, bounds)`: `IsMinimized` false.
    pub fn new(handle: isize, title: impl Into<String>, bounds: WindowRect) -> Self {
        DockWindowEntry { handle, title: title.into(), bounds, is_minimized: false }
    }
}

/// Dock üzerinde gösterilen bir uygulamanın (sabitli, çalışan veya her ikisi) durum modeli.
/// Win32 veya WPF tipi içermez, saf modeldir. `Default`: C# isteğe bağlı parametre varsayılanları
/// (`ProcessId = 0`, `IsFlashing = false`, `BadgeCount = 0`, `Windows = null`, `AppUserModelId = null`) ile boş değerler.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DockItem {
    pub id: String,
    pub title: String,
    pub file_path: Option<String>,
    pub target_path: Option<String>,
    pub icon_data: Option<Vec<u8>>,
    pub is_pinned: bool,
    pub is_running: bool,
    pub is_active: bool,
    pub window_handles: Vec<isize>,
    pub process_id: i32,
    pub is_flashing: bool,
    pub badge_count: i32,
    pub windows: Option<Vec<DockWindowEntry>>,
    pub app_user_model_id: Option<String>,
}

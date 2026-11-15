use super::window_rect::WindowRect;
use crate::event::Event;

/// Pencere tanıtıcı bilgisi ve ekran konumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowInfo {
    pub handle: isize,
    pub bounds: WindowRect,
}

/// Açık olan bir üst düzey uygulama penceresinin bilgisi.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenWindowInfo {
    pub handle: isize,
    pub title: String,
    pub process_path: Option<String>,
    pub process_id: i32,
    pub bounds: WindowRect,
    pub icon_data: Option<Vec<u8>>,
    pub app_user_model_id: Option<String>,
    pub class_name: Option<String>,
}

impl OpenWindowInfo {
    /// C# kaydındaki zorunlu alanlar; isteğe bağlılar (`IconData`, `AppUserModelId`, `ClassName`) `None`.
    pub fn new(handle: isize, title: impl Into<String>, process_path: Option<String>, process_id: i32, bounds: WindowRect) -> Self {
        OpenWindowInfo {
            handle,
            title: title.into(),
            process_path,
            process_id,
            bounds,
            icon_data: None,
            app_user_model_id: None,
            class_name: None,
        }
    }
}

/// Ön plan penceresi, konum değişiklikleri ve açık pencereleri izleyen sözleşme.
/// Win32 tipleri içermez, pencere tanıtıcıları `isize` ile temsil edilir. (`IDisposable` → uygulayıcının `Drop`'u.)
pub trait IWindowTracker: Send + Sync {
    fn foreground_window_changed(&self) -> &Event<WindowInfo>;

    fn windows_changed(&self) -> &Event<()>;

    fn window_flashing(&self) -> &Event<isize>;

    fn current_foreground(&self) -> Option<WindowInfo>;

    fn get_open_windows(&self) -> Vec<OpenWindowInfo>;

    fn activate_window(&self, handle: isize);

    fn minimize_window(&self, handle: isize);

    fn close_window(&self, handle: isize);

    fn terminate_process(&self, process_id: i32);

    fn start(&self);

    fn stop(&self);

    /// `GetCursorPosition(out x, out y)`: başarılıysa `Some((x, y))`.
    fn get_cursor_position(&self) -> Option<(i32, i32)>;
}

//! Testlerde ortak kullanılan sahte servisler (C# testlerindeki `FakePinnedAppsService`, `FakeWindowTracker`,
//! `FakeShellActionService`, `FakeFullscreenDetector` ve benzerlerinin karşılığı).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::dock::{DockController, IPinnedAppsService, PinnedApp};
use crate::event::Event;
use crate::shell::{IFullscreenDetector, IShellActionService, IWindowTracker, OpenWindowInfo, WindowInfo};

#[derive(Default)]
pub struct FakePins {
    pub apps: Mutex<Vec<PinnedApp>>,
}

impl FakePins {
    pub fn new(apps: Vec<PinnedApp>) -> Arc<Self> {
        Arc::new(FakePins { apps: Mutex::new(apps) })
    }
}

impl IPinnedAppsService for FakePins {
    fn get_pinned_apps(&self) -> Vec<PinnedApp> {
        self.apps.lock().unwrap().clone()
    }
}

#[derive(Default)]
pub struct FakeTracker {
    pub windows: Mutex<Vec<OpenWindowInfo>>,
    pub current_foreground: Mutex<Option<WindowInfo>>,
    pub activated: Mutex<Vec<isize>>,
    pub minimized: Mutex<Vec<isize>>,
    pub closed: Mutex<Vec<isize>>,
    pub terminated: Mutex<Vec<i32>>,
    pub scan_count: AtomicUsize,
    foreground_window_changed: Event<WindowInfo>,
    windows_changed: Event<()>,
    window_flashing: Event<isize>,
}

impl FakeTracker {
    pub fn new() -> Arc<Self> {
        Arc::new(FakeTracker::default())
    }

    pub fn with_windows(windows: Vec<OpenWindowInfo>) -> Arc<Self> {
        let t = FakeTracker::default();
        *t.windows.lock().unwrap() = windows;
        Arc::new(t)
    }

    pub fn set_windows(&self, windows: Vec<OpenWindowInfo>) {
        *self.windows.lock().unwrap() = windows;
    }

    pub fn set_foreground(&self, info: Option<WindowInfo>) {
        *self.current_foreground.lock().unwrap() = info;
    }

    pub fn trigger_window_flashing(&self, handle: isize) {
        self.window_flashing.invoke(&handle);
    }

    pub fn trigger_foreground_change(&self, info: WindowInfo) {
        self.set_foreground(Some(info));
        self.foreground_window_changed.invoke(&info);
    }

    pub fn trigger_windows_change(&self) {
        self.windows_changed.invoke(&());
    }

    pub fn activated(&self) -> Vec<isize> {
        self.activated.lock().unwrap().clone()
    }

    pub fn minimized(&self) -> Vec<isize> {
        self.minimized.lock().unwrap().clone()
    }

    pub fn closed(&self) -> Vec<isize> {
        self.closed.lock().unwrap().clone()
    }

    pub fn terminated(&self) -> Vec<i32> {
        self.terminated.lock().unwrap().clone()
    }

    pub fn scan_count(&self) -> usize {
        self.scan_count.load(Ordering::SeqCst)
    }
}

impl IWindowTracker for FakeTracker {
    fn foreground_window_changed(&self) -> &Event<WindowInfo> {
        &self.foreground_window_changed
    }

    fn windows_changed(&self) -> &Event<()> {
        &self.windows_changed
    }

    fn window_flashing(&self) -> &Event<isize> {
        &self.window_flashing
    }

    fn current_foreground(&self) -> Option<WindowInfo> {
        *self.current_foreground.lock().unwrap()
    }

    fn get_open_windows(&self) -> Vec<OpenWindowInfo> {
        self.scan_count.fetch_add(1, Ordering::SeqCst);
        self.windows.lock().unwrap().clone()
    }

    fn activate_window(&self, handle: isize) {
        self.activated.lock().unwrap().push(handle);
    }

    fn minimize_window(&self, handle: isize) {
        self.minimized.lock().unwrap().push(handle);
    }

    fn close_window(&self, handle: isize) {
        self.closed.lock().unwrap().push(handle);
    }

    fn terminate_process(&self, process_id: i32) {
        self.terminated.lock().unwrap().push(process_id);
    }

    fn start(&self) {}

    fn stop(&self) {}

    fn get_cursor_position(&self) -> Option<(i32, i32)> {
        Some((0, 0))
    }
}

#[derive(Default)]
pub struct FakeShell {
    pub launched: Mutex<Vec<String>>,
    pub win_key_count: AtomicUsize,
    pub tray_toggle_count: AtomicUsize,
}

impl FakeShell {
    pub fn new() -> Arc<Self> {
        Arc::new(FakeShell::default())
    }

    pub fn launched(&self) -> Vec<String> {
        self.launched.lock().unwrap().clone()
    }
}

impl IShellActionService for FakeShell {
    fn send_win_key(&self) {
        self.win_key_count.fetch_add(1, Ordering::SeqCst);
    }

    fn toggle_system_tray(&self) {
        self.tray_toggle_count.fetch_add(1, Ordering::SeqCst);
    }

    fn open_quick_settings(&self) {}

    fn open_notification_center(&self) {}

    fn launch(&self, path: &str, _arguments: Option<&str>) -> Result<(), String> {
        self.launched.lock().unwrap().push(path.to_string());
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeFullscreen {
    pub fullscreen: AtomicBool,
}

impl IFullscreenDetector for FakeFullscreen {
    fn is_fullscreen(&self) -> bool {
        self.fullscreen.load(Ordering::SeqCst)
    }
}

/// `new DockController(pins, tracker, shell)`: yenileme eşzamanlı, tam ekran algılayıcı yok.
pub fn controller(pins: Arc<dyn IPinnedAppsService>, tracker: Arc<FakeTracker>, shell: Arc<FakeShell>) -> DockController {
    DockController::new(pins, tracker, shell, None, None, None, None)
}

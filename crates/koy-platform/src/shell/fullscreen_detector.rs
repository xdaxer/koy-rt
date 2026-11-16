//! Tam ekran uygulama algılayıcı. SHQueryUserNotificationState ve ön plan penceresinin monitör sınırlarını kaplama
//! durumunu denetler.

use std::collections::HashSet;
use std::sync::Mutex;

use koy_core::shell::{FullscreenRule, IFullscreenDetector, ShellNotificationState, ShellNotificationStates, WindowRect};
use koy_core::text;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows::Win32::UI::Shell::SHQueryUserNotificationState;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GWL_STYLE, GetClassNameW, GetDesktopWindow, GetForegroundWindow, GetShellWindow, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, WS_CAPTION, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_MAXIMIZE,
};
use windows::core::BOOL;

use super::window_styles::{get_process_path, hwnd};

const SHELL_CLASSES: [&str; 5] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "ScreenClippingHost"];

pub(crate) fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: arabellek boyutu verilir.
    let len = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

fn rect_of(h: HWND) -> Option<WindowRect> {
    let mut r = RECT::default();
    // SAFETY: çıktı yerel.
    unsafe { GetWindowRect(h, &mut r) }.ok()?;
    Some(WindowRect { left: r.left, top: r.top, right: r.right, bottom: r.bottom })
}

fn monitor_rect(mon: HMONITOR) -> Option<WindowRect> {
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: yapı boyutu ayarlandı.
    if !unsafe { GetMonitorInfoW(mon, &mut mi) }.as_bool() {
        return None;
    }
    let r = mi.rcMonitor;
    Some(WindowRect { left: r.left, top: r.top, right: r.right, bottom: r.bottom })
}

fn style_flags(h: HWND) -> (bool, bool) {
    // SAFETY: yalnız stil okuma.
    let style = unsafe { GetWindowLongW(h, GWL_STYLE) } as u32;
    (style & WS_CAPTION.0 == WS_CAPTION.0, style & WS_MAXIMIZE.0 != 0)
}

pub struct FullscreenDetector {
    notch_handle: Mutex<isize>,
    ignored_handles: Mutex<HashSet<isize>>,
}

impl Default for FullscreenDetector {
    fn default() -> Self {
        FullscreenDetector { notch_handle: Mutex::new(0), ignored_handles: Mutex::new(HashSet::new()) }
    }
}

impl FullscreenDetector {
    pub fn notch_handle(&self) -> isize {
        *self.notch_handle.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_notch_handle(&self, h: isize) {
        *self.notch_handle.lock().unwrap_or_else(|e| e.into_inner()) = h;
    }

    /// C# `IgnoredHandles` kümesi.
    pub fn ignored_handles(&self) -> std::sync::MutexGuard<'_, HashSet<isize>> {
        self.ignored_handles.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn is_ignored(&self, h: isize) -> bool {
        self.ignored_handles().contains(&h)
    }

    fn is_top_window_fullscreen_on(&self, monitor_handle: isize) -> bool {
        struct Ctx<'a> {
            detector: &'a FullscreenDetector,
            monitor: isize,
            own_pid: u32,
            notch: isize,
            result: Option<bool>,
        }

        unsafe extern "system" fn visit(h: HWND, lparam: LPARAM) -> BOOL {
            // SAFETY: lparam yalnız EnumWindows süresince geçerli olan bağlamımızı gösterir.
            let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
            let handle = h.0 as isize;
            // SAFETY: pencere sorguları.
            unsafe {
                if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() || ctx.detector.is_ignored(handle) || handle == ctx.notch {
                    return BOOL(1);
                }
                let mut pid = 0u32;
                if GetWindowThreadProcessId(h, Some(&mut pid)) == 0 || pid == ctx.own_pid {
                    return BOOL(1);
                }
                // Gizlenmiş (cloaked) UWP/başka sanal masaüstü pencereleri görünür sayılmaz.
                let mut cloaked = 0i32;
                if DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut i32 as *mut _, 4).is_ok() && cloaked != 0 {
                    return BOOL(1);
                }
                let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
                if ex & (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_TRANSPARENT.0) != 0 {
                    return BOOL(1);
                }
            }
            let cn = class_name(h);
            if SHELL_CLASSES.contains(&cn.as_str()) {
                return BOOL(1);
            }
            let Some(wr) = rect_of(h) else { return BOOL(1) };
            // SAFETY: yalnız sorgu.
            let mon = unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) };
            if mon.0 as isize != ctx.monitor {
                return BOOL(1);
            }
            let Some(mr) = monitor_rect(mon) else { return BOOL(1) };
            let (has_caption, is_maximized) = style_flags(h);
            ctx.result = Some(FullscreenRule::is_fullscreen(wr, mr, has_caption, is_maximized, ShellNotificationState::Unknown));
            BOOL(0) // ilk gerçek pencere karar verdi, taramayı bitir
        }

        let mut ctx = Ctx { detector: self, monitor: monitor_handle, own_pid: std::process::id(), notch: self.notch_handle(), result: None };
        // EnumWindows üstten alta (z sırası) gezer; hedef monitördeki ilk gerçek pencere karar verir.
        // SAFETY: bağlam çağrı süresince yaşar.
        let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut ctx as *mut Ctx as isize)) };
        ctx.result == Some(true)
    }

    fn is_foreground_fullscreen(&self, target_monitor: isize) -> bool {
        // SAFETY: yalnız sorgular.
        let fg = unsafe { GetForegroundWindow() };
        if fg.is_invalid() || fg == unsafe { GetDesktopWindow() } || fg == unsafe { GetShellWindow() } {
            return false;
        }
        let fg_handle = fg.0 as isize;
        let notch = self.notch_handle();
        if (notch != 0 && fg_handle == notch) || self.is_ignored(fg_handle) {
            return false;
        }
        // Masaüstü pencerelerini ya da görev çubuklarını yoksay.
        if SHELL_CLASSES.contains(&class_name(fg).as_str()) || Self::is_screen_clipping_host(fg_handle) {
            return false;
        }
        // SAFETY: yalnız sorgu.
        let fg_mon = unsafe { MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST) };
        if fg_mon.is_invalid() {
            return false;
        }
        // Belirli bir monitör sorgulanıyorsa ve ön plan penceresi başka monitördeyse bu ekran için tam ekran sayılmaz.
        if target_monitor != 0 && fg_mon.0 as isize != target_monitor {
            return false;
        }
        // Üç işaret birleşir (FullscreenRule): Windows'un tam ekran bildirimi, başlık çubuğunun olmayışı ve monitörü tamamen
        // kaplama. Sorgu başarısızsa durum bilinmiyor sayılır.
        // SAFETY: yalnız sorgu.
        let shell_state = unsafe { SHQueryUserNotificationState() }.map_or(ShellNotificationState::Unknown, |s| ShellNotificationStates::from_quns(s.0));
        let Some(mr) = monitor_rect(fg_mon) else { return false };
        let Some(wr) = rect_of(fg) else { return false };
        let (has_caption, is_maximized) = style_flags(fg);
        FullscreenRule::is_fullscreen(wr, mr, has_caption, is_maximized, shell_state)
    }

    pub fn is_screen_clipping_host(handle: isize) -> bool {
        if handle == 0 {
            return false;
        }
        let h = hwnd(handle);
        if class_name(h) == "ScreenClippingHost" {
            return true;
        }
        let mut pid = 0u32;
        // SAFETY: çıktı yerel.
        unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
        if pid != 0
            && let Some(path) = get_process_path(pid)
        {
            let name = koy_core::win_path::get_file_name_without_extension(&path);
            return text::eq_ignore_case(name, "ScreenClippingHost") || text::eq_ignore_case(name, "SnippingTool");
        }
        false
    }
}

impl IFullscreenDetector for FullscreenDetector {
    fn is_fullscreen(&self) -> bool {
        self.is_fullscreen_on(0)
    }

    /// Belirtilen monitörde tam ekran var mı (0: ön plan hangi monitördeyse orada). Odak başka ekrandaki bir pencerede
    /// olsa bile bu monitörün en üstündeki gerçek pencere tam ekransa o ekran tam ekran sayılır.
    fn is_fullscreen_on(&self, target_monitor_handle: isize) -> bool {
        if self.is_foreground_fullscreen(target_monitor_handle) {
            return true;
        }
        target_monitor_handle != 0 && self.is_top_window_fullscreen_on(target_monitor_handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_without_failing() {
        let d = FullscreenDetector::default();
        let _ = d.is_fullscreen();
        assert!(!FullscreenDetector::is_screen_clipping_host(0));
    }
}

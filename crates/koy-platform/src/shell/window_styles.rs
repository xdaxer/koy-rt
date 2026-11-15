//! Notch penceresinin Windows stil ayarları. Alt+Tab'dan gizler ve tıklamada odak çalmasını önler.

use koy_core::shell::WindowRect;
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GWL_STYLE, GetForegroundWindow, GetWindowLongW, GetWindowThreadProcessId, HWND_TOPMOST, IsIconic, SET_WINDOW_POS_FLAGS,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowLongW, SetWindowPos,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};
use windows::core::PWSTR;

pub(crate) fn hwnd(h: isize) -> HWND {
    HWND(h as *mut _)
}

fn ex_style(h: HWND) -> u32 {
    // SAFETY: yalnız stil okuma.
    unsafe { GetWindowLongW(h, GWL_EXSTYLE) as u32 }
}

fn set_ex_style(h: HWND, style: u32) {
    // SAFETY: stil yazma; geçersiz pencerede çağrı başarısız olur.
    let _ = unsafe { SetWindowLongW(h, GWL_EXSTYLE, style as i32) };
}

fn refresh_frame(h: HWND) {
    // SAFETY: konum ve boyut değişmez, yalnız çerçeve yenilenir.
    let _ = unsafe { SetWindowPos(h, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED) };
}

/// Sınırlı sorgu hakkıyla süreç exe yolu (yönetici süreçlerde de çalışır); çözülemezse `None`.
pub(crate) fn get_process_path(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    // SAFETY: süreç tutamacı her yolda kapatılır; arabellek boyutu verilir.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size).is_ok();
        let _ = CloseHandle(process);
        ok.then(|| String::from_utf16_lossy(&buf[..size as usize]))
    }
}

pub struct WindowStyles;

impl WindowStyles {
    pub fn apply_tool_window_and_no_activate(h: isize) {
        let h = hwnd(h);
        set_ex_style(h, ex_style(h) | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0);
        refresh_frame(h);
    }

    /// Odak çalmama (WS_EX_NOACTIVATE) stilini açar ya da kapatır. Çentikte yazı yazılacak bir alana tıklanınca geçici
    /// olarak kapatılır (etkinliksiz pencere klavye girdisi alamaz), odak gidince yeniden açılır.
    pub fn set_no_activate(h: isize, no_activate: bool) {
        let h = hwnd(h);
        let s = ex_style(h);
        set_ex_style(h, if no_activate { s | WS_EX_NOACTIVATE.0 } else { s & !WS_EX_NOACTIVATE.0 });
    }

    /// Pencereyi tıklama geçirgen (WS_EX_TRANSPARENT) yapar ya da tık alabilir hale getirir.
    pub fn set_click_through(h: isize, click_through: bool) {
        let h = hwnd(h);
        let s = ex_style(h);
        set_ex_style(h, if click_through { s | WS_EX_TRANSPARENT.0 } else { s & !WS_EX_TRANSPARENT.0 });
        refresh_frame(h);
    }

    /// Pencereyi fiziksel piksel koordinatlarıyla yerleştirir (z sırası ve odak değişmez). `resize` false ise yalnız konum.
    pub fn set_bounds(h: isize, rect: WindowRect, resize: bool) {
        let mut flags: SET_WINDOW_POS_FLAGS = SWP_NOZORDER | SWP_NOACTIVATE;
        if !resize {
            flags |= SWP_NOSIZE;
        }
        // SAFETY: konum çağrısı.
        let _ = unsafe { SetWindowPos(hwnd(h), None, rect.left, rect.top, rect.width(), rect.height(), flags) };
    }

    /// Odak çalmadan pencereyi en üste (Topmost) taşır.
    pub fn reassert_topmost(h: isize) {
        // SAFETY: z sırası çağrısı.
        let _ = unsafe { SetWindowPos(hwnd(h), Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    }

    /// Pencereyi ön plana (foreground) taşır.
    pub fn set_foreground(h: isize) {
        if h == 0 {
            return;
        }
        // SAFETY: ön plan isteği.
        let _ = unsafe { SetForegroundWindow(hwnd(h)) };
    }

    /// Geçerli ön plan penceresinin tanıtıcısı.
    pub fn get_foreground() -> isize {
        // SAFETY: yalnız sorgu.
        unsafe { GetForegroundWindow() }.0 as isize
    }

    /// Pencere ekranı kaplıyor mu (WS_MAXIMIZE).
    pub fn is_window_maximized(h: isize) -> bool {
        if h == 0 {
            return false;
        }
        const WS_MAXIMIZE: u32 = 0x0100_0000;
        // SAFETY: yalnız stil okuma.
        (unsafe { GetWindowLongW(hwnd(h), GWL_STYLE) } as u32) & WS_MAXIMIZE != 0
    }

    /// Pencere simge durumunda mı (IsIconic).
    pub fn is_window_minimized(h: isize) -> bool {
        // SAFETY: yalnız sorgu.
        h != 0 && unsafe { IsIconic(hwnd(h)) }.as_bool()
    }

    /// Pencerenin ait olduğu süreç kimliği (PID).
    pub fn get_window_process_id(h: isize) -> u32 {
        if h == 0 {
            return 0;
        }
        let mut pid = 0u32;
        // SAFETY: çıktı yerel değişkene yazılır.
        unsafe { GetWindowThreadProcessId(hwnd(h), Some(&mut pid)) };
        pid
    }

    /// Pencerenin süreç exe adı, uzantısız (ör. "chrome"); çözülemezse `None`.
    pub fn get_window_process_name(h: isize) -> Option<String> {
        let path = get_process_path(Self::get_window_process_id(h)).filter(|p| !p.is_empty())?;
        Some(koy_core::win_path::get_file_name_without_extension(&path).to_string())
    }

    /// Pencerenin bulunduğu monitörün tanıtıcısı (HMONITOR).
    pub fn get_window_monitor(h: isize) -> isize {
        if h == 0 {
            return 0;
        }
        // SAFETY: yalnız sorgu.
        unsafe { MonitorFromWindow(hwnd(h), MONITOR_DEFAULTTONEAREST) }.0 as isize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_own_process_path() {
        let path = get_process_path(std::process::id()).unwrap();
        assert!(path.to_ascii_lowercase().ends_with(".exe"));
        assert!(!WindowStyles::is_window_maximized(0));
        assert_eq!(WindowStyles::get_window_process_id(0), 0);
    }
}

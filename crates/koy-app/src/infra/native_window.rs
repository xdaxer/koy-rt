//! Slint pencereleri için yerel (Win32) yardımcılar: HWND erişimi, pencere stilleri, DWM çerçevesi ve pencere
//! yordamına alt sınıfla ileti dinleme (WPF `HwndSource.AddHook` / `Window.Deactivated` karşılığı).

use std::cell::RefCell;
use std::collections::HashMap;

use koy_core::diagnostics::{CrashError, CrashLog};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetCursorPos, GetWindowLongW, GetWindowRect, HWND_TOPMOST, IsWindowVisible, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowLongW, SetWindowPos, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

pub const WM_ACTIVATE: u32 = 0x0006;
pub const WM_ACTIVATEAPP: u32 = 0x001C;
pub const WM_DISPLAYCHANGE: u32 = 0x007E;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const WM_SETTINGCHANGE: u32 = 0x001A;

/// Slint penceresinin HWND'si (pencere henüz oluşmadıysa 0).
pub fn hwnd_of(window: &slint::Window) -> isize {
    match window.window_handle().window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
        _ => 0,
    }
}

fn h(hwnd: isize) -> HWND {
    HWND(hwnd as *mut _)
}

/// Çerçevesiz açılır pencere (WPF `WindowStyle=None` karşılığı): başlık, sistem menüsü ve kenarlık bitleri atılır,
/// `WS_POPUP` verilir. winit `no-frame` pencerede bile `WS_CAPTION` kalır; pencere bölgesi (SetWindowRgn) değişince
/// Windows başlık çubuğunu bölgenin içine çizebiliyordu (dock'un üstünde "Koy Dock" yazılı açık mavi şerit).
pub fn make_popup(hwnd: isize) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
    };
    if hwnd == 0 {
        return;
    }
    // SAFETY: kendi penceremizin stil bitleri değişir; SWP_FRAMECHANGED çerçeveyi yeniden hesaplatır.
    unsafe {
        let style = GetWindowLongW(h(hwnd), GWL_STYLE) as u32;
        let strip = WS_CAPTION.0 | WS_SYSMENU.0 | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0;
        let next = (style & !strip) | WS_POPUP.0;
        if next != style {
            SetWindowLongW(h(hwnd), GWL_STYLE, next as i32);
            let _ = SetWindowPos(h(hwnd), None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        }
    }
}

/// Görev çubuğunda düğmesi olmayan araç penceresi (C# `ShowInTaskbar = false`).
pub fn set_tool_window(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    // SAFETY: stil okuma/yazma ve çerçeve yenileme.
    unsafe {
        let ex = GetWindowLongW(h(hwnd), GWL_EXSTYLE) as u32;
        SetWindowLongW(h(hwnd), GWL_EXSTYLE, ((ex | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0) as i32);
        let _ = SetWindowPos(h(hwnd), None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    }
}

/// DWM çerçevesini bütün istemci alanına uzatır (C# `WindowChrome GlassFrameThickness=-1`): sistem arka planı
/// (akrilik), gölge ve köşeler DWM'den gelir.
pub fn extend_frame(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
    // SAFETY: pencere tanıtıcısı geçerli.
    let _ = unsafe { DwmExtendFrameIntoClientArea(h(hwnd), &margins) };
}

pub fn set_topmost(hwnd: isize) {
    if hwnd != 0 {
        // SAFETY: z sırası çağrısı.
        let _ = unsafe { SetWindowPos(h(hwnd), Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    }
}

/// Pencereyi öne alır ve etkinleştirir (C# `Activate()`).
/// Pencereyi imlecin bulunduğu ekranın çalışma alanında ortalar (WPF `WindowStartupLocation="CenterScreen"`).
pub fn center_on_cursor_monitor(hwnd: isize) {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint};
    if hwnd == 0 {
        return;
    }
    // SAFETY: salt okunur sorgular ve kendi penceremizin konumu.
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(mon, &mut mi).as_bool() {
            return;
        }
        let mut r = RECT::default();
        if GetWindowRect(h(hwnd), &mut r).is_err() {
            return;
        }
        let (w, ht) = (r.right - r.left, r.bottom - r.top);
        let wa = mi.rcWork;
        let x = wa.left + ((wa.right - wa.left) - w).max(0) / 2;
        let y = wa.top + ((wa.bottom - wa.top) - ht).max(0) / 2;
        let _ = SetWindowPos(h(hwnd), None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

pub fn activate(hwnd: isize) {
    if hwnd != 0 {
        // SAFETY: ön plan isteği.
        let _ = unsafe { SetForegroundWindow(h(hwnd)) };
    }
}

pub fn is_visible(hwnd: isize) -> bool {
    // SAFETY: yalnız sorgu.
    hwnd != 0 && unsafe { IsWindowVisible(h(hwnd)) }.as_bool()
}

pub fn window_rect(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    let mut r = RECT::default();
    // SAFETY: çıktı yerel.
    unsafe { GetWindowRect(h(hwnd), &mut r) }.ok()?;
    Some((r.left, r.top, r.right, r.bottom))
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    // SAFETY: çıktı yerel.
    unsafe { GetCursorPos(&mut p) }.ok().map(|_| (p.x, p.y))
}

/// İleti işleyicisi: `Some(sonuç)` işlendi demektir, `None` Slint'in kendi yordamına gider.
pub type Hook = Box<dyn FnMut(u32, WPARAM, LPARAM) -> Option<isize>>;

thread_local! {
    static HOOKS: RefCell<HashMap<isize, Vec<(usize, Hook)>>> = RefCell::new(HashMap::new());
    static NEXT: std::cell::Cell<usize> = const { std::cell::Cell::new(1) };
}

const SUBCLASS_ID: usize = 0x4B4F59; // "KOY"

unsafe extern "system" fn subclass_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    let key = hwnd.0 as isize;
    // İşleyiciler geçici olarak çıkarılır: işleyici içinde yeni kanca eklenebilir ya da pencere kapanabilir.
    let taken = HOOKS.with(|m| m.borrow_mut().remove(&key));
    let mut result = None;
    if let Some(mut hooks) = taken {
        for (_, hook) in hooks.iter_mut() {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hook(msg, wparam, lparam))) {
                Ok(Some(r)) => {
                    result = Some(r);
                    break;
                }
                Ok(None) => {}
                Err(_) => CrashLog::write("NativeWindow.WndProc", &CrashError::exception("Panic", "hook panicked", "")),
            }
        }
        HOOKS.with(|m| {
            let mut m = m.borrow_mut();
            let entry = m.entry(key).or_default();
            // Çağrı sırasında eklenenler sona kalır.
            let added = std::mem::take(entry);
            *entry = hooks;
            entry.extend(added);
        });
    }
    if let Some(r) = result {
        return LRESULT(r);
    }
    // SAFETY: zincirdeki bir sonraki yordama iletilir.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Pencere iletilerini dinler; dönen kimlikle `remove_hook` ile kaldırılır.
pub fn add_hook(hwnd: isize, hook: Hook) -> usize {
    if hwnd == 0 {
        return 0;
    }
    let id = NEXT.with(|n| {
        let v = n.get();
        n.set(v + 1);
        v
    });
    let first = HOOKS.with(|m| {
        let mut m = m.borrow_mut();
        let entry = m.entry(hwnd).or_default();
        entry.push((id, hook));
        entry.len() == 1
    });
    if first {
        // SAFETY: alt sınıf bu iş parçacığında kurulur; pencere kapanınca remove_hook çağrılır.
        let _ = unsafe { SetWindowSubclass(h(hwnd), Some(subclass_proc), SUBCLASS_ID, 0) };
    }
    id
}

pub fn remove_hook(hwnd: isize, id: usize) {
    let empty = HOOKS.with(|m| {
        let mut m = m.borrow_mut();
        let Some(list) = m.get_mut(&hwnd) else { return false };
        list.retain(|(i, _)| *i != id);
        if list.is_empty() {
            m.remove(&hwnd);
            true
        } else {
            false
        }
    });
    if empty {
        // SAFETY: kendi alt sınıfımız.
        let _ = unsafe { RemoveWindowSubclass(h(hwnd), Some(subclass_proc), SUBCLASS_ID) };
    }
}

/// WM_ACTIVATE'in "etkinliğini kaybetti" (WA_INACTIVE) olup olmadığı.
pub fn is_deactivate(msg: u32, wparam: WPARAM) -> bool {
    msg == WM_ACTIVATE && (wparam.0 & 0xFFFF) == 0
}

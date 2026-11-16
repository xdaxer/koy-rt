//! Gizli, üst düzey ileti penceresi (WPF `HwndSource` + `AddHook` karşılığı).
//!
//! Arayüz iş parçacığında kurulur; iletileri Slint/winit'in mesaj döngüsü dağıtır. Yalnız ileti alan (HWND_MESSAGE)
//! pencere yayınları (TaskbarCreated, WM_DISPLAYCHANGE) almadığı için görünmez üst düzey pencere kullanılır; stili 0,
//! boyutu 0 (C#'taki `HwndSourceParameters { WindowStyle = 0, Width = 0, Height = 0 }`).

use std::cell::RefCell;
use std::sync::Once;

use koy_core::diagnostics::{CrashError, CrashLog};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetWindowLongPtrW, RegisterClassExW, SetWindowLongPtrW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WNDCLASSEXW,
};
use windows::core::{HSTRING, PCWSTR, w};

const CLASS_NAME: PCWSTR = w!("KoyMessageWindow");

/// İleti işleyicisi: `Some(sonuç)` işlendi demektir (C# `handled = true`), `None` varsayılan işleyiciye gider.
pub type Handler = Box<dyn FnMut(u32, WPARAM, LPARAM) -> Option<isize>>;

struct Slot {
    source: String,
    handler: RefCell<Handler>,
}

pub struct MessageWindow {
    hwnd: isize,
    // Pencere yordamı bu kutunun adresini GWLP_USERDATA'dan okur; pencere yok edilene dek yaşamalı.
    slot: *mut Slot,
}

unsafe extern "system" fn wnd_proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: GWLP_USERDATA yalnız bizim Slot göstergemizi tutar; pencere yok edilmeden önce sıfırlanır.
    let slot = unsafe { GetWindowLongPtrW(h, GWLP_USERDATA) } as *const Slot;
    if !slot.is_null() {
        // SAFETY: Slot MessageWindow düşene dek yaşar.
        let slot = unsafe { &*slot };
        // Yeniden girişte (işleyici içinde SendMessage) ikinci ödünç alma reddedilir; ileti varsayılana gider.
        if let Ok(mut handler) = slot.handler.try_borrow_mut() {
            // WndProc'tan sızan panik süreci düşürür; logla ve yut.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (handler)(msg, wparam, lparam))) {
                Ok(Some(r)) => return LRESULT(r),
                Ok(None) => {}
                Err(_) => {
                    CrashLog::write(&slot.source, &CrashError::exception("Panic", "WndProc panicked", ""));
                    return LRESULT(0);
                }
            }
        }
    }
    // SAFETY: varsayılan işleyici.
    unsafe { DefWindowProcW(h, msg, wparam, lparam) }
}

fn register_class() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: süreç ömrü boyunca bir kez kaydedilir.
        unsafe {
            let instance = GetModuleHandleW(PCWSTR::null()).unwrap_or_default();
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance.into(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            RegisterClassExW(&wc);
        }
    });
}

impl MessageWindow {
    /// `title`: pencere adı (C# HwndSourceParameters adı; tanılama için). `source`: CrashLog kaynak adı.
    pub fn new(title: &str, source: &str, handler: Handler) -> Option<MessageWindow> {
        register_class();
        let slot = Box::into_raw(Box::new(Slot { source: source.to_string(), handler: RefCell::new(handler) }));
        // SAFETY: pencere bu iş parçacığında kurulur; gösterge pencere ömrü boyunca geçerli.
        unsafe {
            let instance = GetModuleHandleW(PCWSTR::null()).unwrap_or_default();
            let h = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASS_NAME,
                &HSTRING::from(title),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            );
            match h {
                Ok(h) => {
                    SetWindowLongPtrW(h, GWLP_USERDATA, slot as isize);
                    Some(MessageWindow { hwnd: h.0 as isize, slot })
                }
                Err(_) => {
                    drop(Box::from_raw(slot));
                    None
                }
            }
        }
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd
    }
}

impl Drop for MessageWindow {
    fn drop(&mut self) {
        // SAFETY: önce gösterge sökülür, sonra pencere yok edilir, en son kutu bırakılır.
        unsafe {
            let h = HWND(self.hwnd as *mut _);
            SetWindowLongPtrW(h, GWLP_USERDATA, 0);
            let _ = DestroyWindow(h);
            drop(Box::from_raw(self.slot));
        }
    }
}

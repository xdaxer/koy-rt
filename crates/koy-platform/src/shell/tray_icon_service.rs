//! Windows bildirim alanı (System Tray) simgesi için Shell_NotifyIcon sarmalayıcısı. Dock kapalıyken kullanıcının Koy
//! menüsüne erişebilmesini sağlar.

use std::sync::{Mutex, OnceLock};

use windows::Win32::UI::Shell::{ExtractIconExW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON, RegisterWindowMessageW};
use windows::core::{HSTRING, w};

use super::window_styles::hwnd;

const ICON_ID: u32 = 1001;

struct State {
    icon: isize,
    is_added: bool,
    tooltip: String,
}

pub struct TrayIconService {
    hwnd: isize,
    callback_message: u32,
    state: Mutex<State>,
}

impl TrayIconService {
    /// Explorer (yeniden) başlayıp görev çubuğunu oluşturunca bütün üst düzey pencerelere yayınlanan mesaj. Tepsi simgeleri
    /// Explorer ile birlikte silindiği için bu mesajda simge yeniden eklenmeli.
    pub fn taskbar_created_message() -> u32 {
        static MSG: OnceLock<u32> = OnceLock::new();
        // SAFETY: sabit ad.
        *MSG.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) })
    }

    /// C# varsayılanı: `callback_message = 0x8001`.
    pub fn new(hwnd: isize, callback_message: u32) -> Self {
        TrayIconService { hwnd, callback_message, state: Mutex::new(State { icon: 0, is_added: false, tooltip: "Koy".into() }) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn destroy_icon(st: &mut State) {
        if st.icon != 0 {
            // SAFETY: ExtractIconEx'in verdiği simge bize ait.
            let _ = unsafe { DestroyIcon(HICON(st.icon as *mut _)) };
            st.icon = 0;
        }
    }

    pub fn add(&self, tooltip: &str) -> bool {
        let mut st = self.lock();
        if st.is_added || self.hwnd == 0 {
            return false;
        }
        st.tooltip = tooltip.to_string();

        // Uygulamanın kendi gömülü simgesi (exe); çıkarılamazsa simgesiz devam edilir.
        if let Ok(exe) = std::env::current_exe() {
            let mut small = HICON::default();
            // SAFETY: tek küçük simge çıkarılır; sahipliği bizde, Remove/Recreate'te yok edilir.
            let extracted = unsafe { ExtractIconExW(&HSTRING::from(exe.as_os_str()), 0, None, Some(&mut small), 1) };
            if extracted > 0 && !small.is_invalid() {
                st.icon = small.0 as isize;
            }
        }

        let mut flags = NIF_MESSAGE | NIF_TIP;
        if st.icon != 0 {
            flags |= NIF_ICON;
        }
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd(self.hwnd),
            uID: ICON_ID,
            uFlags: flags,
            uCallbackMessage: self.callback_message,
            hIcon: HICON(st.icon as *mut _),
            ..Default::default()
        };
        let units: Vec<u16> = tooltip.encode_utf16().take(127).collect();
        data.szTip[..units.len()].copy_from_slice(&units);
        data.szTip[units.len()] = 0;

        // SAFETY: yapı tamamen dolduruldu.
        st.is_added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool();
        st.is_added
    }

    /// Explorer yeniden başlatıldıktan sonra simgeyi yeniden ekler (eski kayıt zaten silinmiştir).
    pub fn recreate(&self) -> bool {
        let tooltip = {
            let mut st = self.lock();
            st.is_added = false;
            Self::destroy_icon(&mut st);
            st.tooltip.clone()
        };
        self.add(&tooltip)
    }

    pub fn remove(&self) {
        let mut st = self.lock();
        if !st.is_added || self.hwnd == 0 {
            return;
        }
        let data = NOTIFYICONDATAW { cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd(self.hwnd), uID: ICON_ID, ..Default::default() };
        // SAFETY: kendi simgemiz silinir.
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
        st.is_added = false;
        Self::destroy_icon(&mut st);
    }
}

impl Drop for TrayIconService {
    fn drop(&mut self) {
        self.remove();
    }
}

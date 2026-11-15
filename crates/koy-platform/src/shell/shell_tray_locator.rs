//! Windows Gezgini'nin görev çubuğu penceresini bulur.
//! Koy kendi tepsisini açınca aynı sınıf adlı (Shell_TrayWnd) ikinci bir pencere olur ve FindWindow onu
//! döndürebilir; görev çubuğunu gizleyip geri getiren kod yanlış pencereye dokunmasın diye Koy'un pencereleri atlanır.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetWindowThreadProcessId};
use windows::core::{PCWSTR, w};

pub const TRAY_CLASS_NAME: PCWSTR = w!("Shell_TrayWnd");

/// `alsoSkipProcessId`: atlanacak ek süreç (gözcü, ölen Koy sürecinin pencerelerini atlar). 0 ise yalnız kendi
/// sürecimiz atlanır.
pub fn find_explorer_tray(also_skip_process_id: u32) -> Option<HWND> {
    let own_process_id = std::process::id();
    let mut hwnd = HWND::default();
    loop {
        // SAFETY: sınıf adı sabit, geniş karakterli ve sıfırla biter; HWND değerleri yalnız karşılaştırılır.
        hwnd = unsafe { FindWindowExW(None, Some(hwnd), TRAY_CLASS_NAME, PCWSTR::null()) }.unwrap_or_default();
        if hwnd.is_invalid() {
            return None;
        }
        let mut process_id = 0u32;
        // SAFETY: process_id yerel değişkene yazılır.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
        if process_id != own_process_id && (also_skip_process_id == 0 || process_id != also_skip_process_id) {
            return Some(hwnd);
        }
    }
}

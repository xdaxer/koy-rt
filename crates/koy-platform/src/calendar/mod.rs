//! Karşılığı: `koy/src/Platform/Calendar/`.

mod calendar_sync_service;
mod dpapi_token_protector;
mod loopback_oauth_listener;

pub use calendar_sync_service::*;
pub use dpapi_token_protector::*;
pub use loopback_oauth_listener::*;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

/// `Process.Start(new ProcessStartInfo(url) { UseShellExecute = true })`: varsayılan tarayıcıda açar.
pub(crate) fn open_url(url: &str) {
    let target = HSTRING::from(url);
    // SAFETY: dizeler geçerli ve sıfırla biter; pencere sahibi yok.
    unsafe { ShellExecuteW(None, w!("open"), &target, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
}

//! Windows sesle yazmayı (Win+H) açar; Windows, odaktaki metin kutusuna kullanıcının klavye dilinde yazar
//! (Türkçe, İngilizce…). Konuşma tanıma dili paketi gerekmez; ses Microsoft'un çevrimiçi hizmetine gider.

use std::time::{Duration, Instant};

use koy_core::cancel::{CancellationToken, OperationCanceled};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_H, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT};

use crate::shell::send_win_combo;

pub struct VoiceTyping;

impl VoiceTyping {
    /// Kısayolun (Ctrl+Alt+V) tuşları bırakılana kadar bekler, sonra Win+H gönderir; basılı Ctrl/Alt ile gönderilen Win+H
    /// başka bir kısayola dönüşürdü. Tuşlar en çok `max_wait` içinde bırakılmazsa yine gönderir.
    pub fn open(max_wait: Duration, ct: &CancellationToken) -> Result<(), OperationCanceled> {
        let deadline = Instant::now() + max_wait;
        while any_modifier_down() && Instant::now() < deadline {
            if ct.is_cancellation_requested() {
                return Err(OperationCanceled);
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        ct.throw_if_cancellation_requested()?;
        send_win_combo(VK_H);
        Ok(())
    }
}

fn any_modifier_down() -> bool {
    [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
        .iter()
        // En yüksek bit: tuş şu an basılı.
        // SAFETY: yalnız tuş durumu sorgusu.
        .any(|k| unsafe { GetAsyncKeyState(k.0 as i32) } as u16 & 0x8000 != 0)
}

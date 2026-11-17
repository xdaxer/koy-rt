//! Sesli komut yanıt kartı (tek örnek). Karşılığı: `Features/Notch/VoiceReplyWindow.cs`.

use std::cell::RefCell;

use slint::ComponentHandle;

use koy_platform::shell::WindowStyles;

use crate::VoiceReplyWindow;
use crate::infra::native_window;
use crate::resources::strings::Strings;
use crate::resources::theme_manager::ThemeManager;

const CARD_WIDTH: f64 = 480.0;

thread_local! {
    static CURRENT: RefCell<Option<VoiceReplyWindow>> = const { RefCell::new(None) };
}

/// Yanıtı çentiğin altında gösterir; önceki kart varsa yerine geçer. `notch_center_x` ve `top` DIP'tir.
pub fn show_reply(notch_center_x: f64, top: f64, prompt: &str, info: &str, body: &str, open_file: Box<dyn Fn()>) {
    close_current();
    let Ok(w) = VoiceReplyWindow::new() else { return };
    ThemeManager::install(&w);
    w.set_title_text(Strings::get("VoiceReplyTitle").into());
    w.set_close_text(Strings::get("VoiceReplyClose").into());
    w.set_open_file_text(Strings::get("VoiceReplyOpenFile").into());
    w.set_prompt(prompt.into());
    w.set_info(info.into());
    w.set_body(body.into());
    w.on_close_requested(close_current);
    w.on_open_file(move || open_file());
    w.window().set_position(slint::LogicalPosition::new((notch_center_x - CARD_WIDTH / 2.0) as f32, top as f32));
    if w.show().is_err() {
        return;
    }
    // Odak çalmasın: yerel pencere hazır olunca araç penceresi + etkinleşmez biçimi uygulanır.
    let weak = w.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(30), move || {
        if let Some(w) = weak.upgrade() {
            let hwnd = native_window::hwnd_of(w.window());
            if hwnd != 0 {
                crate::infra::native_window::make_popup(hwnd);
                WindowStyles::apply_tool_window_and_no_activate(hwnd);
            }
        }
    });
    CURRENT.with(|c| *c.borrow_mut() = Some(w));
}

pub fn close_current() {
    let w = CURRENT.with(|c| c.borrow_mut().take());
    if let Some(w) = w {
        let _ = w.hide();
    }
}

/// Kart açıksa öne alır ve odak verir (Esc ile kapanır). Açık değilse false.
pub fn focus_current() -> bool {
    CURRENT.with(|c| {
        let c = c.borrow();
        let Some(w) = c.as_ref() else { return false };
        let hwnd = native_window::hwnd_of(w.window());
        if hwnd != 0 {
            WindowStyles::set_no_activate(hwnd, false);
            native_window::activate(hwnd);
        }
        true
    })
}

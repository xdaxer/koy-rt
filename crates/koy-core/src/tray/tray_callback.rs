use super::tray_notify_data::TrayIcon;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayMouseAction {
    LeftClick,
    LeftDoubleClick,
    RightClick,
}

/// Uygulamanın geri çağrı penceresine gönderilecek tek mesaj.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrayCallbackMessage {
    pub message: u32,
    pub w_param: isize,
    pub l_param: isize,
}

/// Dock'taki tepsi simgesine tıklanınca uygulamaya, Windows görev çubuğunun göndereceği mesajların aynısını üretir.
/// NOTIFYICON_VERSION_4 (4) ve sonrası: wParam = imleç konumu, lParam = (mesaj | kimlik << 16).
/// Eski sürümler: wParam = kimlik, lParam = fare mesajı.
pub struct TrayCallback;

impl TrayCallback {
    pub const WM_CONTEXT_MENU: u32 = 0x007B;
    pub const WM_L_BUTTON_DOWN: u32 = 0x0201;
    pub const WM_L_BUTTON_UP: u32 = 0x0202;
    pub const WM_L_BUTTON_DBL_CLK: u32 = 0x0203;
    pub const WM_R_BUTTON_DOWN: u32 = 0x0204;
    pub const WM_R_BUTTON_UP: u32 = 0x0205;
    pub const NIN_SELECT: u32 = 0x0400;

    pub fn build(icon: &TrayIcon, action: TrayMouseAction, cursor_x: i32, cursor_y: i32) -> Vec<TrayCallbackMessage> {
        if icon.callback_message == 0 || icon.owner_hwnd == 0 {
            return Vec::new();
        }

        let modern = icon.version >= 4;
        let mouse: Vec<u32> = match action {
            TrayMouseAction::LeftClick => {
                if modern {
                    vec![Self::WM_L_BUTTON_DOWN, Self::WM_L_BUTTON_UP, Self::NIN_SELECT]
                } else {
                    vec![Self::WM_L_BUTTON_DOWN, Self::WM_L_BUTTON_UP]
                }
            }
            TrayMouseAction::LeftDoubleClick => vec![Self::WM_L_BUTTON_DBL_CLK, Self::WM_L_BUTTON_UP],
            TrayMouseAction::RightClick => {
                if modern {
                    vec![Self::WM_R_BUTTON_DOWN, Self::WM_R_BUTTON_UP, Self::WM_CONTEXT_MENU]
                } else {
                    vec![Self::WM_R_BUTTON_DOWN, Self::WM_R_BUTTON_UP]
                }
            }
        };

        // `(nint)(int)(((uint)cursorY & 0xFFFF) << 16 | ((uint)cursorX & 0xFFFF))`: iki işaretli 16 bitlik sözcük.
        let position = ((((cursor_y as u32) & 0xFFFF) << 16) | ((cursor_x as u32) & 0xFFFF)) as i32 as isize;
        mouse
            .iter()
            .map(|&m| {
                if modern {
                    TrayCallbackMessage {
                        message: icon.callback_message,
                        w_param: position,
                        l_param: ((m & 0xFFFF) | ((icon.id & 0xFFFF) << 16)) as i32 as isize,
                    }
                } else {
                    TrayCallbackMessage { message: icon.callback_message, w_param: icon.id as isize, l_param: m as isize }
                }
            })
            .collect()
    }
}

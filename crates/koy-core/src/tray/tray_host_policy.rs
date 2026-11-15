/// Koy'un kendi tepsi penceresini (Shell_TrayWnd) ne zaman kuracağına ve hangi simgeleri alacağına dair kararlar.
/// Saf mantık: Win32 çağrısı yok, test edilir (AGENTS.md #11).
pub struct TrayHostPolicy;

impl TrayHostPolicy {
    /// Tepsi barındırma yalnız Windows görev çubuğu gerçekten gizliyken anlamlıdır: dock'un gizlediği tepsiye
    /// ulaşmanın tek yolu budur. Görev çubuğu görünürken (ayar kapalı ya da dock penceresi yok) Windows'un
    /// tepsisine dokunulmaz; aksi halde iki pencere Shell_TrayWnd sahipliği için çakışırdı.
    ///
    /// `hide_windows_taskbar`: "Windows görev çubuğunu gizle" ayarı. `show_tray`: "Sistem tepsisini dock'ta göster"
    /// ayarı. `dock_window_count`: şu an açık dock penceresi sayısı (görev çubuğu yalnız bunlar varken gizlenir).
    pub fn should_host(hide_windows_taskbar: bool, show_tray: bool, dock_window_count: i32) -> bool {
        hide_windows_taskbar && show_tray && dock_window_count > 0
    }

    /// Koy'un kendi bildirim alanı simgesi yalnız dock yokken gösterilir: dock açıkken Koy simgesi zaten dock'un
    /// sağ ucunda durur ve iki simge çıkmasın (ROADMAP 6.5). Dock kapanınca ya da duraklatılınca simge geri gelir.
    ///
    /// `dock_window_count`: şu an açık dock penceresi sayısı (dock yalnız pencereleri varken Koy düğmesini gösterir).
    pub fn should_show_notification_area_icon(dock_window_count: i32) -> bool {
        dock_window_count <= 0
    }

    /// Simgenin sahibi Koy'un kendisiyse (dock kapalıyken menüye erişim için kurduğu tepsi simgesi) dock'un
    /// tepsi penceresinde gösterilmez; kayıt yine de Windows'a iletilir. Sahip penceresi bilinmiyorsa (0) süzülmez.
    pub fn is_own_icon(owner_process_id: u32, own_process_id: u32) -> bool {
        owner_process_id != 0 && owner_process_id == own_process_id
    }
}

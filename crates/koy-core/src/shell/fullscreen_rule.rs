use super::window_rect::WindowRect;

/// Windows'un `SHQueryUserNotificationState` yanıtı, tam ekran kararı için sadeleştirilmiş.
/// Bilgi sistem geneli olup ön plan penceresine aittir; başka ekrandaki pencere için bilinmiyor (Unknown) sayılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ShellNotificationState {
    /// Sorgulanmadı, başarısız oldu ya da güvenilmez (Sessiz saatler, ekran koruyucu): yalnız geometri karar verir.
    #[default]
    Unknown,

    /// Windows bildirim kabul ediyor: tam ekran uygulama yok diyor.
    Normal,

    /// QUNS_BUSY / QUNS_APP: Windows tam ekran bir uygulama çalıştığını söylüyor.
    Busy,

    /// QUNS_RUNNING_D3D_FULL_SCREEN: özel kipte Direct3D oyunu.
    D3dFullScreen,

    /// QUNS_PRESENTATION_MODE: sunum ayarları açık.
    Presentation,
}

pub struct ShellNotificationStates;

impl ShellNotificationStates {
    /// QUERY_USER_NOTIFICATION_STATE sayısal değerini sadeleştirir (Win32 tipi Core'a sızmaz).
    pub fn from_quns(value: i32) -> ShellNotificationState {
        match value {
            2 | 7 => ShellNotificationState::Busy,      // QUNS_BUSY, QUNS_APP
            3 => ShellNotificationState::D3dFullScreen, // QUNS_RUNNING_D3D_FULL_SCREEN
            4 => ShellNotificationState::Presentation,  // QUNS_PRESENTATION_MODE
            5 => ShellNotificationState::Normal,        // QUNS_ACCEPTS_NOTIFICATIONS
            _ => ShellNotificationState::Unknown,       // NOT_PRESENT (1), QUIET_TIME (6) ve bilinmeyenler
        }
    }
}

/// Ön plan penceresinin tam ekran (oyun, video, F11 tarayıcı) sayılıp sayılmayacağına karar verir.
/// Üç işaretin birleşimi: Windows'un tam ekran bildirimi, başlık çubuğunun olmayışı, monitörü tamamen kaplama.
/// Tek başına hiçbiri yetmez (Sessiz saatler "meşgul" diyebilir; özel çerçeveli büyütülmüş pencere başlıksız
/// görünür). Büyütülmüş (IsZoomed) pencere ancak monitöre tam oturuyorsa (F11 tarayıcı, video) sayılır; kenar payıyla
/// monitörü aşan normal büyütülmüş pencere asla. Windows açıkça "tam ekran yok" derse (Normal) geometri geçersizdir.
/// Özel kipli Direct3D oyunu ve sunum modu kesin tam ekrandır.
pub struct FullscreenRule;

impl FullscreenRule {
    /// `state` için C# varsayılanı `ShellNotificationState::Unknown` (`ShellNotificationState::default()`).
    pub fn is_fullscreen(
        window: WindowRect,
        monitor: WindowRect,
        has_caption: bool,
        is_maximized: bool,
        state: ShellNotificationState,
    ) -> bool {
        if matches!(state, ShellNotificationState::D3dFullScreen | ShellNotificationState::Presentation) {
            return true;
        }

        // Başlıklı pencere ekranı kaplasa da tam ekran sayılmaz.
        if has_caption {
            return false;
        }

        // Monitörü tamamen kaplamayan pencere tam ekran değildir.
        if window.left > monitor.left || window.top > monitor.top || window.right < monitor.right || window.bottom < monitor.bottom {
            return false;
        }

        // Büyütülmüş (WS_MAXIMIZE) pencere:
        // Normal büyütülmüş pencereler (Chrome, VS Code) görev çubuğu yokken kenar boyutlandırma
        // payıyla monitörü aşar (-8, -8, 1928, 1088). F11 tarayıcı veya video oynatıcılar WS_MAXIMIZE
        // bayrağını korusa dahi tam olarak monitör sınırlarına oturur; yine de Windows'un da "tam ekran"
        // demesi gerekir, yoksa özel çerçeveli büyütülmüş bir uygulama olabilir.
        if is_maximized {
            let is_exact_monitor_fit = (window.left - monitor.left).abs() <= 1
                && (window.top - monitor.top).abs() <= 1
                && (window.right - monitor.right).abs() <= 1
                && (window.bottom - monitor.bottom).abs() <= 1;

            if !is_exact_monitor_fit {
                return false;
            }
        }

        // Geometri tutuyor. Windows açıkça "bildirim kabul ediyorum" (tam ekran uygulama yok) diyorsa tam ekran değildir
        // (örn. monitör boyutunda bir yer paylaşımı ya da özel çerçeveli büyütülmüş uygulama). Meşgul ya da bilinmiyorsa
        // (başka ekrandaki pencere, Sessiz saatler) geometri yeter.
        state != ShellNotificationState::Normal
    }
}

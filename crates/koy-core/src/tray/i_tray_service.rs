use super::tray_callback::TrayMouseAction;
use super::tray_notify_data::{TrayBalloonNotification, TrayIcon, TrayIconKey};
use crate::event::Event;

/// Uygulamaların sistem tepsisi simgelerini toplayan servis. Dock Windows görev çubuğunu gizlediğinde
/// tepsiye ulaşmanın tek yolu budur. (`IDisposable` → uygulayıcının `Drop`'u.)
pub trait ITrayService: Send + Sync {
    /// Simge listesi değişti. Arka plan iş parçacığından gelebilir.
    fn icons_changed(&self) -> &Event<()>;

    /// Bir tepsi simgesi balon bildirimi gönderdi. Arka plan iş parçacığından gelebilir.
    fn balloon_received(&self) -> &Event<TrayBalloonNotification>;

    /// Gösterilecek simgeler (Koy'un kendi simgeleri hariç), ekleniş sırasıyla.
    fn get_visible_icons(&self) -> Vec<TrayIcon>;

    /// Simgenin PNG görüntüsü; yoksa `None`.
    fn get_icon_image(&self, key: &TrayIconKey) -> Option<Vec<u8>>;

    /// Uygulamaya tepsi tıklamasını iletir ve menüsünü öne getirebilmesine izin verir.
    fn send_mouse(&self, key: &TrayIconKey, action: TrayMouseAction, cursor_x: i32, cursor_y: i32);

    /// Sahibi kapanmış simgeleri temizler.
    fn remove_dead_icons(&self);

    /// Tepsi gerçekten çalışıyor mu. `start` başarısız olduysa ya da kapandıysa false.
    fn is_running(&self) -> bool;

    fn start(&self);

    fn stop(&self);
}

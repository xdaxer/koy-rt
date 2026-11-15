use crate::event::Event;
use crate::time::DateTimeOffset;

/// Windows'un bildirim erişim izni. Core'da WinRT tipi görünmez (AGENTS #11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NotificationAccessStatus {
    /// Kullanıcı henüz sorulmadı; istek Windows'un izin diyaloğunu açar.
    #[default]
    Unspecified,
    Allowed,
    Denied,
    /// Okunamadı (API yok ya da hata): izin yokmuş gibi davranılır.
    Unavailable,
}

/// Aksiyon merkezindeki bir Windows bildirimi. `icon` kodlanmış uygulama logosu ya da `None`;
/// `fallback_icon` logo yoksa/boşsa kullanılacak kabuk simgesi (klasik Win32 uygulamalar için).
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationItem {
    /// Windows'un verdiği artan kimlik; yeni gelen bildirim bununla ayırt edilir.
    pub id: u32,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub time: DateTimeOffset,
    pub icon: Option<Vec<u8>>,
    pub fallback_icon: Option<Vec<u8>>,
}

impl NotificationItem {
    /// C# kurucusunun zorunlu parametreleri; `Icon` / `FallbackIcon` varsayılan `null`.
    pub fn new(id: u32, app_name: impl Into<String>, title: impl Into<String>, body: impl Into<String>, time: DateTimeOffset) -> Self {
        NotificationItem {
            id,
            app_name: app_name.into(),
            title: title.into(),
            body: body.into(),
            time,
            icon: None,
            fallback_icon: None,
        }
    }

    /// `Icon` verilmiş kurucu çağrısı.
    pub fn with_icon(mut self, icon: Vec<u8>) -> Self {
        self.icon = Some(icon);
        self
    }

    /// `FallbackIcon` verilmiş kurucu çağrısı.
    pub fn with_fallback_icon(mut self, fallback_icon: Vec<u8>) -> Self {
        self.fallback_icon = Some(fallback_icon);
        self
    }
}

/// Windows bildirimlerini okuyan kaynak. Paketsiz süreçte olay yok (UserNotificationListener.NotificationChanged
/// 0x80070490 verir), bu yüzden uygulama yoklama ile çalışır: [`INotificationListener::notification_arrived`]
/// arka plan iş parçacığından gelir. İlk okumada var olan bildirimler "yeni" sayılmaz.
/// (`IDisposable` karşılığı uygulayan tipin `Drop`'udur.)
pub trait INotificationListener: Send + Sync {
    fn access_status(&self) -> NotificationAccessStatus;

    /// Durum [`NotificationAccessStatus::Unspecified`] ise Windows izin diyaloğunu açar; yeni durumu döner.
    /// C#: `RequestAccessAsync` (burada bloklayan çağrı; çağıran çalışan iş parçacığında çalıştırır).
    fn request_access(&self) -> NotificationAccessStatus;

    fn notification_arrived(&self) -> &Event<NotificationItem>;

    /// Yoklamayı başlatır. İzin [`NotificationAccessStatus::Allowed`] değilse çağrılmaz.
    fn start(&self);

    /// Yoklamayı durdurur ve zamanlayıcıyı söker (AGENTS #5).
    fn stop(&self);
}

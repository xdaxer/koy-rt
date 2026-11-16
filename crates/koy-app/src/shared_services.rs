//! Dock ve Notch'un ortak kullandığı platform servislerinin tek örneği (WinEvent kancaları ve GSMTC oturumu ikişer kez
//! kurulmasın). Örnek ilk isteyen özellik için oluşur, son kullanan bıraktığında kapatılır; böylece özellikler kapalıyken
//! hiçbir şey belleğe yüklenmez. Yalnız arayüz iş parçacığından kullanılır (`Rc` ile paylaşılır).
//!
//! Rust notu: C# son bırakışta `Dispose` çağırır. Burada açık `dispose` yöntemi olan servislerde o çağrılır; olmayanlarda
//! havuzun `Arc`'ı bırakılır, çağıranlar da kendi kopyalarını `release` ile birlikte bırakmalıdır.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use koy_core::clipboard::IClipboardHistoryService;
use koy_core::event::Event;
use koy_core::network::INetworkService;
use koy_core::notifications::INotificationListener;
use koy_core::privacy::IPrivacyMonitorService;
use koy_core::quick_launch::QuickLaunchService;
use koy_core::settings::{SettingsStore, ValueList};
use koy_core::system_stats::ISystemStatsService;
use koy_core::tasks::{ITaskService, TaskService};
use koy_core::tray::{ITrayService, TrayBalloonNotification, TrayHostLease};
use koy_platform::audio::SpectrumCaptureService;
use koy_platform::bluetooth::BluetoothService;
use koy_platform::clipboard::ClipboardHistoryService;
use koy_platform::media::MediaService;
use koy_platform::network::NetworkService;
use koy_platform::notifications::WinRtNotificationListener;
use koy_platform::power::PowerService;
use koy_platform::privacy::ConsentStorePrivacyService;
use koy_platform::radio::RadioService;
use koy_platform::shell::{ShellActionService, TrayHostService, WindowTracker};
use koy_platform::system_stats::SystemStatsService;
use koy_platform::wireless::WifiService;

use crate::infra::dispatch;

/// Sayaçlı tek örnek.
struct Counted<T> {
    value: Option<Arc<T>>,
    users: u32,
}

impl<T> Default for Counted<T> {
    fn default() -> Self {
        Counted { value: None, users: 0 }
    }
}

impl<T> Counted<T> {
    fn acquire(&mut self, create: impl FnOnce() -> T) -> Arc<T> {
        self.users += 1;
        Arc::clone(self.value.get_or_insert_with(|| Arc::new(create())))
    }

    /// Son kullanan bıraktıysa örneği döndürür (çağıran kapatır).
    fn release(&mut self) -> Option<Arc<T>> {
        if self.users == 0 {
            return None;
        }
        self.users -= 1;
        if self.users == 0 { self.value.take() } else { None }
    }

    fn take_all(&mut self) -> Option<Arc<T>> {
        self.users = 0;
        self.value.take()
    }
}

#[derive(Default)]
struct State {
    window_tracker: Counted<WindowTracker>,
    media: Counted<MediaService>,
    radio: Counted<RadioService>,
    network: Counted<NetworkService>,
    wifi: Counted<WifiService>,
    bluetooth: Counted<BluetoothService>,
    spectrum: Counted<SpectrumCaptureService>,
    tray_host: Option<Arc<TrayHostService>>,
    tray_host_lease: TrayHostLease,
    clipboard: Counted<ClipboardHistoryService>,
    system_stats: Counted<SystemStatsService>,
    privacy: Counted<ConsentStorePrivacyService>,
    power: Counted<PowerService>,
    notifications: Counted<WinRtNotificationListener>,
    tasks: Option<Arc<TaskService>>,
    quick_launch: Option<Rc<RefCell<QuickLaunchService>>>,
    disposed: bool,
}

#[derive(Default)]
pub struct SharedServices {
    state: RefCell<State>,
    /// Tepsi barındırıcısı aktifken gelen balon bildirimleri (arayüz iş parçacığına taşınmış olarak yükselir).
    tray_balloon_received: Arc<Event<TrayBalloonNotification>>,
}

impl SharedServices {
    fn st(&self) -> std::cell::RefMut<'_, State> {
        let st = self.state.borrow_mut();
        assert!(!st.disposed, "Cannot access a disposed object. Object name: 'SharedServices'.");
        st
    }

    pub fn acquire_window_tracker(&self) -> Arc<WindowTracker> {
        self.st().window_tracker.acquire(|| WindowTracker::new(Some(dispatch::post_fn())))
    }

    pub fn release_window_tracker(&self) {
        let released = self.state.borrow_mut().window_tracker.release();
        if let Some(t) = released {
            t.dispose();
        }
    }

    /// Çalışan medya servisi (yoksa `None`); kısayol gibi tek seferlik işler yeni servis kurmasın diye.
    pub fn running_media_service(&self) -> Option<Arc<MediaService>> {
        self.state.borrow().media.value.clone()
    }

    pub fn acquire_media_service(&self) -> Arc<MediaService> {
        let (service, created) = {
            let mut st = self.st();
            let created = st.media.value.is_none();
            (st.media.acquire(MediaService::default), created)
        };
        if created {
            // C#: `_ = StartAsync()`; GSMTC isteği arayüzü bekletmesin diye arka planda başlatılır.
            let s = Arc::clone(&service);
            std::thread::spawn(move || {
                let _com = koy_platform::com::ComApartment::mta();
                koy_core::media::IMediaService::start(&*s);
            });
        }
        service
    }

    pub fn release_media_service(&self) {
        let released = self.state.borrow_mut().media.release();
        if let Some(m) = released {
            koy_core::media::IMediaService::stop(&*m);
        }
    }

    /// Wi-Fi / Bluetooth radyo servisi; her monitördeki çentik aynı örneği paylaşır.
    pub fn acquire_radio_service(&self) -> Arc<RadioService> {
        self.st().radio.acquire(RadioService::default)
    }

    pub fn release_radio_service(&self) {
        let released = self.state.borrow_mut().radio.release();
        if let Some(r) = released {
            r.dispose();
        }
    }

    /// Wi-Fi ağ listesi servisi; yalnız Wi-Fi listesi sayfası açıkken alınır.
    pub fn acquire_wifi_service(&self) -> Arc<WifiService> {
        self.st().wifi.acquire(WifiService::default)
    }

    pub fn release_wifi_service(&self) {
        let released = self.state.borrow_mut().wifi.release();
        if let Some(w) = released {
            w.dispose();
        }
    }

    /// Bluetooth cihaz listesi servisi (DeviceWatcher); son kullanan bırakınca izleyiciler durur.
    pub fn acquire_bluetooth_service(&self) -> Arc<BluetoothService> {
        self.st().bluetooth.acquire(BluetoothService::new)
    }

    pub fn release_bluetooth_service(&self) {
        let released = self.state.borrow_mut().bluetooth.release();
        if let Some(b) = released {
            b.dispose();
        }
    }

    /// Müzik kartındaki çubuklar için ses yakalama (WASAPI loopback); her alımda başlatılır.
    pub fn acquire_spectrum_service(&self) -> Arc<SpectrumCaptureService> {
        let service = self.st().spectrum.acquire(SpectrumCaptureService::default);
        service.start();
        service
    }

    pub fn release_spectrum_service(&self) {
        let released = self.state.borrow_mut().spectrum.release();
        if let Some(s) = released {
            s.dispose();
        }
    }

    /// Ağ durumu servisi (olay tabanlı); son kullanan bıraktığında Windows olay aboneliği kalkar.
    pub fn acquire_network_service(&self) -> Arc<NetworkService> {
        self.st().network.acquire(NetworkService::new)
    }

    pub fn release_network_service(&self) {
        drop(self.state.borrow_mut().network.release());
    }

    /// Pil ve güç servisi; Notch ve Dock aynı örneği paylaşır.
    pub fn acquire_power_service(&self) -> Arc<PowerService> {
        self.st().power.acquire(PowerService::new)
    }

    pub fn release_power_service(&self) {
        drop(self.state.borrow_mut().power.release());
    }

    /// Tepsi barındırıcısı balon olayları.
    pub fn tray_balloon_received(&self) -> &Event<TrayBalloonNotification> {
        &self.tray_balloon_received
    }

    /// Koy'un Shell_TrayWnd penceresi. Monitör başına ayrı pencere kurulmaz: bütün dock'lar aynı örneği paylaşır.
    /// Pencere kurulamazsa `None` döner (sayılmaz, bırakmaya gerek yok).
    pub fn acquire_tray_host(&self) -> Option<Arc<TrayHostService>> {
        let mut st = self.st();
        // Pencere beklenmedik kapandıysa (request_close sonrası) yenisi kurulur.
        if st.tray_host.as_ref().is_some_and(|h| !h.is_running())
            && let Some(old) = st.tray_host.take()
        {
            old.balloon_received().clear();
            old.dispose();
        }
        if st.tray_host.is_none() {
            let host = Arc::new(TrayHostService::default());
            host.start();
            if !host.is_running() {
                host.dispose();
                return None;
            }
            let relay = Arc::clone(&self.tray_balloon_received);
            host.balloon_received().subscribe(move |b: &TrayBalloonNotification| {
                let relay = Arc::clone(&relay);
                let b = b.clone();
                dispatch::post(move || relay.invoke(&b));
            });
            st.tray_host = Some(host);
        }
        st.tray_host_lease.acquire();
        st.tray_host.clone()
    }

    pub fn release_tray_host(&self) {
        // Önce alan boşaltılır, sonra kapatılır: kapatma beklerken bir olay yeniden acquire çağırabilir.
        let host = {
            let mut st = self.state.borrow_mut();
            if !st.tray_host_lease.release() {
                return;
            }
            st.tray_host.take()
        };
        if let Some(h) = host {
            h.balloon_received().clear();
            h.dispose();
        }
    }

    /// Pano geçmişi. Çağıran önce ayara bakmalı: kapalıysa hiç çağrılmaz. Servis başlatılmış döner.
    pub fn acquire_clipboard_history(&self) -> Arc<ClipboardHistoryService> {
        let (service, created) = {
            let mut st = self.st();
            let created = st.clipboard.value.is_none();
            (st.clipboard.acquire(|| ClipboardHistoryService::new(None)), created)
        };
        if created {
            service.start();
        }
        service
    }

    pub fn release_clipboard_history(&self) {
        let released = self.state.borrow_mut().clipboard.release();
        if let Some(c) = released {
            c.dispose();
        }
    }

    /// Windows bildirim dinleyicisi. Başlatılmamış döner: çağıran izni denetler, yalnız `Allowed` ise `start` der.
    pub fn acquire_notification_listener(&self) -> Arc<WinRtNotificationListener> {
        self.st().notifications.acquire(WinRtNotificationListener::default)
    }

    pub fn release_notification_listener(&self) {
        let released = self.state.borrow_mut().notifications.release();
        if let Some(n) = released {
            n.dispose();
        }
    }

    /// Sistem verisi örnekleyicisi; başlatılmış döner. `network` ağ adı içindir; ilk alan verir.
    pub fn acquire_system_stats(&self, network: Option<Arc<dyn INetworkService>>) -> Arc<SystemStatsService> {
        let (service, created) = {
            let mut st = self.st();
            let created = st.system_stats.value.is_none();
            let s = st.system_stats.acquire(move || {
                SystemStatsService::new(network.map(|n| Box::new(move || n.current().name) as koy_platform::system_stats::NetworkNameFn))
            });
            (s, created)
        };
        if created {
            service.start();
        }
        service
    }

    pub fn release_system_stats(&self) {
        let released = self.state.borrow_mut().system_stats.release();
        if let Some(s) = released {
            s.dispose();
        }
    }

    /// Kamera ve mikrofon izleyicisi; başlatılmış döner.
    pub fn acquire_privacy_monitor(&self) -> Arc<ConsentStorePrivacyService> {
        let (service, created) = {
            let mut st = self.st();
            let created = st.privacy.value.is_none();
            (st.privacy.acquire(ConsentStorePrivacyService::default), created)
        };
        if created {
            service.start();
        }
        service
    }

    pub fn release_privacy_monitor(&self) {
        let released = self.state.borrow_mut().privacy.release();
        if let Some(p) = released {
            p.dispose();
        }
    }

    /// Odak görev listesi: tek örnek (tasks.json), ilk istendiğinde diskten okunur.
    pub fn tasks(&self) -> Arc<dyn ITaskService> {
        let mut st = self.st();
        Arc::clone(st.tasks.get_or_insert_with(|| Arc::new(TaskService::new(None, None, None)))) as Arc<dyn ITaskService>
    }

    /// Hızlı başlat listesi: tek örnek (arayüz iş parçacığında). Değişiklikler settings.json'a (QuickLaunch.Items) yazılır.
    pub fn quick_launch(&self) -> Rc<RefCell<QuickLaunchService>> {
        let mut st = self.st();
        let service = st.quick_launch.get_or_insert_with(|| {
            let store = SettingsStore::new(SettingsStore::default_path());
            let initial = store.load().quick_launch.items.clone();
            Rc::new(RefCell::new(QuickLaunchService::new(
                initial.iter().cloned().collect::<Vec<String>>(),
                Box::new(move |items: &[String]| {
                    let store = SettingsStore::new(SettingsStore::default_path());
                    let mut settings = store.load();
                    settings.quick_launch.items = ValueList::from(items.to_vec());
                    let _ = store.save(&settings);
                }),
                Arc::new(ShellActionService),
                None,
            )))
        });
        Rc::clone(service)
    }

    /// Tepsi penceresini beklemeden kapatma isteği (çökme işleyicileri). Örnek sonra normal yoldan bırakılır.
    pub fn request_tray_host_close(&self) {
        if let Ok(st) = self.state.try_borrow()
            && let Some(h) = st.tray_host.as_ref()
        {
            h.request_close();
        }
    }

    pub fn dispose(&self) {
        let mut st = self.state.borrow_mut();
        if st.disposed {
            return;
        }
        st.disposed = true;
        if let Some(t) = st.window_tracker.take_all() {
            t.dispose();
        }
        if let Some(m) = st.media.take_all() {
            koy_core::media::IMediaService::stop(&*m);
        }
        if let Some(r) = st.radio.take_all() {
            r.dispose();
        }
        drop(st.network.take_all());
        if let Some(w) = st.wifi.take_all() {
            w.dispose();
        }
        if let Some(b) = st.bluetooth.take_all() {
            b.dispose();
        }
        if let Some(s) = st.spectrum.take_all() {
            s.dispose();
        }
        if let Some(h) = st.tray_host.take() {
            h.balloon_received().clear();
            h.dispose();
        }
        st.tray_host_lease.reset();
        if let Some(c) = st.clipboard.take_all() {
            c.dispose();
        }
        if let Some(n) = st.notifications.take_all() {
            n.dispose();
        }
        if let Some(s) = st.system_stats.take_all() {
            s.dispose();
        }
        if let Some(p) = st.privacy.take_all() {
            p.dispose();
        }
        drop(st.power.take_all());
    }
}

impl Drop for SharedServices {
    fn drop(&mut self) {
        self.dispose();
    }
}

// Tür denetimi: havuzdaki servisler core sözleşmelerini uygular.
#[allow(dead_code)]
fn _contracts(c: &ClipboardHistoryService, n: &WinRtNotificationListener, p: &ConsentStorePrivacyService, s: &SystemStatsService) {
    let _: &dyn IClipboardHistoryService = c;
    let _: &dyn INotificationListener = n;
    let _: &dyn IPrivacyMonitorService = p;
    let _: &dyn ISystemStatsService = s;
}

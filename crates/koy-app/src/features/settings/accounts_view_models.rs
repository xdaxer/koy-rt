//! Karşılıkları: `koy/src/App/Features/Settings/WeatherLocationViewModel.cs` ve `CalendarAccountsViewModel.cs`.
//!
//! Hava durumu: "konumu kullan" anahtarı ve yedek olarak şehir arama. Kararlar `WeatherLocationPolicy`'de (Core); ağ ve
//! konum servisleri yalnız kullanıcı bir şey yapınca çağrılır (sayfa açılınca istek atılmaz).
//! Takvim hesapları: Google ve Outlook salt okunur OAuth 2.0 PKCE bağlantısı. Belirteçler DPAPI ile şifreli saklanır,
//! etkinlikler yerel önbellekte. Uzun işler (ağ, tarayıcı onayı) arka plan iş parçacığında, sonuç arayüz turunda yazılır.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use chrono::{Duration, Utc};
use koy_core::calendar::{CalendarProvider, CalendarSettings, CalendarSettingsStore, ICalendarSyncService};
use koy_core::cancel::CancellationToken;
use koy_core::weather::{GeoPlace, IGeocodingService, ILocationService, LocationStatus, WeatherLocationPolicy, WeatherLocationSource};
use koy_core::settings::AppSettings;
use koy_platform::calendar::CalendarSyncService;
use koy_platform::weather::{GeocodingService, LocationService};

use crate::features::calendar::shared_events::{built_in_clients, resolve_client};
use crate::infra::observable::Notifier;
use crate::infra::{dispatch, ui_ref::UiRef};
use crate::resources::strings::Strings;

use super::settings_view_model::SettingsViewModel;

// ---------------------------------------------------------------- hava durumu konumu

#[derive(Default)]
struct WeatherState {
    use_location: bool,
    query: String,
    is_busy: bool,
    status_text: String,
    show_location_settings_link: bool,
    has_searched: bool,
    has_location: bool,
    results: Vec<GeoPlace>,
}

pub struct WeatherLocationViewModel {
    vm: Rc<SettingsViewModel>,
    st: RefCell<WeatherState>,
    pub changed: Notifier,
}

fn describe_status(s: &AppSettings) -> String {
    if !WeatherLocationPolicy::has_location(s) {
        return Strings::get("WeatherStatusNone");
    }
    if s.weather.location_source == WeatherLocationSource::Device {
        Strings::get("WeatherStatusDevice")
    } else {
        Strings::format("WeatherStatusCity", &[&s.weather.city])
    }
}

/// (anahtar, sorgu, meşgul, durum, ayar bağlantısı, sonuç yok, konum var, sonuç adları)
pub type WeatherView = (bool, String, bool, String, bool, bool, bool, Vec<String>);

impl WeatherLocationViewModel {
    pub fn new(vm: Rc<SettingsViewModel>) -> Rc<Self> {
        let me = Rc::new(WeatherLocationViewModel { vm, st: RefCell::new(WeatherState::default()), changed: Notifier::default() });
        me.load();
        me
    }

    pub fn load(&self) {
        let s = self.vm.store().load();
        {
            let mut st = self.st.borrow_mut();
            st.use_location = s.weather.use_location;
            st.has_location = WeatherLocationPolicy::has_location(&s);
            st.status_text = describe_status(&s);
            st.show_location_settings_link = false;
        }
        self.changed.notify();
    }

    pub fn view(&self) -> WeatherView {
        let st = self.st.borrow();
        (
            st.use_location,
            st.query.clone(),
            st.is_busy,
            st.status_text.clone(),
            st.show_location_settings_link,
            st.has_searched && st.results.is_empty() && !st.is_busy,
            st.has_location,
            st.results.iter().map(GeoPlace::display_name).collect(),
        )
    }

    pub fn set_query(&self, q: &str) {
        self.st.borrow_mut().query = q.to_string();
    }

    pub fn set_use_location(self: &Rc<Self>, value: bool) {
        let updated = WeatherLocationPolicy::set_use_location(&self.vm.store().load(), value);
        {
            let mut st = self.st.borrow_mut();
            st.use_location = value;
            st.has_location = WeatherLocationPolicy::has_location(&updated);
            st.show_location_settings_link = false;
            st.status_text = describe_status(&updated);
        }
        // Çentik koordinatı ayarı kurulurken okur; değişince yeni ayarla yeniden kurulur.
        self.vm.commit_external("Weather.Location", updated);
        self.changed.notify();
        if value {
            self.fetch_device_location();
        }
    }

    // İzin sorusu (Windows tarafında çıkarsa) ve tek seferlik düşük doğruluklu sorgu; kullanıcı anahtarı açınca.
    fn fetch_device_location(self: &Rc<Self>) {
        {
            let mut st = self.st.borrow_mut();
            st.is_busy = true;
            st.status_text = Strings::get("WeatherStatusWorking");
        }
        self.changed.notify();
        let ui = UiRef::new(self);
        std::thread::spawn(move || {
            let result = LocationService.get_location(&CancellationToken::none()).ok();
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                me.st.borrow_mut().is_busy = false;
                let current = me.vm.store().load();
                // Bu arada anahtar kapatıldıysa sonuç yazılmaz.
                if !current.weather.use_location {
                    me.changed.notify();
                    return;
                }
                let Some(result) = result else {
                    me.st.borrow_mut().status_text = Strings::get("WeatherStatusFailed");
                    me.changed.notify();
                    return;
                };
                let updated = WeatherLocationPolicy::apply_device_location(&current, &result, Utc::now().fixed_offset());
                {
                    let mut st = me.st.borrow_mut();
                    st.has_location = WeatherLocationPolicy::has_location(&updated);
                    st.show_location_settings_link = result.status == LocationStatus::Denied;
                    st.status_text = match result.status {
                        LocationStatus::Success => describe_status(&updated),
                        LocationStatus::Denied => Strings::get("WeatherStatusDenied"),
                        _ => Strings::get("WeatherStatusFailed"),
                    };
                }
                if result.is_success() {
                    me.vm.commit_external("Weather.Location", updated);
                }
                me.changed.notify();
            });
        });
    }

    pub fn search(self: &Rc<Self>) {
        if self.st.borrow().is_busy {
            return;
        }
        let query = {
            let mut st = self.st.borrow_mut();
            st.is_busy = true;
            st.has_searched = false;
            st.results.clear();
            st.query.clone()
        };
        self.changed.notify();
        let language = self.vm.store().load().general.language;
        let ui = UiRef::new(self);
        std::thread::spawn(move || {
            let places = GeocodingService::default().search(&query, &language, &CancellationToken::none()).unwrap_or_default();
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                {
                    let mut st = me.st.borrow_mut();
                    st.results = places;
                    st.has_searched = true;
                    st.is_busy = false;
                }
                me.changed.notify();
            });
        });
    }

    pub fn select_place(&self, index: usize) {
        let Some(place) = self.st.borrow().results.get(index).cloned() else { return };
        let updated = WeatherLocationPolicy::apply_city(&self.vm.store().load(), &place, Utc::now().fixed_offset());
        {
            let mut st = self.st.borrow_mut();
            st.use_location = false;
            st.has_location = true;
            st.show_location_settings_link = false;
            st.status_text = describe_status(&updated);
            st.results.clear();
            st.has_searched = false;
            st.query.clear();
        }
        self.vm.commit_external("Weather.Location", updated);
        self.changed.notify();
    }

    pub fn clear_location(&self) {
        let updated = WeatherLocationPolicy::cleared(&WeatherLocationPolicy::set_use_location(&self.vm.store().load(), false));
        {
            let mut st = self.st.borrow_mut();
            st.use_location = false;
            st.has_location = false;
            st.show_location_settings_link = false;
            st.status_text = describe_status(&updated);
        }
        self.vm.commit_external("Weather.Location", updated);
        self.changed.notify();
    }

    pub fn open_location_settings(&self) {
        koy_platform::wireless::SettingsUri::open("ms-settings:privacy-location");
    }
}

// ---------------------------------------------------------------- takvim hesapları

#[derive(Default, Clone)]
pub struct CalendarView {
    pub google_client_id: String,
    pub google_client_secret: String,
    pub show_google_client_fields: bool,
    pub outlook_client_id: String,
    pub is_google_connected: bool,
    pub is_outlook_connected: bool,
    pub is_outlook_available: bool,
    pub is_google_busy: bool,
    pub is_outlook_busy: bool,
    pub google_status_text: String,
    pub outlook_status_text: String,
    pub status_message: String,
}

pub struct CalendarAccountsViewModel {
    store: CalendarSettingsStore,
    sync: Arc<CalendarSyncService>,
    st: RefCell<CalendarView>,
    loading: Cell<bool>,
    pub changed: Notifier,
    /// Hesap bağlandı / kesildi / eşitlendi: çentiğin takvimi önbellekten yeniden okur.
    pub external_changed: Notifier,
}

impl CalendarAccountsViewModel {
    pub fn new() -> Rc<Self> {
        let me = Rc::new(CalendarAccountsViewModel {
            store: CalendarSettingsStore::new(None),
            sync: Arc::new(CalendarSyncService::new(None, Some(koy_core::calendar::CalendarCacheStore::new(None)))),
            st: RefCell::new(CalendarView::default()),
            loading: Cell::new(false),
            changed: Notifier::default(),
            external_changed: Notifier::default(),
        });
        me.load();
        me
    }

    pub fn view(&self) -> CalendarView {
        self.st.borrow().clone()
    }

    pub fn load(&self) {
        self.loading.set(true);
        let s = self.store.load();
        {
            let mut st = self.st.borrow_mut();
            st.google_client_id = s.google_client_id.clone();
            st.google_client_secret = s.google_client_secret.clone();
            st.show_google_client_fields = built_in_clients().google_client_id.trim().is_empty() || !s.google_client_id.trim().is_empty();
            st.outlook_client_id = s.outlook_client_id.clone();
            st.is_google_connected = s.google_enabled && self.sync.has_token(CalendarProvider::Google);
            st.is_outlook_connected = s.outlook_enabled && self.sync.has_token(CalendarProvider::Outlook);
            st.is_outlook_available = st.is_outlook_connected || resolve_client(CalendarProvider::Outlook, &s).is_configured();
        }
        self.update_status_texts(&s);
        self.loading.set(false);
        self.changed.notify();
    }

    pub fn set_google_client_id(&self, v: &str) {
        let mut s = self.store.load();
        s.google_client_id = v.trim().to_string();
        let _ = self.store.save(&s);
        self.st.borrow_mut().google_client_id = v.to_string();
    }

    pub fn set_google_client_secret(&self, v: &str) {
        let mut s = self.store.load();
        s.google_client_secret = v.trim().to_string();
        let _ = self.store.save(&s);
        self.st.borrow_mut().google_client_secret = v.to_string();
    }

    pub fn set_outlook_client_id(&self, v: &str) {
        let mut s = self.store.load();
        s.outlook_client_id = v.trim().to_string();
        let _ = self.store.save(&s);
        self.st.borrow_mut().outlook_client_id = v.to_string();
    }

    fn set_status(&self, text: String) {
        self.st.borrow_mut().status_message = text;
        self.changed.notify();
    }

    fn set_busy(&self, provider: CalendarProvider, busy: bool) {
        let mut st = self.st.borrow_mut();
        if provider == CalendarProvider::Google {
            st.is_google_busy = busy;
        } else {
            st.is_outlook_busy = busy;
        }
    }

    pub fn sync_now(self: &Rc<Self>) {
        self.set_status(Strings::get("CalSyncing"));
        let current = self.store.load();
        let sync = Arc::clone(&self.sync);
        let ui = UiRef::new(self);
        std::thread::spawn(move || {
            let now = Utc::now().fixed_offset();
            let (from, to) = (now - Duration::days(14), now + Duration::days(60));
            let mut any_error = false;
            let mut from_cache = false;
            for (enabled, provider) in
                [(current.google_enabled, CalendarProvider::Google), (current.outlook_enabled, CalendarProvider::Outlook)]
            {
                if enabled && sync.has_token(provider) {
                    let client = resolve_client(provider, &current);
                    let res = sync.sync(provider, &client, from, to, &CancellationToken::none());
                    if !res.success {
                        any_error = true;
                        from_cache |= res.from_cache;
                    }
                }
            }
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                let mut updated = current;
                updated.last_sync_unix = now.timestamp();
                let _ = me.store.save(&updated);
                me.update_status_texts(&updated);
                me.set_status(Strings::get(if any_error {
                    if from_cache { "CalSyncOfflineCache" } else { "CalSyncFailed" }
                } else {
                    "CalSyncDone"
                }));
                me.external_changed.notify();
            });
        });
    }

    pub fn connect(self: &Rc<Self>, provider: CalendarProvider) {
        let current = self.store.load();
        let client = resolve_client(provider, &current);
        if !client.is_configured() {
            self.set_status(Strings::get("CalClientIdRequired"));
            return;
        }
        self.set_busy(provider, true);
        self.set_status(Strings::get("CalAuthorizing"));
        let sync = Arc::clone(&self.sync);
        let ui = UiRef::new(self);
        std::thread::spawn(move || {
            let result = sync.connect(provider, &client, &CancellationToken::none());
            if result.success {
                let now = Utc::now().fixed_offset();
                sync.sync(provider, &client, now - Duration::days(14), now + Duration::days(60), &CancellationToken::none());
            }
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                me.set_busy(provider, false);
                if !result.success {
                    me.set_status(Strings::format("CalConnectFailed", &[result.error.as_deref().unwrap_or("")]));
                    return;
                }
                let mut updated = me.store.load();
                if provider == CalendarProvider::Google {
                    updated.google_enabled = true;
                    updated.google_account_label = result.account_label.clone();
                    me.st.borrow_mut().is_google_connected = true;
                } else {
                    updated.outlook_enabled = true;
                    updated.outlook_account_label = result.account_label.clone();
                    me.st.borrow_mut().is_outlook_connected = true;
                }
                let _ = me.store.save(&updated);
                me.update_status_texts(&updated);
                me.set_status(Strings::get("CalConnectedDone"));
                me.external_changed.notify();
            });
        });
    }

    pub fn disconnect(&self, provider: CalendarProvider) {
        self.set_busy(provider, true);
        let _ = futures_lite_block_on(self.sync.disconnect_async(provider, CancellationToken::none()));
        let mut updated = self.store.load();
        if provider == CalendarProvider::Google {
            updated.google_enabled = false;
            updated.google_account_label.clear();
            self.st.borrow_mut().is_google_connected = false;
        } else {
            updated.outlook_enabled = false;
            updated.outlook_account_label.clear();
            self.st.borrow_mut().is_outlook_connected = false;
        }
        let _ = self.store.save(&updated);
        self.update_status_texts(&updated);
        self.set_busy(provider, false);
        self.set_status(Strings::get("CalDisconnectedDone"));
        self.external_changed.notify();
    }

    fn update_status_texts(&self, s: &CalendarSettings) {
        let mut st = self.st.borrow_mut();
        let connected = |label: &str| {
            if label.trim().is_empty() {
                Strings::get("CalStatusConnected")
            } else {
                Strings::format("CalStatusConnectedAs", &[label])
            }
        };
        st.google_status_text =
            if st.is_google_connected { connected(&s.google_account_label) } else { Strings::get("CalStatusDisconnected") };
        st.outlook_status_text = if !st.is_outlook_available {
            Strings::get("CalComingSoon")
        } else if st.is_outlook_connected {
            connected(&s.outlook_account_label)
        } else {
            Strings::get("CalStatusDisconnected")
        };
    }
}

/// Bekleme noktası olmayan geleceği (disconnect_async yalnız dosya siler) yerinde tamamlar.
fn futures_lite_block_on<F: std::future::Future>(f: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn noop(_: *const ()) {}
    fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    // SAFETY: işlevsiz uyandırıcı; gelecek hiç beklemez.
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut f = std::pin::pin!(f);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

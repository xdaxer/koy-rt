//! Wi-Fi listesi sayfası. Tarama yalnız sayfa açıkken yapılır: açılışta bir kez, sonra 10 sn'de bir; sayfa kapanınca
//! zamanlayıcı ve servis bırakılır (boşta %0 CPU). Karşılığı: `Features/ControlCenter/WifiPageViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use koy_core::wireless::{IWifiService, WifiConnectStatus, WifiList, WifiListState, WifiNetwork, WifiRowAction, WifiScanResult};

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;

const SCAN_INTERVAL: Duration = Duration::from_secs(10);

/// Wi-Fi listesindeki tek ağ satırı. Şifre yalnız bu alanda, bellekte durur; bağlanma denemesinden sonra silinir.
#[derive(Debug, Clone)]
pub struct WifiRow {
    pub ssid: String,
    pub network: WifiNetwork,
    pub display_name: String,
    pub bars: i32,
    pub is_secured: bool,
    pub is_connected: bool,
    pub status_text: Option<String>,
    pub error_text: Option<String>,
    pub is_busy: bool,
    pub is_password_open: bool,
    pub can_connect: bool,
    /// Satır tıklanabilir mi (bağlı ve gizli ağlarda tıklama yok).
    pub is_clickable: bool,
    password: String,
}

impl WifiRow {
    pub fn new(network: WifiNetwork) -> Self {
        let mut row = WifiRow {
            ssid: network.ssid.clone(),
            network: network.clone(),
            display_name: String::new(),
            bars: 0,
            is_secured: false,
            is_connected: false,
            status_text: None,
            error_text: None,
            is_busy: false,
            is_password_open: false,
            can_connect: false,
            is_clickable: false,
            password: String::new(),
        };
        row.update(network);
        row
    }

    pub fn has_status(&self) -> bool {
        self.is_busy || self.status_text.as_deref().is_some_and(|s| !s.is_empty()) || self.error_text.as_deref().is_some_and(|s| !s.is_empty())
    }

    pub fn display_status(&self) -> String {
        if self.is_busy {
            Strings::get("WifiConnecting")
        } else {
            self.error_text.clone().or_else(|| self.status_text.clone()).unwrap_or_default()
        }
    }

    pub fn update(&mut self, network: WifiNetwork) {
        self.display_name = if network.is_hidden() { Strings::get("WifiHidden") } else { network.ssid.clone() };
        self.bars = WifiList::signal_bars(network.signal_quality);
        self.is_secured = network.is_secured;
        self.is_connected = network.is_connected;
        self.status_text = WifiList::status_key(&network).map(Strings::get);
        self.is_clickable = WifiList::action_for(&network) != WifiRowAction::None;
        if !self.is_clickable {
            self.is_password_open = false;
        }
        self.network = network;
    }

    pub fn set_password(&mut self, password: &str) {
        self.password = password.to_string();
        self.can_connect = !password.is_empty();
    }

    /// Şifreyi verir ve bellekten siler.
    pub fn take_password(&mut self) -> String {
        let v = std::mem::take(&mut self.password);
        self.can_connect = false;
        v
    }

    pub fn clear_password(&mut self) {
        self.password.clear();
        self.can_connect = false;
    }
}

pub type AcquireWifi = Box<dyn Fn() -> Arc<dyn IWifiService>>;

#[derive(Debug, Clone, Default)]
pub struct WifiPageState {
    pub state: WifiListState,
    /// Liste yerine gösterilen açıklama (izin yok, adaptör yok…); liste hazırken boş.
    pub message_text: String,
    pub show_message: bool,
    /// İzin açıklamasının altındaki "Konum ayarlarını aç" bağlantısı.
    pub show_location_link: bool,
    pub show_list: bool,
}

pub struct WifiPageViewModel {
    acquire: AcquireWifi,
    release: Box<dyn Fn()>,
    is_radio_on: Box<dyn Fn() -> bool>,
    timer: RefCell<Option<slint::Timer>>,
    rows: RefCell<HashMap<String, WifiRow>>,
    order: RefCell<Vec<String>>,
    service: RefCell<Option<Arc<dyn IWifiService>>>,
    /// Hangi servis örneğinin taraması sürüyor (sayfa kapanıp açıldıysa eski sonuç atılır).
    generation: Cell<u64>,
    last: RefCell<Option<WifiScanResult>>,
    scanning: Cell<bool>,
    disposed: Cell<bool>,
    page: RefCell<WifiPageState>,
    pub changed: Notifier,
}

impl WifiPageViewModel {
    pub fn new(acquire: AcquireWifi, release: Box<dyn Fn()>, is_radio_on: Box<dyn Fn() -> bool>) -> Rc<Self> {
        let vm = Rc::new(WifiPageViewModel {
            acquire,
            release,
            is_radio_on,
            timer: RefCell::new(None),
            rows: RefCell::new(HashMap::new()),
            order: RefCell::new(Vec::new()),
            service: RefCell::new(None),
            generation: Cell::new(0),
            last: RefCell::new(None),
            scanning: Cell::new(false),
            disposed: Cell::new(false),
            page: RefCell::new(WifiPageState::default()),
            changed: Notifier::default(),
        });
        vm.apply_state();
        vm
    }

    pub fn page(&self) -> std::cell::Ref<'_, WifiPageState> {
        self.page.borrow()
    }

    pub fn rows(&self) -> Vec<WifiRow> {
        let rows = self.rows.borrow();
        self.order.borrow().iter().filter_map(|s| rows.get(s).cloned()).collect()
    }

    /// Sayfa açılınca true: servis alınır, bir kez taranır, zamanlayıcı başlar. false: her şey bırakılır.
    pub fn set_active(self: &Rc<Self>, active: bool) {
        if self.disposed.get() {
            return;
        }
        if active {
            if self.service.borrow().is_some() {
                return;
            }
            *self.service.borrow_mut() = Some((self.acquire)());
            self.generation.set(self.generation.get() + 1);
            let weak = Rc::downgrade(self);
            let timer = slint::Timer::default();
            timer.start(slint::TimerMode::Repeated, SCAN_INTERVAL, move || {
                if let Some(vm) = weak.upgrade() {
                    vm.refresh();
                }
            });
            *self.timer.borrow_mut() = Some(timer);
            self.refresh();
            return;
        }
        *self.timer.borrow_mut() = None;
        if self.service.borrow_mut().take().is_some() {
            (self.release)();
        }
        self.generation.set(self.generation.get() + 1);
        for r in self.rows.borrow_mut().values_mut() {
            r.clear_password();
        }
        self.rows.borrow_mut().clear();
        self.order.borrow_mut().clear();
        *self.last.borrow_mut() = None;
        self.apply_state();
        self.changed.notify();
    }

    /// Radyo açılıp kapanınca liste yeniden değerlendirilir ve açıldıysa taranır.
    pub fn on_radio_changed(self: &Rc<Self>) {
        if self.service.borrow().is_none() {
            return;
        }
        self.apply_state();
        self.changed.notify();
        if (self.is_radio_on)() {
            self.refresh();
        }
    }

    fn refresh(self: &Rc<Self>) {
        let Some(service) = self.service.borrow().clone() else { return };
        if self.scanning.get() || self.disposed.get() {
            return;
        }
        // Bağlanma sürerken satırlar yeniden sıralanmaz.
        if self.rows.borrow().values().any(|r| r.is_busy) {
            return;
        }
        self.scanning.set(true);
        let generation = self.generation.get();
        let me = UiRef::new(self);
        std::thread::spawn(move || {
            let result = service.scan();
            dispatch::post(move || {
                let Some(vm) = me.upgrade() else { return };
                vm.scanning.set(false);
                if generation != vm.generation.get() {
                    return;
                }
                *vm.last.borrow_mut() = Some(result);
                vm.apply_result();
                vm.changed.notify();
            });
        });
    }

    fn apply_result(&self) {
        self.apply_state();
        if self.page.borrow().state != WifiListState::Ready {
            return;
        }
        let Some(last) = self.last.borrow().clone() else { return };
        let networks = WifiList::normalize(&last.networks);
        let mut rows = self.rows.borrow_mut();
        let mut seen = HashSet::new();
        for n in &networks {
            seen.insert(n.ssid.clone());
            match rows.get_mut(&n.ssid) {
                Some(r) => r.update(n.clone()),
                None => {
                    rows.insert(n.ssid.clone(), WifiRow::new(n.clone()));
                }
            }
        }
        rows.retain(|s, r| {
            let keep = seen.contains(s);
            if !keep {
                r.clear_password();
            }
            keep
        });
        *self.order.borrow_mut() = networks.into_iter().map(|n| n.ssid).collect();
    }

    fn apply_state(&self) {
        let state = WifiList::resolve(self.last.borrow().as_ref(), (self.is_radio_on)());
        let mut p = self.page.borrow_mut();
        p.state = state;
        p.show_list = state == WifiListState::Ready;
        p.show_location_link = matches!(state, WifiListState::LocationBlocked | WifiListState::Empty | WifiListState::NoAccess);
        p.message_text = match state {
            WifiListState::Loading => Strings::get("Loading"),
            WifiListState::RadioOff => Strings::get("WifiRadioOff"),
            WifiListState::NoAdapter => Strings::get("WifiNoAdapter"),
            WifiListState::NoAccess => Strings::get("WifiNoAccess"),
            WifiListState::LocationBlocked => Strings::get("WifiLocationBlocked"),
            WifiListState::Empty => Strings::get("WifiEmpty"),
            WifiListState::ScanFailed => Strings::get("WifiScanFailed"),
            _ => String::new(),
        };
        p.show_message = !p.show_list;
    }

    fn with_row<R>(&self, ssid: &str, f: impl FnOnce(&mut WifiRow) -> R) -> Option<R> {
        self.rows.borrow_mut().get_mut(ssid).map(f)
    }

    /// Satıra tıklama: kayıtlı/açık ağa doğrudan bağlanır, güvenli ve kayıtsız ağda satırın içinde şifre kutusunu açar.
    pub fn activate(self: &Rc<Self>, ssid: &str) {
        let Some((busy, network, open)) = self.with_row(ssid, |r| (r.is_busy, r.network.clone(), r.is_password_open)) else { return };
        if busy {
            return;
        }
        match WifiList::action_for(&network) {
            WifiRowAction::ConnectDirect => self.connect(ssid, None),
            WifiRowAction::AskPassword => {
                for r in self.rows.borrow_mut().values_mut() {
                    r.is_password_open = false;
                    r.clear_password();
                }
                self.with_row(ssid, |r| {
                    r.error_text = None;
                    r.is_password_open = !open;
                });
                self.changed.notify();
            }
            _ => {}
        }
    }

    pub fn set_password(&self, ssid: &str, password: &str) {
        self.with_row(ssid, |r| r.set_password(password));
        self.changed.notify();
    }

    /// Şifre kutusundaki "Bağlan".
    pub fn connect_with_password(self: &Rc<Self>, ssid: &str) {
        let Some(Some(password)) = self.with_row(ssid, |r| (!r.is_busy && r.can_connect).then(|| r.take_password())) else { return };
        self.connect(ssid, Some(password));
    }

    fn connect(self: &Rc<Self>, ssid: &str, password: Option<String>) {
        let Some(service) = self.service.borrow().clone() else { return };
        self.with_row(ssid, |r| {
            r.error_text = None;
            r.is_busy = true;
        });
        self.changed.notify();
        let me = UiRef::new(self);
        let ssid = ssid.to_string();
        std::thread::spawn(move || {
            let mut password = password;
            let status = service.connect(&ssid, password.as_deref());
            password.take();
            dispatch::post(move || {
                let Some(vm) = me.upgrade() else { return };
                vm.with_row(&ssid, |r| {
                    r.error_text = WifiList::error_key(status).map(Strings::get);
                    // Yanlış şifre: kutu yeniden açık kalır, kullanıcı düzeltebilsin.
                    r.is_password_open = status == WifiConnectStatus::WrongPassword;
                    r.is_busy = false;
                });
                vm.changed.notify();
                vm.refresh();
            });
        });
    }

    pub fn disconnect(self: &Rc<Self>) {
        let Some(service) = self.service.borrow().clone() else { return };
        service.disconnect();
        // Windows bağlantıyı kestikten sonra durum değişir; kısa bekleyip listeyi tazele.
        let weak = Rc::downgrade(self);
        slint::Timer::single_shot(Duration::from_millis(700), move || {
            if let Some(vm) = weak.upgrade() {
                vm.refresh();
            }
        });
    }

    pub fn open_location_settings(&self) {
        match self.service.borrow().as_ref() {
            Some(s) => s.open_location_settings(),
            None => koy_platform::wireless::SettingsUri::open("ms-settings:privacy-location"),
        }
    }

    pub fn open_settings(&self) {
        match self.service.borrow().as_ref() {
            Some(s) => s.open_settings(),
            None => koy_platform::wireless::SettingsUri::open("ms-settings:network-wifi"),
        }
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.disposed.get() {
            return;
        }
        self.set_active(false);
        self.disposed.set(true);
    }
}

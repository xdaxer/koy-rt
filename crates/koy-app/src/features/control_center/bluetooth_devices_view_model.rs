//! Eşleşmiş Bluetooth cihazları: Ana'daki kulaklık kartı, Kontrol'deki "Bağlı cihazlar" ve Bluetooth listesi aynı
//! veriyi kullanır. Servis yalnız bir sayfa "izliyorken" alınır (`set_viewing`); izleyen kalmayınca DeviceWatcher
//! durur ve listeler boşaltılır. Karşılığı: `Features/ControlCenter/BluetoothDevicesViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use koy_core::bluetooth::{
    BluetoothActionResult, BluetoothDeviceInfo, BluetoothDeviceKind, BluetoothList, BluetoothListState, BluetoothRowAction, IBluetoothService,
};
use koy_core::event::Subscription;
use koy_core::radio::RadioTileStatus;

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::{self, Strings};

/// Bluetooth listesindeki tek cihaz satırı.
#[derive(Debug, Clone)]
pub struct BluetoothDeviceRow {
    pub id: String,
    pub name: String,
    pub kind: BluetoothDeviceKind,
    pub is_connected: bool,
    pub is_audio: bool,
    pub battery_text: Option<String>,
    pub status_text: String,
    /// Ses dışı cihazlarda satırın altındaki küçük yazı (tıklayınca Windows ayarları açılır).
    pub hint_text: Option<String>,
    pub error_text: Option<String>,
    pub is_busy: bool,
    pub action_text: String,
    pub info: BluetoothDeviceInfo,
}

impl BluetoothDeviceRow {
    pub fn new(info: BluetoothDeviceInfo) -> Self {
        let mut row = BluetoothDeviceRow {
            id: info.id.clone(),
            name: String::new(),
            kind: info.kind,
            is_connected: false,
            is_audio: false,
            battery_text: None,
            status_text: String::new(),
            hint_text: None,
            error_text: None,
            is_busy: false,
            action_text: String::new(),
            info: info.clone(),
        };
        row.update(info);
        row
    }

    pub fn has_battery(&self) -> bool {
        self.battery_text.is_some()
    }

    /// Satır Windows Bluetooth ayarlarını açar (ses dışı cihaz).
    pub fn opens_settings(&self) -> bool {
        !self.is_audio
    }

    pub fn update(&mut self, info: BluetoothDeviceInfo) {
        self.name = info.name.clone();
        self.kind = info.kind;
        self.is_connected = info.is_connected;
        self.is_audio = info.is_audio();
        self.battery_text =
            if info.is_connected { BluetoothList::battery_text(info.battery_percent, &strings::culture()) } else { None };
        self.info = info;
        self.refresh();
    }

    // Durum satırı: işlem sürerken "Bağlanıyor…", hata varsa hata, yoksa gerçek bağlantı durumu.
    pub fn refresh(&mut self) {
        let action = BluetoothList::action_for(&self.info);
        self.action_text = match action {
            BluetoothRowAction::Connect => Strings::get("BluetoothConnect"),
            BluetoothRowAction::Disconnect => Strings::get("BluetoothDisconnect"),
            _ => String::new(),
        };
        self.hint_text = (action == BluetoothRowAction::OpenSettings).then(|| Strings::get("BluetoothOpenSettingsHint"));
        self.status_text = if self.is_busy {
            Strings::get(if self.info.is_connected { "BluetoothDisconnecting" } else { "BluetoothConnecting" })
        } else {
            self.error_text.clone().unwrap_or_else(|| Strings::get(BluetoothList::status_key(&self.info)))
        };
    }
}

pub type AcquireBluetooth = Box<dyn Fn() -> Arc<dyn IBluetoothService>>;

pub struct BluetoothDevicesViewModel {
    acquire: AcquireBluetooth,
    release: Box<dyn Fn()>,
    viewers: RefCell<HashSet<String>>,
    rows: RefCell<HashMap<String, BluetoothDeviceRow>>,
    /// Bütün eşleşmiş cihazların kimlikleri (bağlı olanlar üstte).
    order: RefCell<Vec<String>>,
    primary_audio: RefCell<Option<String>>,
    is_ready: Cell<bool>,
    service: RefCell<Option<(Arc<dyn IBluetoothService>, Subscription)>>,
    sync_queued: Rc<Cell<bool>>,
    disposed: Cell<bool>,
    pub changed: Notifier,
}

impl BluetoothDevicesViewModel {
    pub fn new(acquire: AcquireBluetooth, release: Box<dyn Fn()>) -> Rc<Self> {
        Rc::new(BluetoothDevicesViewModel {
            acquire,
            release,
            viewers: RefCell::new(HashSet::new()),
            rows: RefCell::new(HashMap::new()),
            order: RefCell::new(Vec::new()),
            primary_audio: RefCell::new(None),
            is_ready: Cell::new(false),
            service: RefCell::new(None),
            sync_queued: Rc::new(Cell::new(false)),
            disposed: Cell::new(false),
            changed: Notifier::default(),
        })
    }

    /// Bütün eşleşmiş cihazlar (bağlı olanlar üstte).
    pub fn devices(&self) -> Vec<BluetoothDeviceRow> {
        let rows = self.rows.borrow();
        self.order.borrow().iter().filter_map(|id| rows.get(id).cloned()).collect()
    }

    /// Kontrol'deki "Bağlı cihazlar" kartı: yalnız bağlı cihazlar.
    pub fn connected(&self) -> Vec<BluetoothDeviceRow> {
        self.devices().into_iter().filter(|r| r.is_connected).collect()
    }

    /// Ana'daki kulaklık kartı: bağlı ses cihazı; yoksa None.
    pub fn primary_audio(&self) -> Option<BluetoothDeviceRow> {
        let id = self.primary_audio.borrow().clone()?;
        self.rows.borrow().get(&id).cloned()
    }

    pub fn has_primary_audio(&self) -> bool {
        self.primary_audio.borrow().is_some()
    }

    pub fn has_connected(&self) -> bool {
        let rows = self.rows.borrow();
        self.order.borrow().iter().any(|id| rows.get(id).is_some_and(|r| r.is_connected))
    }

    pub fn is_ready(&self) -> bool {
        self.is_ready.get()
    }

    /// Bu sahip (ör. "home", "control") şu an cihaz verisini gösteriyor mu. Hiç sahip kalmayınca servis bırakılır.
    pub fn set_viewing(self: &Rc<Self>, owner: &str, viewing: bool) {
        if self.disposed.get() {
            return;
        }
        let changed = if viewing { self.viewers.borrow_mut().insert(owner.to_string()) } else { self.viewers.borrow_mut().remove(owner) };
        if !changed {
            return;
        }
        if self.viewers.borrow().is_empty() {
            self.stop();
        } else {
            self.start();
        }
    }

    fn start(self: &Rc<Self>) {
        if self.service.borrow().is_some() {
            return;
        }
        let service = (self.acquire)();
        let me = UiRef::new(self);
        let queued = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sub = service.devices_changed().subscribe(move |_| {
            // WinRT iş parçacığından gelir; art arda olaylar tek eşitlemeye indirilir.
            if queued.swap(true, std::sync::atomic::Ordering::AcqRel) {
                return;
            }
            let q = Arc::clone(&queued);
            dispatch::post(move || {
                q.store(false, std::sync::atomic::Ordering::Release);
                if let Some(vm) = me.upgrade() {
                    vm.sync();
                }
            });
        });
        service.set_battery_reads_enabled(true);
        *self.service.borrow_mut() = Some((service, sub));
        self.sync();
    }

    fn stop(&self) {
        let Some((service, sub)) = self.service.borrow_mut().take() else { return };
        service.set_battery_reads_enabled(false);
        service.devices_changed().unsubscribe(sub);
        drop(service);
        (self.release)();
        self.rows.borrow_mut().clear();
        self.order.borrow_mut().clear();
        *self.primary_audio.borrow_mut() = None;
        self.is_ready.set(false);
        self.changed.notify();
    }

    fn sync(&self) {
        if self.disposed.get() {
            return;
        }
        let Some(service) = self.service.borrow().as_ref().map(|(s, _)| Arc::clone(s)) else { return };
        self.is_ready.set(service.is_ready());
        let devices = BluetoothList::normalize(&service.devices());
        {
            // Satır nesneleri korunur (busy / hata durumu kaybolmaz).
            let mut rows = self.rows.borrow_mut();
            let mut seen = HashSet::new();
            for info in &devices {
                seen.insert(info.id.clone());
                match rows.get_mut(&info.id) {
                    Some(row) => row.update(info.clone()),
                    None => {
                        rows.insert(info.id.clone(), BluetoothDeviceRow::new(info.clone()));
                    }
                }
            }
            rows.retain(|id, _| seen.contains(id));
        }
        *self.order.borrow_mut() = devices.iter().map(|d| d.id.clone()).collect();
        *self.primary_audio.borrow_mut() = BluetoothList::primary_audio(&devices).map(|d| d.id.clone());
        self.changed.notify();
    }

    /// Satıra ya da düğmesine basılınca: ses cihazı bağlanır / bağlantısı kesilir, diğerleri için Windows ayarları açılır.
    pub fn activate(self: &Rc<Self>, id: &str) {
        let Some(service) = self.service.borrow().as_ref().map(|(s, _)| Arc::clone(s)) else { return };
        let info = {
            let rows = self.rows.borrow();
            match rows.get(id) {
                Some(r) if !r.is_busy => r.info.clone(),
                _ => return,
            }
        };
        let action = BluetoothList::action_for(&info);
        if action == BluetoothRowAction::OpenSettings {
            service.open_settings();
            return;
        }
        let connecting = action == BluetoothRowAction::Connect;
        self.with_row(id, |r| {
            r.error_text = None;
            r.is_busy = true;
            r.refresh();
        });
        self.changed.notify();
        let me = UiRef::new(self);
        let id = id.to_string();
        std::thread::spawn(move || {
            let result: BluetoothActionResult = if connecting { service.connect(&id) } else { service.disconnect(&id) };
            dispatch::post(move || {
                let Some(vm) = me.upgrade() else { return };
                let key = BluetoothList::error_key(result, connecting);
                vm.with_row(&id, |r| {
                    r.error_text = key.map(Strings::get);
                    r.is_busy = false;
                    r.refresh();
                });
                vm.changed.notify();
            });
        });
    }

    fn with_row(&self, id: &str, f: impl FnOnce(&mut BluetoothDeviceRow)) {
        if let Some(r) = self.rows.borrow_mut().get_mut(id) {
            f(r);
        }
    }

    /// Listenin hangi hâlde gösterileceği (radyo durumu + servis).
    pub fn state_for(&self, radio: RadioTileStatus) -> BluetoothListState {
        BluetoothList::resolve(radio, self.is_ready.get(), self.order.borrow().len() as i32)
    }

    pub fn open_settings(&self) {
        if let Some((s, _)) = self.service.borrow().as_ref() {
            s.open_settings();
            return;
        }
        koy_platform::wireless::SettingsUri::open("ms-settings:bluetooth");
    }

    pub fn dispose(&self) {
        if self.disposed.get() {
            return;
        }
        self.stop();
        self.disposed.set(true);
        self.viewers.borrow_mut().clear();
    }
}

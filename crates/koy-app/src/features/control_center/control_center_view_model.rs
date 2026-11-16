//! Kontrol sekmesi: ses, parlaklık, Wi-Fi / Bluetooth döşemeleri, Rahatsız Etme ve liste sayfaları. Servisler panel
//! ilk açıldığında kurulur (AGENTS.md #5); radyo servisi kapalı çentikteki simgeler için çentik açılırken kurulur ve
//! bütün monitörlerde paylaşılır. Karşılığı: `Features/ControlCenter/ControlCenterViewModel.cs` ve `RadioTileViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use koy_core::audio::{AudioStateChangedEventArgs, IAudioService};
use koy_core::bluetooth::BluetoothListState;
use koy_core::display::IBrightnessService;
use koy_core::event::Subscription;
use koy_core::notch::ControlCenterTiles;
use koy_core::radio::{IRadioService, RadioAccess, RadioKind, RadioState, RadioStateChangedEventArgs, RadioTileAction, RadioTileResolver, RadioTileState, RadioTileStatus};
use koy_core::settings::{SettingsStore, ValueList};
use koy_core::shell::IFocusAssistService;

use super::bluetooth_devices_view_model::BluetoothDevicesViewModel;
use super::wifi_page_view_model::WifiPageViewModel;
use crate::infra::dispatch;
use crate::infra::observable::{Notifier, SharedSettings};
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;

/// Kontrol sekmesinde açık olan sayfa: ana döşemeler ya da bir ağ / cihaz listesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlPage {
    #[default]
    Main,
    WiFi,
    Bluetooth,
}

/// Wi-Fi ya da Bluetooth döşemesinin görünen durumu. Karar `RadioTileResolver`'da verilir.
#[derive(Debug, Clone)]
pub struct RadioTile {
    pub kind: RadioKind,
    pub state: RadioTileState,
    pub is_on: bool,
    pub is_unavailable: bool,
    /// Kapalı çentikteki durum simgesi gösterilsin mi.
    pub is_present: bool,
    pub status_text: String,
    pub tool_tip: Option<String>,
}

impl RadioTile {
    pub fn new(kind: RadioKind) -> Self {
        let mut t = RadioTile {
            kind,
            state: RadioTileResolver::resolve(RadioAccess::Unknown, RadioState::Unknown, false),
            is_on: false,
            is_unavailable: false,
            is_present: false,
            status_text: String::new(),
            tool_tip: None,
        };
        t.apply(t.state);
        t
    }

    pub fn apply(&mut self, state: RadioTileState) {
        self.state = state;
        self.is_on = state.is_on;
        self.is_unavailable = state.is_unavailable();
        self.is_present = state.is_present();
        self.status_text = Strings::get(match state.status {
            RadioTileStatus::On => "On",
            RadioTileStatus::Off => "Off",
            RadioTileStatus::NotFound => "RadioNotFound",
            RadioTileStatus::NoAccess => "RadioNoAccess",
            RadioTileStatus::HardwareOff => "RadioDisabled",
            RadioTileStatus::Failed => "RadioToggleFailed",
            _ => "Loading",
        });
        self.tool_tip = match state.action {
            RadioTileAction::OpenSettings => Some(Strings::get("RadioOpenSettingsHint")),
            RadioTileAction::OpenPrivacySettings => Some(Strings::get("RadioPrivacyHint")),
            _ => None,
        };
    }
}

#[derive(Debug, Clone)]
pub struct ControlState {
    pub page: ControlPage,
    pub wifi: RadioTile,
    pub bluetooth: RadioTile,
    pub tile_order: Vec<String>,
    /// Bluetooth sayfasında / kartında liste yerine gösterilen açıklama.
    pub bluetooth_message: String,
    pub show_bluetooth_message: bool,
    pub show_bluetooth_list: bool,
    /// "Bağlı cihazlar" kartı görünür mü (Bluetooth radyosu var).
    pub show_connected_card: bool,
    pub connected_empty_text: String,
    pub show_connected_empty: bool,
    pub volume: f32,
    pub volume_percent: i32,
    pub is_muted: bool,
    pub brightness: i32,
    pub is_brightness_supported: bool,
    /// Kaynak ilk kez belirlenene kadar satır gizli kalır.
    pub is_brightness_resolved: bool,
    /// Rahatsız Etme döşemesinin alt satırı.
    pub do_not_disturb_status: String,
}

pub struct ControlCenterFactories {
    pub audio: Box<dyn Fn() -> Arc<dyn IAudioService>>,
    pub radio: Box<dyn Fn() -> Arc<dyn IRadioService>>,
    pub radio_release: Box<dyn Fn()>,
    /// Parlaklık servisi (WMI sorgusu): arka planda kurulur.
    pub brightness: Arc<dyn Fn() -> Arc<dyn IBrightnessService> + Send + Sync>,
}

pub struct ControlCenterViewModel {
    factories: ControlCenterFactories,
    focus_assist: Arc<dyn IFocusAssistService>,
    settings: SharedSettings,
    pub wifi_page: Rc<WifiPageViewModel>,
    pub bluetooth_devices: Rc<BluetoothDevicesViewModel>,
    audio: RefCell<Option<(Arc<dyn IAudioService>, Subscription)>>,
    radio: RefCell<Option<(Arc<dyn IRadioService>, Subscription)>>,
    brightness: RefCell<Option<(Arc<dyn IBrightnessService>, Subscription, Subscription)>>,
    brightness_started: Cell<bool>,
    dnd_sub: Cell<Option<Subscription>>,
    // Son açma/kapama denemesi reddedildiyse döşeme bunu yazar; radyo durumu değişince silinir.
    toggle_failed: RefCell<HashMap<RadioKind, bool>>,
    state: RefCell<ControlState>,
    disposed: Cell<bool>,
    bt_sub: Cell<Option<u64>>,
    pub changed: Notifier,
}

impl ControlCenterViewModel {
    pub fn new(
        factories: ControlCenterFactories,
        focus_assist: Arc<dyn IFocusAssistService>,
        settings: SharedSettings,
        wifi_page: impl FnOnce(Box<dyn Fn() -> bool>) -> Rc<WifiPageViewModel>,
        bluetooth_devices: Rc<BluetoothDevicesViewModel>,
    ) -> Rc<Self> {
        let tile_order = ControlCenterTiles::resolve(Some(settings.borrow().notch.control_center_layout.as_slice()));
        // Wi-Fi sayfası radyonun açık olup olmadığını döşemeden okur.
        let radio_on: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let ro = Rc::clone(&radio_on);
        let wifi_page = wifi_page(Box::new(move || ro.get()));
        let vm = Rc::new(ControlCenterViewModel {
            factories,
            focus_assist,
            settings,
            wifi_page,
            bluetooth_devices,
            audio: RefCell::new(None),
            radio: RefCell::new(None),
            brightness: RefCell::new(None),
            brightness_started: Cell::new(false),
            dnd_sub: Cell::new(None),
            toggle_failed: RefCell::new(HashMap::from([(RadioKind::WiFi, false), (RadioKind::Bluetooth, false)])),
            state: RefCell::new(ControlState {
                page: ControlPage::Main,
                wifi: RadioTile::new(RadioKind::WiFi),
                bluetooth: RadioTile::new(RadioKind::Bluetooth),
                tile_order,
                bluetooth_message: String::new(),
                show_bluetooth_message: false,
                show_bluetooth_list: false,
                show_connected_card: false,
                connected_empty_text: String::new(),
                show_connected_empty: false,
                volume: 0.0,
                volume_percent: 0,
                is_muted: false,
                brightness: 0,
                is_brightness_supported: false,
                is_brightness_resolved: false,
                do_not_disturb_status: Strings::get("OpenInSettings"),
            }),
            disposed: Cell::new(false),
            bt_sub: Cell::new(None),
            changed: Notifier::default(),
        });
        // Döşeme durumu radyo anahtarını besler (Wi-Fi sayfası okur).
        let weak = Rc::downgrade(&vm);
        let ro2 = Rc::clone(&radio_on);
        vm.changed.subscribe(move || {
            if let Some(vm) = weak.upgrade() {
                ro2.set(vm.state.borrow().wifi.is_on);
            }
        });
        let weak = Rc::downgrade(&vm);
        let id = vm.bluetooth_devices.changed.subscribe(move || {
            if let Some(vm) = weak.upgrade() {
                vm.update_bluetooth_view();
                vm.changed.notify();
            }
        });
        vm.bt_sub.set(Some(id));
        vm.update_bluetooth_view();
        vm
    }

    pub fn state(&self) -> std::cell::Ref<'_, ControlState> {
        self.state.borrow()
    }

    /// Sürükleme bitince yeni sırayı ayara yazar. Disk hatası sırayı bozmaz: bu oturumda geçerli kalır.
    pub fn save_tile_order(&self, order: Vec<String>) {
        if ControlCenterTiles::same_order(&order, &self.state.borrow().tile_order) {
            return;
        }
        self.state.borrow_mut().tile_order = order.clone();
        self.settings.borrow_mut().notch.control_center_layout = ValueList::from(order.clone());
        let store = SettingsStore::new(SettingsStore::default_path());
        let mut current = store.load();
        current.notch.control_center_layout = ValueList::from(order);
        if let Err(e) = store.save(&current) {
            koy_core::diagnostics::CrashLog::write(
                "Kontrol merkezi sırası kaydedilemedi",
                &koy_core::diagnostics::CrashError::exception("IOException", &e.to_string(), ""),
            );
        }
        self.changed.notify();
    }

    // Bluetooth sayfası ve kart, radyo döşemesinin durumundan ve cihaz listesinden beslenir.
    fn update_bluetooth_view(&self) {
        let radio = self.state.borrow().bluetooth.state.status;
        let bt_state = self.bluetooth_devices.state_for(radio);
        let has_connected = self.bluetooth_devices.has_connected();
        let mut st = self.state.borrow_mut();
        st.show_bluetooth_list = bt_state == BluetoothListState::Ready;
        st.show_bluetooth_message = !st.show_bluetooth_list;
        st.bluetooth_message = Strings::get(match bt_state {
            BluetoothListState::RadioOff => "BluetoothRadioOff",
            BluetoothListState::NotFound => "BluetoothNotFound",
            BluetoothListState::NoAccess => "RadioNoAccess",
            BluetoothListState::Empty => "BluetoothEmpty",
            _ => "Loading",
        });
        st.show_connected_card = !matches!(radio, RadioTileStatus::NotFound | RadioTileStatus::NoAccess | RadioTileStatus::Loading);
        st.show_connected_empty = !has_connected;
        st.connected_empty_text =
            Strings::get(if bt_state == BluetoothListState::RadioOff { "BluetoothRadioOff" } else { "BluetoothNoConnected" });
    }

    /// Kapalı çentikteki Wi-Fi / Bluetooth simgeleri için radyoları panel açılmadan izlemeye başlar.
    pub fn start_radio_status(self: &Rc<Self>) {
        if self.disposed.get() || self.radio.borrow().is_some() {
            return;
        }
        let created = self.ensure_radio_service();
        // Paylaşımlı servis başka bir monitörde zaten tarandıysa yeniden taranmaz.
        if created
            && let Some(r) = self.radio_service()
            && r.wifi_state() == RadioState::Unknown
            && r.bluetooth_state() == RadioState::Unknown
        {
            std::thread::spawn(move || r.refresh());
        }
    }

    fn radio_service(&self) -> Option<Arc<dyn IRadioService>> {
        self.radio.borrow().as_ref().map(|(r, _)| Arc::clone(r))
    }

    fn ensure_radio_service(self: &Rc<Self>) -> bool {
        if self.radio.borrow().is_some() {
            return false;
        }
        let service = (self.factories.radio)();
        let me = UiRef::new(self);
        let sub = service.state_changed().subscribe(move |e: &RadioStateChangedEventArgs| {
            let e = *e;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    vm.on_radio_state_changed(e);
                }
            });
        });
        *self.radio.borrow_mut() = Some((service, sub));
        self.update_radio_tile(RadioKind::WiFi);
        self.update_radio_tile(RadioKind::Bluetooth);
        self.changed.notify();
        true
    }

    fn refresh_do_not_disturb(&self) {
        self.state.borrow_mut().do_not_disturb_status =
            if self.focus_assist.is_do_not_disturb() { Strings::get("FocusOn") } else { Strings::get("OpenInSettings") };
    }

    fn set_do_not_disturb_active(self: &Rc<Self>, active: bool) {
        if active {
            // Sekme açılınca tek okuma; Odaklanma oturumu için olay da dinlenir (zamanlayıcı yok).
            self.refresh_do_not_disturb();
            let watching = {
                let s = self.dnd_sub.take();
                let w = s.is_some();
                self.dnd_sub.set(s);
                w
            };
            if !watching {
                let me = UiRef::new(self);
                let sub = self.focus_assist.state_changed().subscribe(move |_| {
                    dispatch::post(move || {
                        if let Some(vm) = me.upgrade()
                            && !vm.disposed.get()
                        {
                            vm.refresh_do_not_disturb();
                            vm.changed.notify();
                        }
                    });
                });
                self.dnd_sub.set(Some(sub));
            }
        } else if let Some(sub) = self.dnd_sub.take() {
            self.focus_assist.state_changed().unsubscribe(sub);
        }
    }

    pub fn set_active(self: &Rc<Self>, active: bool) {
        if self.disposed.get() {
            return;
        }
        self.set_do_not_disturb_active(active);
        // Sekme değişince ya da çentik kapanınca liste sayfaları kapanır.
        self.bluetooth_devices.set_viewing("control", active);
        if !active {
            self.close_page();
            return;
        }
        self.ensure_radio_service();
        // Her açılışta yeniden taranır (USB Wi-Fi, sonradan verilen izin). Tek seferlik sorgu.
        if let Some(r) = self.radio_service() {
            std::thread::spawn(move || r.refresh());
        }
        self.ensure_audio_service();
        if self.brightness_started.get() {
            // Ekran düzeni çentik kapalıyken değişmiş olabilir.
            self.refresh_brightness();
        } else {
            self.brightness_started.set(true);
            self.initialize_brightness();
        }
        self.changed.notify();
    }

    fn ensure_audio_service(self: &Rc<Self>) {
        if self.audio.borrow().is_some() || self.disposed.get() {
            return;
        }
        let service = (self.factories.audio)();
        let me = UiRef::new(self);
        let sub = service.state_changed().subscribe(move |e: &AudioStateChangedEventArgs| {
            let e = *e;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    {
                        let mut st = vm.state.borrow_mut();
                        st.volume = e.volume;
                        st.volume_percent = (e.volume as f64 * 100.0).round() as i32;
                        st.is_muted = e.is_muted;
                    }
                    vm.changed.notify();
                }
            });
        });
        {
            let mut st = self.state.borrow_mut();
            st.volume = service.volume();
            st.volume_percent = (st.volume as f64 * 100.0).round() as i32;
            st.is_muted = service.is_muted();
        }
        *self.audio.borrow_mut() = Some((service, sub));
    }

    /// Çentik tekerlek eylemi "Ses" iken: panel açılmadan sistem sesini yüzde olarak artırır ya da azaltır.
    pub fn adjust_volume(self: &Rc<Self>, delta_percent: i32) {
        self.ensure_audio_service();
        let Some(current) = self.audio.borrow().as_ref().map(|(a, _)| (a.volume() as f64 * 100.0).round() as i32) else { return };
        self.on_volume_slider_changed((current + delta_percent).clamp(0, 100) as f64);
    }

    fn initialize_brightness(self: &Rc<Self>) {
        // WMI sorgusu arayüz iş parçacığını bekletmesin.
        let factory = Arc::clone(&self.factories.brightness);
        let me = UiRef::new(self);
        std::thread::spawn(move || {
            let service = factory();
            dispatch::post(move || {
                let Some(vm) = me.upgrade() else { return };
                if vm.disposed.get() {
                    return;
                }
                let s1 = service.brightness_changed().subscribe(move |b: &i32| {
                    let b = *b;
                    dispatch::post(move || {
                        if let Some(vm) = me.upgrade() {
                            vm.state.borrow_mut().brightness = b;
                            vm.changed.notify();
                        }
                    });
                });
                let s2 = service.availability_changed().subscribe(move |_| {
                    dispatch::post(move || {
                        if let Some(vm) = me.upgrade()
                            && !vm.disposed.get()
                        {
                            vm.apply_brightness_state();
                            vm.changed.notify();
                        }
                    });
                });
                *vm.brightness.borrow_mut() = Some((service, s1, s2));
                vm.apply_brightness_state();
                vm.changed.notify();
            });
        });
    }

    fn apply_brightness_state(&self) {
        let Some(service) = self.brightness.borrow().as_ref().map(|(s, _, _)| Arc::clone(s)) else { return };
        let mut st = self.state.borrow_mut();
        st.is_brightness_supported = service.is_supported();
        st.brightness = service.brightness();
        st.is_brightness_resolved = true;
    }

    /// Parlaklık kaynağını arka planda yeniden belirler (ekran eklendi/çıkarıldı, çentik başka ekrana geçti).
    pub fn refresh_brightness(&self) {
        if self.disposed.get() {
            return;
        }
        if let Some(s) = self.brightness.borrow().as_ref().map(|(s, _, _)| Arc::clone(s)) {
            std::thread::spawn(move || s.refresh());
        }
    }

    fn on_radio_state_changed(self: &Rc<Self>, e: RadioStateChangedEventArgs) {
        let was_on = self.tile(e.kind).state.is_on;
        if was_on != (e.state == RadioState::On) {
            self.toggle_failed.borrow_mut().insert(e.kind, false);
        }
        self.update_radio_tile(e.kind);
        self.changed.notify();
        if e.kind == RadioKind::WiFi {
            self.wifi_page.on_radio_changed();
        }
    }

    fn tile(&self, kind: RadioKind) -> RadioTile {
        let st = self.state.borrow();
        if kind == RadioKind::WiFi { st.wifi.clone() } else { st.bluetooth.clone() }
    }

    fn update_radio_tile(&self, kind: RadioKind) {
        let Some(r) = self.radio_service() else { return };
        let state = if kind == RadioKind::WiFi { r.wifi_state() } else { r.bluetooth_state() };
        let failed = self.toggle_failed.borrow().get(&kind).copied().unwrap_or(false);
        let resolved = RadioTileResolver::resolve(r.access(), state, failed);
        {
            let mut st = self.state.borrow_mut();
            if kind == RadioKind::WiFi { st.wifi.apply(resolved) } else { st.bluetooth.apply(resolved) }
        }
        if kind == RadioKind::Bluetooth {
            self.update_bluetooth_view();
        }
    }

    /// Döşemeye tıklama: radyo değiştirilebiliyorsa açar/kapatır, değiştirilemiyorsa ilgili Windows ayarını açar.
    pub fn activate_radio(self: &Rc<Self>, kind: RadioKind) {
        let Some(r) = self.radio_service() else { return };
        match self.tile(kind).state.action {
            RadioTileAction::Toggle => {
                let me = UiRef::new(self);
                std::thread::spawn(move || {
                    let ok = r.toggle(kind);
                    dispatch::post(move || {
                        if let Some(vm) = me.upgrade() {
                            vm.toggle_failed.borrow_mut().insert(kind, !ok);
                            vm.update_radio_tile(kind);
                            // Başlık anahtarı reddedilen denemede gerçek duruma döner (görünüm yeniden okur).
                            vm.changed.notify();
                        }
                    });
                });
            }
            RadioTileAction::OpenSettings => r.open_settings(kind),
            RadioTileAction::OpenPrivacySettings => r.open_privacy_settings(),
            _ => {}
        }
    }

    /// Döşemenin okuna basınca: Wi-Fi ya da Bluetooth listesi sayfası çentik içinde açılır.
    pub fn open_page(self: &Rc<Self>, kind: RadioKind) {
        if self.disposed.get() {
            return;
        }
        let page = if kind == RadioKind::WiFi { ControlPage::WiFi } else { ControlPage::Bluetooth };
        self.state.borrow_mut().page = page;
        self.wifi_page.set_active(page == ControlPage::WiFi);
        self.update_bluetooth_view();
        self.changed.notify();
    }

    /// Geri oku: ana görünüme döner ve liste servislerini bırakır.
    pub fn back(self: &Rc<Self>) {
        self.close_page();
    }

    fn close_page(self: &Rc<Self>) {
        self.wifi_page.set_active(false);
        self.state.borrow_mut().page = ControlPage::Main;
        self.changed.notify();
    }

    /// Rahatsız Etme'yi kodla açıp kapatmak yok; döşeme Windows'taki ayarı açar.
    pub fn open_do_not_disturb_settings(&self) {
        self.focus_assist.open_settings();
    }

    pub fn toggle_mute(&self) {
        if let Some((a, _)) = self.audio.borrow().as_ref() {
            a.toggle_mute();
        }
    }

    pub fn on_volume_slider_changed(&self, percent: f64) {
        let normalized = (percent / 100.0).clamp(0.0, 1.0) as f32;
        if let Some((a, _)) = self.audio.borrow().as_ref() {
            a.set_volume(normalized);
        }
        {
            let mut st = self.state.borrow_mut();
            st.volume = normalized;
            st.volume_percent = percent.round() as i32;
        }
        self.changed.notify();
    }

    pub fn on_brightness_slider_changed(&self, value: f64) {
        if !self.state.borrow().is_brightness_supported {
            return;
        }
        let v = value.clamp(0.0, 100.0) as i32;
        // Bloklamaz: servis değeri arka planda uygular, art arda gelenleri birleştirir.
        if let Some((s, _, _)) = self.brightness.borrow().as_ref() {
            s.set_brightness(v);
        }
        self.state.borrow_mut().brightness = v;
        self.changed.notify();
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.disposed.replace(true) {
            return;
        }
        self.set_do_not_disturb_active(false);
        self.wifi_page.dispose();
        if let Some(id) = self.bt_sub.take() {
            self.bluetooth_devices.changed.unsubscribe(id);
        }
        self.bluetooth_devices.dispose();
        if let Some((a, s)) = self.audio.borrow_mut().take() {
            a.state_changed().unsubscribe(s);
        }
        if let Some((r, s)) = self.radio.borrow_mut().take() {
            r.state_changed().unsubscribe(s);
            drop(r);
            (self.factories.radio_release)();
        }
        if let Some((b, s1, s2)) = self.brightness.borrow_mut().take() {
            b.brightness_changed().unsubscribe(s1);
            b.availability_changed().unsubscribe(s2);
        }
    }
}

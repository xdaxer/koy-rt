//! `IBluetoothService` uygulaması. Eşleşmiş cihazlar iki `DeviceWatcher` ile izlenir (klasik ve LE); bağlantı ve pil
//! değişiklikleri olayla gelir, yoklama yok. Servis yaşadığı sürece izleyiciler çalışır, `dispose` ikisini de durdurur.
//! Bağlan / kes: yalnız ses cihazlarında `BluetoothAudioKs` ile; sonuç bağlantı durumundan okunur.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use koy_core::bluetooth::{
    BatteryRawValue, BluetoothActionResult, BluetoothBatteryFilter, BluetoothDeviceInfo, BluetoothDeviceKind, BluetoothList,
    GattBatteryParser, IBluetoothService,
};
use koy_core::event::Event;
use koy_core::text;
use windows::Devices::Bluetooth::GenericAttributeProfile::{GattCharacteristicUuids, GattCommunicationStatus, GattServiceUuids};
use windows::Devices::Bluetooth::{BluetoothCacheMode, BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::{DeviceInformation, DeviceInformationKind, DeviceInformationUpdate, DeviceWatcher, DeviceWatcherStatus};
use windows_collections::{IIterable, IMapView};
use windows::Foundation::{IReference, TypedEventHandler};
use windows::Storage::Streams::DataReader;
use windows::core::{GUID, HSTRING, IInspectable, Interface};

use super::bluetooth_audio_ks::{BluetoothAudioKs, Operation};
use super::support::{ComApartment, log_error};
use crate::wireless::SettingsUri;

const PROP_IS_CONNECTED: &str = "System.Devices.Aep.IsConnected";
const PROP_CONTAINER_ID: &str = "System.Devices.Aep.ContainerId";
// DEVPKEY_Bluetooth_Battery
const PROP_BATTERY: &str = "{104EA319-6EE2-4701-BD47-8DDBF425BBE5} 2";
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(8);
const BATTERY_THROTTLE: Duration = Duration::from_secs(60);
// HRESULT_FROM_WIN32(ERROR_DEVICE_NOT_AVAILABLE): "Cihaz kullanıma hazır değil".
const DEVICE_NOT_AVAILABLE: i32 = 0x8007_10DF_u32 as i32;

struct Entry {
    id: String,
    is_low_energy: bool,
    name: String,
    is_connected: bool,
    battery: Option<i32>,
    container_id: Option<String>,
    kind: BluetoothDeviceKind,
    kind_requested: bool,
    has_prop_battery: bool,
}

impl Entry {
    fn info(&self) -> BluetoothDeviceInfo {
        let mut info = BluetoothDeviceInfo::new(self.id.clone(), self.name.clone(), self.kind, self.is_connected, self.battery)
            .with_low_energy(self.is_low_energy);
        if let Some(c) = &self.container_id {
            info = info.with_container_id(c.clone());
        }
        info
    }
}

struct State {
    entries: HashMap<String, Entry>,
    // Anahtar büyük harfli kimlik (OrdinalIgnoreCase).
    battery_lookups: HashMap<String, Instant>,
    classic: Option<DeviceWatcher>,
    le: Option<DeviceWatcher>,
    disposed: bool,
    battery_reads_enabled: bool,
    generation: u64,
}

// SAFETY: WinRT nesneleri çeviktir; yalnız kilit altında kullanılır.
unsafe impl Send for State {}

struct Inner {
    state: Mutex<State>,
    changed: Condvar,
    kind_gate: Mutex<()>,
    battery_filter: BluetoothBatteryFilter,
    pending_enumerations: AtomicI32,
    is_ready: AtomicBool,
    canceled: AtomicBool,
    devices_changed: Event<()>,
}

pub struct BluetoothService {
    inner: Arc<Inner>,
}

impl Default for BluetoothService {
    fn default() -> Self {
        Self::new()
    }
}

fn prop<'a>(props: &'a IMapView<HSTRING, IInspectable>, name: &str) -> Option<IInspectable> {
    let key = HSTRING::from(name);
    if !props.HasKey(&key).unwrap_or(false) {
        return None;
    }
    props.Lookup(&key).ok()
}

fn as_bool(v: &IInspectable) -> Option<bool> {
    v.cast::<IReference<bool>>().ok()?.Value().ok()
}

fn as_guid(v: &IInspectable) -> Option<GUID> {
    v.cast::<IReference<GUID>>().ok()?.Value().ok()
}

/// WinRT kutulu değeri `BatteryRawValue`'ya (C#'taki `object` + `Convert.ToInt32`).
fn as_battery(v: &IInspectable) -> BatteryRawValue {
    if let Ok(r) = v.cast::<IReference<u8>>().and_then(|r| r.Value()) {
        return BatteryRawValue::Byte(r);
    }
    if let Ok(r) = v.cast::<IReference<i32>>().and_then(|r| r.Value()) {
        return BatteryRawValue::Int(r);
    }
    if let Ok(r) = v.cast::<IReference<u32>>().and_then(|r| r.Value()) {
        return BatteryRawValue::Long(r as i64);
    }
    if let Ok(r) = v.cast::<IReference<i64>>().and_then(|r| r.Value()) {
        return BatteryRawValue::Long(r);
    }
    if let Ok(r) = v.cast::<IReference<f64>>().and_then(|r| r.Value()) {
        return BatteryRawValue::Double(r);
    }
    if let Ok(r) = v.cast::<IReference<bool>>().and_then(|r| r.Value()) {
        return BatteryRawValue::Boolean(r);
    }
    if let Ok(r) = v.cast::<IReference<HSTRING>>().and_then(|r| r.Value()) {
        return BatteryRawValue::Text(r.to_string());
    }
    BatteryRawValue::Unsupported
}

/// `Guid.ToString("D")`.
fn guid_d(g: &GUID) -> String {
    format!("{g:?}").to_ascii_lowercase()
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn apply_properties(&self, entry: &mut Entry, props: &IMapView<HSTRING, IInspectable>) {
        if let Some(v) = prop(props, PROP_IS_CONNECTED) {
            entry.is_connected = as_bool(&v) == Some(true);
        }
        if let Some(g) = prop(props, PROP_CONTAINER_ID).and_then(|v| as_guid(&v))
            && g != GUID::zeroed()
        {
            entry.container_id = Some(guid_d(&g));
        }
        // Pil güncellemesi yalnız pil değişince gelir; yok olan değer önceki pili silmez.
        if let Some(v) = prop(props, PROP_BATTERY)
            && let Some(parsed) = BluetoothList::parse_battery(Some(&as_battery(&v)))
        {
            entry.has_prop_battery = true;
            entry.battery = Some(self.battery_filter.filter(&entry.id, parsed));
        }
    }

    fn notify(&self) {
        {
            let mut st = self.lock();
            if st.disposed {
                return;
            }
            st.generation += 1;
        }
        self.changed.notify_all();
        // Başlangıç taraması bitmeden her cihaz için olay yayınlanmaz: arayüz tek seferde güncellenir.
        if !self.is_ready.load(Ordering::SeqCst) {
            return;
        }
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.devices_changed.invoke(&()))).is_err() {
            koy_core::diagnostics::CrashLog::write(
                "BluetoothService.DevicesChanged abonesi",
                &koy_core::diagnostics::CrashError::exception("System.Exception", "subscriber panicked", ""),
            );
        }
    }

    fn on_added(self: &Arc<Self>, info: &DeviceInformation, is_low_energy: bool) {
        let Ok(id) = info.Id().map(|h| h.to_string()) else { return };
        {
            let mut st = self.lock();
            if st.disposed {
                return;
            }
            let mut entry = Entry {
                id: id.clone(),
                is_low_energy,
                name: info.Name().map(|n| n.to_string()).unwrap_or_default(),
                is_connected: false,
                battery: None,
                container_id: None,
                kind: BluetoothDeviceKind::Other,
                kind_requested: false,
                has_prop_battery: false,
            };
            if let Ok(props) = info.Properties() {
                self.apply_properties(&mut entry, &props);
            }
            st.entries.insert(id.clone(), entry);
        }
        self.queue_kind_lookup(&id);
        self.queue_battery_lookup(&id);
        self.notify();
    }

    fn on_updated(self: &Arc<Self>, update: &DeviceInformationUpdate) {
        let Ok(id) = update.Id().map(|h| h.to_string()) else { return };
        let mut lookup = false;
        {
            let mut st = self.lock();
            if st.disposed || !st.entries.contains_key(&id) {
                return;
            }
            let key = text::to_upper_invariant(&id);
            let last = st.battery_lookups.get(&key).copied();
            let entry = st.entries.get_mut(&id).expect("entry");
            let was_connected = entry.is_connected;
            if let Ok(props) = update.Properties() {
                self.apply_properties(entry, &props);
            }
            let (connected, le, has_prop) = (entry.is_connected, entry.is_low_energy, entry.has_prop_battery);
            if !connected && was_connected {
                self.battery_filter.reset(&id);
                st.battery_lookups.remove(&key);
            } else if connected && le && !has_prop && (!was_connected || last.is_none_or(|l| l.elapsed() >= BATTERY_THROTTLE)) {
                lookup = true;
            }
        }
        if lookup {
            self.queue_battery_lookup(&id);
        }
        self.notify();
    }

    fn on_removed(&self, update: &DeviceInformationUpdate) {
        let Ok(id) = update.Id().map(|h| h.to_string()) else { return };
        self.battery_filter.reset(&id);
        {
            let mut st = self.lock();
            st.battery_lookups.remove(&text::to_upper_invariant(&id));
            if st.disposed || st.entries.remove(&id).is_none() {
                return;
            }
        }
        self.notify();
    }

    fn on_enumeration_completed(&self) {
        if self.pending_enumerations.fetch_sub(1, Ordering::SeqCst) - 1 <= 0 {
            self.is_ready.store(true, Ordering::SeqCst);
            self.notify();
        }
    }

    // Cihaz türü yalnız FromIdAsync ile okunur (ClassOfDevice / Appearance); bir kez, sırayla, sonra nesne bırakılır.
    fn queue_kind_lookup(self: &Arc<Self>, id: &str) {
        let is_le = {
            let mut st = self.lock();
            let Some(entry) = st.entries.get_mut(id) else { return };
            if entry.kind_requested {
                return;
            }
            entry.kind_requested = true;
            entry.is_low_energy
        };
        let weak = Arc::downgrade(self);
        let id = id.to_string();
        std::thread::spawn(move || {
            let Some(inner) = weak.upgrade() else { return };
            let _com = ComApartment::enter_mta();
            let _gate = inner.kind_gate.lock().unwrap_or_else(|e| e.into_inner());
            if inner.lock().disposed {
                return;
            }
            let kind = if is_le { read_le_kind(&id) } else { read_classic_kind(&id) };
            match kind {
                Ok(kind) => {
                    if let Some(e) = inner.lock().entries.get_mut(&id) {
                        e.kind = kind;
                    }
                    inner.notify();
                }
                // Eşleşmiş ama erişilemeyen cihaz (kapalı, menzil dışı): olağan durum, tür "diğer" kalır; günlüğe yazılmaz.
                Err(e) if e.code().0 == DEVICE_NOT_AVAILABLE => {}
                Err(e) => log_error("BluetoothService.cihaz türü", &e),
            }
        });
    }

    fn queue_battery_lookup(self: &Arc<Self>, id: &str) {
        {
            let mut st = self.lock();
            let Some(entry) = st.entries.get(id) else { return };
            if st.disposed || !st.battery_reads_enabled || !entry.is_connected || !entry.is_low_energy || entry.has_prop_battery {
                return;
            }
            let key = text::to_upper_invariant(id);
            if st.battery_lookups.get(&key).is_some_and(|l| l.elapsed() < BATTERY_THROTTLE) {
                return;
            }
            st.battery_lookups.insert(key, Instant::now());
        }
        let weak = Arc::downgrade(self);
        let id = id.to_string();
        std::thread::spawn(move || {
            let Some(inner) = weak.upgrade() else { return };
            let enabled = inner.lock().battery_reads_enabled;
            if inner.canceled.load(Ordering::SeqCst) || inner.lock().disposed || !enabled {
                return;
            }
            let _com = ComApartment::enter_mta();
            match read_gatt_battery(&id, &inner.canceled) {
                Ok(Some(battery)) => {
                    let filtered = inner.battery_filter.filter(&id, battery);
                    {
                        let mut st = inner.lock();
                        if st.disposed {
                            return;
                        }
                        let Some(current) = st.entries.get_mut(&id) else { return };
                        current.battery = Some(filtered);
                    }
                    inner.notify();
                }
                Ok(None) => {}
                Err(e) if e.code().0 == DEVICE_NOT_AVAILABLE => {}
                Err(e) => log_error("BluetoothService.GATT pil okuma", &e),
            }
        });
    }

    // Aynı fiziksel cihazın klasik ve LE kayıtlarından biri bağlıysa bağlı sayılır (liste birleştirmesiyle aynı kural).
    fn is_connected_now(&self, device_id: &str) -> bool {
        let st = self.lock();
        let Some(entry) = st.entries.get(device_id) else { return false };
        match &entry.container_id {
            None => entry.is_connected,
            Some(c) => st.entries.values().any(|e| e.is_connected && e.container_id.as_deref().is_some_and(|x| text::eq_ignore_case(x, c))),
        }
    }

    fn wait_for_state(&self, device_id: &str, connected: bool) -> bool {
        let deadline = Instant::now() + CONFIRM_TIMEOUT;
        loop {
            let generation = {
                let st = self.lock();
                if st.disposed {
                    return false;
                }
                st.generation
            };
            if self.is_connected_now(device_id) == connected {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let st = self.lock();
            if st.generation == generation {
                let _ = self.changed.wait_timeout(st, deadline - now);
            }
        }
    }
}

fn read_classic_kind(id: &str) -> windows::core::Result<BluetoothDeviceKind> {
    let device = BluetoothDevice::FromIdAsync(&HSTRING::from(id))?.join()?;
    let raw = device.ClassOfDevice()?.RawValue()?;
    let _ = device.Close();
    Ok(BluetoothList::from_class_of_device(raw))
}

fn read_le_kind(id: &str) -> windows::core::Result<BluetoothDeviceKind> {
    let device = BluetoothLEDevice::FromIdAsync(&HSTRING::from(id))?.join()?;
    let raw = device.Appearance()?.RawValue()?;
    let _ = device.Close();
    Ok(BluetoothList::from_appearance(raw as u32))
}

fn read_gatt_battery(id: &str, canceled: &AtomicBool) -> windows::core::Result<Option<i32>> {
    let device = BluetoothLEDevice::FromIdAsync(&HSTRING::from(id))?.join()?;
    let result = (|| {
        if device.ConnectionStatus()? != BluetoothConnectionStatus::Connected || canceled.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let battery = GattServiceUuids::Battery()?;
        let mut services = device.GetGattServicesForUuidWithCacheModeAsync(battery, BluetoothCacheMode::Cached)?.join()?;
        if services.Status()? != GattCommunicationStatus::Success || services.Services()?.Size()? == 0 {
            if canceled.load(Ordering::SeqCst) {
                return Ok(None);
            }
            services = device.GetGattServicesForUuidWithCacheModeAsync(battery, BluetoothCacheMode::Uncached)?.join()?;
            if services.Status()? != GattCommunicationStatus::Success || services.Services()?.Size()? == 0 {
                return Ok(None);
            }
        }

        let level = GattCharacteristicUuids::BatteryLevel()?;
        let mut readings: Vec<Vec<u8>> = Vec::new();
        let list = services.Services()?;
        for service in &list {
            if canceled.load(Ordering::SeqCst) {
                break;
            }
            let mut chars = service.GetCharacteristicsForUuidWithCacheModeAsync(level, BluetoothCacheMode::Cached)?.join()?;
            if chars.Status()? != GattCommunicationStatus::Success || chars.Characteristics()?.Size()? == 0 {
                chars = service.GetCharacteristicsForUuidWithCacheModeAsync(level, BluetoothCacheMode::Uncached)?.join()?;
                if chars.Status()? != GattCommunicationStatus::Success || chars.Characteristics()?.Size()? == 0 {
                    continue;
                }
            }
            for ch in &chars.Characteristics()? {
                if canceled.load(Ordering::SeqCst) {
                    break;
                }
                let mut read = ch.ReadValueWithCacheModeAsync(BluetoothCacheMode::Cached)?.join()?;
                let empty = |r: &windows::Devices::Bluetooth::GenericAttributeProfile::GattReadResult| {
                    r.Status().ok() != Some(GattCommunicationStatus::Success) || r.Value().and_then(|v| v.Length()).unwrap_or(0) == 0
                };
                if empty(&read) {
                    read = ch.ReadValueWithCacheModeAsync(BluetoothCacheMode::Uncached)?.join()?;
                }
                if read.Status()? == GattCommunicationStatus::Success
                    && let Ok(buffer) = read.Value()
                    && buffer.Length()? > 0
                {
                    let mut bytes = vec![0u8; buffer.Length()? as usize];
                    let reader = DataReader::FromBuffer(&buffer)?;
                    reader.ReadBytes(&mut bytes)?;
                    readings.push(bytes);
                }
            }
        }
        for service in &list {
            let _ = service.Close();
        }
        let refs: Vec<&[u8]> = readings.iter().map(Vec::as_slice).collect();
        Ok(GattBatteryParser::parse_lowest(Some(&refs)))
    })();
    let _ = device.Close();
    result
}

fn create_watcher(selector: &HSTRING, is_low_energy: bool, weak: Weak<Inner>) -> windows::core::Result<DeviceWatcher> {
    let props: Vec<HSTRING> = [PROP_IS_CONNECTED, PROP_CONTAINER_ID, PROP_BATTERY].iter().map(|p| HSTRING::from(*p)).collect();
    let requested = IIterable::<HSTRING>::from(props);
    let watcher = DeviceInformation::CreateWatcherWithKindAqsFilterAndAdditionalProperties(
        selector,
        &requested,
        DeviceInformationKind::AssociationEndpoint,
    )?;
    let w = weak.clone();
    watcher.Added(&TypedEventHandler::new(move |_, info: windows::core::Ref<DeviceInformation>| {
        if let (Some(inner), Some(info)) = (w.upgrade(), info.as_ref()) {
            inner.on_added(info, is_low_energy);
        }
        Ok(())
    }))?;
    let w = weak.clone();
    watcher.Updated(&TypedEventHandler::new(move |_, update: windows::core::Ref<DeviceInformationUpdate>| {
        if let (Some(inner), Some(update)) = (w.upgrade(), update.as_ref()) {
            inner.on_updated(update);
        }
        Ok(())
    }))?;
    let w = weak.clone();
    watcher.Removed(&TypedEventHandler::new(move |_, update: windows::core::Ref<DeviceInformationUpdate>| {
        if let (Some(inner), Some(update)) = (w.upgrade(), update.as_ref()) {
            inner.on_removed(update);
        }
        Ok(())
    }))?;
    watcher.EnumerationCompleted(&TypedEventHandler::new(move |_, _: windows::core::Ref<IInspectable>| {
        if let Some(inner) = weak.upgrade() {
            inner.on_enumeration_completed();
        }
        Ok(())
    }))?;
    Ok(watcher)
}

impl BluetoothService {
    pub fn new() -> Self {
        let inner = Arc::new(Inner {
            state: Mutex::new(State {
                entries: HashMap::new(),
                battery_lookups: HashMap::new(),
                classic: None,
                le: None,
                disposed: false,
                battery_reads_enabled: false,
                generation: 0,
            }),
            changed: Condvar::new(),
            kind_gate: Mutex::new(()),
            battery_filter: BluetoothBatteryFilter::default(),
            pending_enumerations: AtomicI32::new(0),
            is_ready: AtomicBool::new(false),
            canceled: AtomicBool::new(false),
            devices_changed: Event::new(),
        });
        let started = (|| -> windows::core::Result<()> {
            let classic = create_watcher(&BluetoothDevice::GetDeviceSelectorFromPairingState(true)?, false, Arc::downgrade(&inner))?;
            let le = create_watcher(&BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)?, true, Arc::downgrade(&inner))?;
            inner.pending_enumerations.store(2, Ordering::SeqCst);
            {
                let mut st = inner.lock();
                st.classic = Some(classic.clone());
                st.le = Some(le.clone());
            }
            classic.Start()?;
            le.Start()?;
            Ok(())
        })();
        if let Err(e) = started {
            // Bluetooth yığını yoksa liste boş ve hazır kalır.
            log_error("BluetoothService başlatma", &e);
            inner.is_ready.store(true, Ordering::SeqCst);
        }
        BluetoothService { inner }
    }

    fn change(&self, device_id: &str, connect: bool) -> BluetoothActionResult {
        let container = {
            let st = self.inner.lock();
            let Some(entry) = st.entries.get(device_id) else { return BluetoothActionResult::Failed };
            entry.container_id.clone()
        };
        if self.inner.is_connected_now(device_id) == connect {
            return BluetoothActionResult::Confirmed;
        }
        let Some(container) = container else { return BluetoothActionResult::Unsupported };

        let operation = if connect { Operation::Reconnect } else { Operation::Disconnect };
        let sent = std::thread::spawn(move || {
            let _com = ComApartment::enter_mta();
            BluetoothAudioKs::try_send(&container, operation)
        })
        .join()
        .unwrap_or(false);
        if !sent {
            return BluetoothActionResult::Unsupported;
        }
        // Sürücünün isteği alması bağlandığı anlamına gelmez: sonuç bağlantı durumundan okunur.
        if self.inner.wait_for_state(device_id, connect) { BluetoothActionResult::Confirmed } else { BluetoothActionResult::NotConfirmed }
    }

    pub fn dispose(&self) {
        let watchers = {
            let mut st = self.inner.lock();
            if st.disposed {
                return;
            }
            st.disposed = true;
            self.inner.canceled.store(true, Ordering::SeqCst);
            self.inner.battery_filter.clear();
            st.battery_lookups.clear();
            st.entries.clear();
            [st.classic.take(), st.le.take()]
        };
        for watcher in watchers.into_iter().flatten() {
            // Zaten durmuş izleyici: hata yutulur.
            if matches!(watcher.Status(), Ok(DeviceWatcherStatus::Started) | Ok(DeviceWatcherStatus::EnumerationCompleted)) {
                let _ = watcher.Stop();
            }
        }
        self.inner.changed.notify_all();
    }
}

impl Drop for BluetoothService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IBluetoothService for BluetoothService {
    fn devices(&self) -> Vec<BluetoothDeviceInfo> {
        self.inner.lock().entries.values().map(Entry::info).collect()
    }

    fn is_ready(&self) -> bool {
        self.inner.is_ready.load(Ordering::SeqCst)
    }

    fn devices_changed(&self) -> &Event<()> {
        &self.inner.devices_changed
    }

    fn connect(&self, device_id: &str) -> BluetoothActionResult {
        self.change(device_id, true)
    }

    fn disconnect(&self, device_id: &str) -> BluetoothActionResult {
        self.change(device_id, false)
    }

    fn set_battery_reads_enabled(&self, enabled: bool) {
        let pending: Vec<String> = {
            let mut st = self.inner.lock();
            if st.disposed {
                return;
            }
            st.battery_reads_enabled = enabled;
            if !enabled {
                return;
            }
            // Sayfa yeni açıldı: pili eksik bağlı BLE cihazlar için okuma hemen sıraya girer.
            st.entries.values().filter(|e| e.is_connected && e.is_low_energy && !e.has_prop_battery).map(|e| e.id.clone()).collect()
        };
        for id in pending {
            self.inner.queue_battery_lookup(&id);
        }
    }

    fn open_settings(&self) {
        SettingsUri::open("ms-settings:bluetooth");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_watchers_and_becomes_ready() {
        let service = BluetoothService::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !service.is_ready() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(service.is_ready());
        let _ = service.devices();
        assert_eq!(service.connect("no-such-device"), BluetoothActionResult::Failed);
        service.dispose();
    }
}

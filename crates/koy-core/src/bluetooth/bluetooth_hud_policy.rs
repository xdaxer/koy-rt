use std::sync::Arc;

use chrono::NaiveDate;

use super::bluetooth_list::BluetoothList;
use super::bluetooth_models::BluetoothDeviceInfo;
use crate::text;
use crate::time::{SystemTimeProvider, TimeProvider, TimeSpan};

/// Bluetooth ses cihazı bağlantı durumu değiştiğinde üretilen HUD olayı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothHudEvent {
    pub name: String,
    pub connected: bool,
    pub battery_percent: Option<i32>,
}

impl BluetoothHudEvent {
    pub fn new(name: impl Into<String>, connected: bool, battery_percent: Option<i32>) -> Self {
        BluetoothHudEvent { name: name.into(), connected, battery_percent }
    }
}

/// `Dictionary<string, V>(StringComparer.OrdinalIgnoreCase)` karşılığı. C# `Dictionary` ekleme sırasını korur
/// (PORTING.md #13) ve burada hiç silme yoktur; anahtar varsa değer yerinde güncellenir, anahtarın ilk yazılışı kalır.
struct IgnoreCaseMap<V> {
    entries: Vec<(String, V)>,
}

impl<V> IgnoreCaseMap<V> {
    fn new() -> Self {
        IgnoreCaseMap { entries: Vec::new() }
    }

    fn get(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| text::eq_ignore_case(k, key)).map(|(_, v)| v)
    }

    fn set(&mut self, key: &str, value: V) {
        match self.entries.iter_mut().find(|(k, _)| text::eq_ignore_case(k, key)) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key.to_owned(), value)),
        }
    }
}

/// Bluetooth cihaz listesindeki bağlantı değişikliklerini izleyip HUD olayları üreten saf mantık.
/// Açılışta tohumlama (seeding) bastırılır; yalnızca ses cihazları (IsAudio) işlenir;
/// aynı cihaz için 3 saniye içindeki titreşimler (debouncing) bastırılır.
pub struct BluetoothHudPolicy {
    time_provider: Arc<dyn TimeProvider>,
    last_emitted_connected: IgnoreCaseMap<bool>,
    last_emitted_time: IgnoreCaseMap<TimeSpan>,
    device_names: IgnoreCaseMap<String>,
    is_seeded: bool,
}

impl BluetoothHudPolicy {
    const DEBOUNCE_WINDOW: TimeSpan = TimeSpan::from_secs(3);

    /// `timeProvider` verilmezse `TimeProvider.System`.
    pub fn new(time_provider: Option<Arc<dyn TimeProvider>>) -> Self {
        BluetoothHudPolicy {
            time_provider: time_provider.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
            last_emitted_connected: IgnoreCaseMap::new(),
            last_emitted_time: IgnoreCaseMap::new(),
            device_names: IgnoreCaseMap::new(),
            is_seeded: false,
        }
    }

    /// Verilen cihaz listesini ve zamanı işleyerek yeni HUD olaylarını döner.
    /// (C#: `Update(devices, now)` aşırı yüklemesi.)
    pub fn update_with_now(&mut self, devices: &[BluetoothDeviceInfo], now: TimeSpan) -> Vec<BluetoothHudEvent> {
        let normalized = BluetoothList::normalize(devices);
        let audio_devices: Vec<&BluetoothDeviceInfo> = normalized.iter().filter(|d| d.is_audio()).collect();

        if !self.is_seeded {
            self.is_seeded = true;
            for device in audio_devices {
                let key = key_of(device);
                self.last_emitted_connected.set(key, device.is_connected);
                self.device_names.set(key, device.name.clone());
            }
            return Vec::new();
        }

        let mut events = Vec::new();
        let mut seen_keys: Vec<&str> = Vec::new();

        for device in audio_devices {
            let key = key_of(device);
            seen_keys.push(key);
            self.device_names.set(key, device.name.clone());

            let was_connected = self.last_emitted_connected.get(key).copied().unwrap_or(false);

            if device.is_connected != was_connected {
                if self.is_debounced(key, now) {
                    // 3 saniyelik titreşim penceresinde bastır
                    continue;
                }

                self.last_emitted_time.set(key, now);
                self.last_emitted_connected.set(key, device.is_connected);
                events.push(BluetoothHudEvent::new(device.name.clone(), device.is_connected, device.battery_percent));
            }
        }

        // Önceden bağlı olup listeden tamamen çıkan (bağlantı kopması / eşleşme silinmesi) cihazlar
        for i in 0..self.last_emitted_connected.entries.len() {
            let (key, was_connected) = {
                let (k, v) = &self.last_emitted_connected.entries[i];
                (k.clone(), *v)
            };
            if was_connected && !seen_keys.iter().any(|s| text::eq_ignore_case(s, &key)) {
                if self.is_debounced(&key, now) {
                    continue;
                }

                self.last_emitted_time.set(&key, now);
                self.last_emitted_connected.set(&key, false);
                let name = self.device_names.get(&key).cloned().unwrap_or_else(|| "Bluetooth".to_owned());
                events.push(BluetoothHudEvent::new(name, false, None));
            }
        }

        events
    }

    /// `TimeProvider` kullanarak anlık zaman ile güncelleme yapar.
    /// (C#: `Update(devices)` aşırı yüklemesi; `TimeSpan.FromTicks(GetUtcNow().Ticks)`.)
    pub fn update(&mut self, devices: &[BluetoothDeviceInfo]) -> Vec<BluetoothHudEvent> {
        // .NET `DateTimeOffset.Ticks`: UTC saatin 0001-01-01'den beri 100 ns sayısı.
        let epoch = NaiveDate::from_ymd_opt(1, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
        let now = TimeSpan::from_chrono(self.time_provider.utc_now().naive_utc() - epoch);
        self.update_with_now(devices, now)
    }

    /// Update ile aynı işlevi görür; alternatif adlandırma için.
    pub fn process_devices(&mut self, devices: &[BluetoothDeviceInfo]) -> Vec<BluetoothHudEvent> {
        self.update(devices)
    }

    /// `_lastEmittedTime.TryGetValue(key, out lastTime) && (now - lastTime) >= Zero && (now - lastTime) < DebounceWindow`.
    fn is_debounced(&self, key: &str, now: TimeSpan) -> bool {
        match self.last_emitted_time.get(key) {
            Some(&last_time) => (now - last_time) >= TimeSpan::ZERO && (now - last_time) < Self::DEBOUNCE_WINDOW,
            None => false,
        }
    }
}

fn key_of(device: &BluetoothDeviceInfo) -> &str {
    match device.container_id.as_deref() {
        Some(c) if !c.is_empty() => c,
        _ => &device.id,
    }
}

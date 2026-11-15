use std::collections::HashMap;
use std::sync::Mutex;

use crate::text;

/// Bluetooth cihazlarında (fare, klavye, kulaklık) yük altındaki anlık voltaj düşüşlerini,
/// toparlanma sıçramalarını ve ardışık tek haneli ADC titreşimlerini süzen saf filtre.
/// Kayan pencerede medyan hesabı ve histerezis eşiği uygular.
pub struct BluetoothBatteryFilter {
    window_size: i32,
    hysteresis_threshold: i32,
    consecutive_threshold: i32,
    // C# `Dictionary(StringComparer.OrdinalIgnoreCase)`: anahtar değişmez büyük harfe çevrilerek tutulur.
    devices: Mutex<HashMap<String, DeviceState>>,
}

impl Default for BluetoothBatteryFilter {
    /// C# varsayılanları: `windowSize = 3, hysteresisThreshold = 2, consecutiveThreshold = 3`.
    fn default() -> Self {
        Self::new(3, 2, 3)
    }
}

impl BluetoothBatteryFilter {
    /// Geçersiz bağımsız değişkende panikler (C#: `ArgumentOutOfRangeException`).
    pub fn new(window_size: i32, hysteresis_threshold: i32, consecutive_threshold: i32) -> Self {
        if !(1..=64).contains(&window_size) {
            panic!("windowSize: Pencere boyutu 1 ile 64 arasında olmalıdır.");
        }
        if hysteresis_threshold < 0 {
            panic!("hysteresisThreshold: Histerezis eşiği negatif olamaz.");
        }
        if consecutive_threshold < 1 {
            panic!("consecutiveThreshold: Ardışık eşik en az 1 olmalıdır.");
        }

        BluetoothBatteryFilter {
            window_size,
            hysteresis_threshold,
            consecutive_threshold,
            devices: Mutex::new(HashMap::new()),
        }
    }

    /// Belirtilen cihaz için ham pil seviyesini (0..100) filtreleyip kararlı pil seviyesini döner.
    /// `device_id` boş ya da yalnız boşluksa panikler (C#: `ArgumentException`).
    pub fn filter(&self, device_id: &str, raw_percent: i32) -> i32 {
        assert!(!text::is_null_or_white_space(Some(device_id)), "deviceId boş ya da yalnız boşluk olamaz.");
        let clamped = raw_percent.clamp(0, 100);

        let mut devices = self.devices.lock().unwrap();
        let state = devices
            .entry(text::to_upper_invariant(device_id))
            .or_insert_with(|| DeviceState::new(self.window_size, self.hysteresis_threshold, self.consecutive_threshold));
        state.push(clamped)
    }

    /// Cihaz yeniden bağlandığında veya bağlantısı kesildiğinde filtre geçmişini sıfırlar.
    pub fn reset(&self, device_id: &str) {
        if text::is_null_or_white_space(Some(device_id)) {
            return;
        }

        self.devices.lock().unwrap().remove(&text::to_upper_invariant(device_id));
    }

    /// Tüm cihaz geçmişini temizler.
    pub fn clear(&self) {
        self.devices.lock().unwrap().clear();
    }
}

struct DeviceState {
    window_size: i32,
    hysteresis_threshold: i32,
    consecutive_threshold: i32,
    samples: Vec<i32>,
    last_reported: Option<i32>,
    consecutive_count: i32,
    // -1: azalan, +1: artan
    last_direction: i32,
}

impl DeviceState {
    fn new(window_size: i32, hysteresis_threshold: i32, consecutive_threshold: i32) -> Self {
        DeviceState {
            window_size,
            hysteresis_threshold,
            consecutive_threshold,
            samples: Vec::with_capacity(window_size as usize),
            last_reported: None,
            consecutive_count: 0,
            last_direction: 0,
        }
    }

    fn push(&mut self, sample: i32) -> i32 {
        if self.samples.len() >= self.window_size as usize {
            self.samples.remove(0);
        }
        self.samples.push(sample);

        let median = self.calculate_median();

        let Some(current) = self.last_reported else {
            self.last_reported = Some(median);
            return median;
        };

        let delta = median - current;

        if delta == 0 {
            self.consecutive_count = 0;
            self.last_direction = 0;
            return current;
        }

        let direction = delta.signum();

        // Eşik aşıldıysa doğrudan uygula (örn. %2 ve üzeri değişimler veya hızlı şarj)
        if delta.abs() >= self.hysteresis_threshold {
            self.last_reported = Some(median);
            self.consecutive_count = 0;
            self.last_direction = 0;
            return median;
        }

        // Küçük titreşimlerde (örn. %1 değişim) ardışık aynı yön kontrolü
        if direction == self.last_direction {
            self.consecutive_count += 1;
        } else {
            self.last_direction = direction;
            self.consecutive_count = 1;
        }

        if self.consecutive_count >= self.consecutive_threshold {
            self.last_reported = Some(median);
            self.consecutive_count = 0;
            self.last_direction = 0;
            return median;
        }

        current
    }

    fn calculate_median(&self) -> i32 {
        let count = self.samples.len();
        if count == 1 {
            return self.samples[0];
        }

        let mut copy = self.samples.clone();
        copy.sort_unstable();

        if count % 2 != 0 {
            return copy[count / 2];
        }

        let mid = count / 2;
        (copy[mid - 1] + copy[mid]) / 2
    }
}

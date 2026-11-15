use std::cmp::Ordering;
use std::collections::HashMap;

use super::bluetooth_models::*;
use crate::clock::Culture;
use crate::radio::RadioTileStatus;
use crate::system_stats::SystemFormat;
use crate::text;

/// `Convert.ToInt32(object, InvariantCulture)` girdisi için .NET nesne türlerinin karşılığı.
/// (`null` ayrı: `parse_battery(None)`.)
#[derive(Debug, Clone, PartialEq)]
pub enum BatteryRawValue {
    Byte(u8),
    Int(i32),
    Long(i64),
    Double(f64),
    Text(String),
    Boolean(bool),
    /// `IConvertible` olmayan nesne (`new object()`): `InvalidCastException` → "pil yok".
    Unsupported,
}

/// Bluetooth listesinin saf mantığı: cihaz türü eşleme, birleştirme, sıralama, pil ve durum metinleri.
pub struct BluetoothList;

// Class of Device (24 bit): ana sınıf bit 8-12, alt sınıf bit 2-7.
const MAJOR_COMPUTER: i32 = 1;
const MAJOR_PHONE: i32 = 2;
const MAJOR_AUDIO_VIDEO: i32 = 4;
const MAJOR_PERIPHERAL: i32 = 5;

impl BluetoothList {
    pub fn from_class_of_device(cod: u32) -> BluetoothDeviceKind {
        let major = ((cod >> 8) & 0x1F) as i32;
        let minor = ((cod >> 2) & 0x3F) as i32;

        match major {
            MAJOR_PHONE => BluetoothDeviceKind::Phone,
            MAJOR_COMPUTER => {
                if minor == 7 {
                    BluetoothDeviceKind::Tablet
                } else {
                    BluetoothDeviceKind::Other
                }
            }
            MAJOR_AUDIO_VIDEO => match minor {
                // Kulaklık takımı, eller serbest, kulaklık
                1 | 2 | 6 => BluetoothDeviceKind::Headphones,
                // Hoparlör, taşınabilir ses, hi-fi, ekran + hoparlör ve ayrıntısız ses cihazları
                _ => BluetoothDeviceKind::Speaker,
            },
            MAJOR_PERIPHERAL => match (minor >> 4) & 0x3 {
                // Alt sınıfın üst iki biti: 1 klavye, 2 işaretçi (fare, dokunmatik yüzey), 3 ikisi birden
                1 | 3 => BluetoothDeviceKind::Keyboard,
                2 => BluetoothDeviceKind::Mouse,
                _ => BluetoothDeviceKind::Other,
            },
            _ => BluetoothDeviceKind::Other,
        }
    }

    /// BLE "Appearance" (16 bit: kategori = değer >> 6, alt kategori = alt 6 bit) değerinden tür.
    pub fn from_appearance(appearance: u32) -> BluetoothDeviceKind {
        let category = (appearance >> 6) as i32;
        let sub = (appearance & 0x3F) as i32;

        match category {
            1 => BluetoothDeviceKind::Phone,
            2 => {
                if sub == 7 {
                    BluetoothDeviceKind::Tablet
                } else {
                    BluetoothDeviceKind::Other
                }
            }
            // HID: 1 klavye, 2 fare, 9 dokunmatik yüzey
            15 => match sub {
                1 => BluetoothDeviceKind::Keyboard,
                2 | 9 => BluetoothDeviceKind::Mouse,
                _ => BluetoothDeviceKind::Other,
            },
            // Ses alıcısı (hoparlör, soundbar)
            33 => BluetoothDeviceKind::Speaker,
            // Giyilebilir ses cihazı (kulakiçi, kulaklık, boyunluk)
            37 => BluetoothDeviceKind::Headphones,
            _ => BluetoothDeviceKind::Other,
        }
    }

    /// Aynı fiziksel cihazın (aynı ContainerId) klasik ve LE kayıtlarını tek satıra indirir: klasik kayıt tercih edilir,
    /// bağlı ve pil bilgisi iki kayıttan birleştirilir. Adsız kayıtlar atılır. Sıra: bağlı olanlar, sonra ada göre.
    ///
    /// C# ile aynı değil: son sıralamadaki `StringComparer.CurrentCultureIgnoreCase` yerine `OrdinalIgnoreCase`
    /// kullanılır (Rust'ta süreç kültürü yok; aksanlı/noktalama içeren adlarda sıra kültüre göre ayrışabilir).
    pub fn normalize(raw: &[BluetoothDeviceInfo]) -> Vec<BluetoothDeviceInfo> {
        let mut result: Vec<BluetoothDeviceInfo> = Vec::new();
        // `Dictionary(StringComparer.OrdinalIgnoreCase)`: anahtar değişmez büyük harfe çevrilerek tutulur.
        let mut by_container: HashMap<String, usize> = HashMap::new();

        // OrderBy(IsLowEnergy).ThenBy(Id, Ordinal): kararlı; Ordinal = UTF-16 kod birimi sırası.
        let mut ordered: Vec<&BluetoothDeviceInfo> = raw.iter().collect();
        ordered.sort_by(|a, b| a.is_low_energy.cmp(&b.is_low_energy).then_with(|| a.id.encode_utf16().cmp(b.id.encode_utf16())));

        for device in ordered {
            if text::is_null_or_white_space(Some(&device.name)) {
                continue;
            }

            if let Some(container) = device.container_id.as_deref().filter(|c| !c.is_empty()) {
                let key = text::to_upper_invariant(container);
                if let Some(&index) = by_container.get(&key) {
                    let kept = &result[index];
                    result[index] = BluetoothDeviceInfo {
                        is_connected: kept.is_connected || device.is_connected,
                        battery_percent: kept.battery_percent.or(device.battery_percent),
                        kind: if kept.kind == BluetoothDeviceKind::Other { device.kind } else { kept.kind },
                        ..kept.clone()
                    };
                    continue;
                }

                by_container.insert(key, result.len());
            }

            result.push(device.clone());
        }

        // OrderByDescending(IsConnected).ThenBy(Name, ...): kararlı.
        result.sort_by(|a, b| match b.is_connected.cmp(&a.is_connected) {
            Ordering::Equal => text::cmp_ignore_case(&a.name, &b.name),
            other => other,
        });
        result
    }

    /// Ana'daki kulaklık kartı için: bağlı ses cihazı (pili olan önce); yoksa `None`.
    pub fn primary_audio(normalized: &[BluetoothDeviceInfo]) -> Option<&BluetoothDeviceInfo> {
        // Where(...).OrderByDescending(HasValue).FirstOrDefault(): kararlı sıralamanın ilki =
        // pili olanların ilki, yoksa bağlı ses cihazlarının ilki.
        let mut candidates = normalized.iter().filter(|d| d.is_connected && d.is_audio());
        let first = candidates.next()?;
        if first.battery_percent.is_some() {
            return Some(first);
        }
        candidates.find(|d| d.battery_percent.is_some()).or(Some(first))
    }

    /// Dile uygun yüzde ("%86", "86%"); pil bilinmiyorsa ya da aralık dışıysa `None` (hiçbir şey gösterilmez).
    pub fn battery_text(percent: Option<i32>, culture: &Culture) -> Option<String> {
        match percent {
            Some(p) if (0..=100).contains(&p) => Some(SystemFormat::percent(p as f64, culture)),
            _ => None,
        }
    }

    /// Ham pil değerini (bayt) yüzdeye çevirir; 0-100 dışı değer ya da okunamayan değer "pil yok" sayılır.
    pub fn parse_battery(raw: Option<&BatteryRawValue>) -> Option<i32> {
        let value = Self::to_int32(raw?)?;
        if (0..=100).contains(&value) { Some(value) } else { None }
    }

    /// `Convert.ToInt32(raw, InvariantCulture)`; FormatException / InvalidCastException / OverflowException → `None`.
    fn to_int32(raw: &BatteryRawValue) -> Option<i32> {
        match raw {
            BatteryRawValue::Byte(b) => Some(*b as i32),
            BatteryRawValue::Int(i) => Some(*i),
            BatteryRawValue::Long(l) => i32::try_from(*l).ok(),
            BatteryRawValue::Double(d) => {
                // Convert.ToInt32(double): en yakına (yarımda çifte) yuvarlar; aralık dışı / NaN OverflowException.
                if d.is_nan() || *d < -2147483649.0 || *d >= 2147483648.0 {
                    None
                } else {
                    Some(text::round(*d) as i32)
                }
            }
            // int.Parse(NumberStyles.Integer): baştaki/sondaki boşluk, isteğe bağlı işaret, yalnız ASCII rakam.
            BatteryRawValue::Text(s) => s.trim_matches(|c: char| matches!(c, '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ')).parse::<i32>().ok(),
            BatteryRawValue::Boolean(b) => Some(*b as i32),
            BatteryRawValue::Unsupported => None,
        }
    }

    /// Satırın durum anahtarı (Strings.resx).
    pub fn status_key(device: &BluetoothDeviceInfo) -> &'static str {
        if device.is_connected { "BluetoothConnected" } else { "BluetoothNotConnected" }
    }

    pub fn action_for(device: &BluetoothDeviceInfo) -> BluetoothRowAction {
        if !device.is_audio() {
            return BluetoothRowAction::OpenSettings;
        }

        if device.is_connected { BluetoothRowAction::Disconnect } else { BluetoothRowAction::Connect }
    }

    /// Radyo durumu ve servis hazırlığından listenin hangi hâlde gösterileceği.
    pub fn resolve(radio: RadioTileStatus, service_ready: bool, device_count: i32) -> BluetoothListState {
        match radio {
            RadioTileStatus::NoAccess => BluetoothListState::NoAccess,
            RadioTileStatus::NotFound => BluetoothListState::NotFound,
            RadioTileStatus::Off | RadioTileStatus::HardwareOff => BluetoothListState::RadioOff,
            RadioTileStatus::Loading => BluetoothListState::Loading,
            _ => {
                if !service_ready {
                    BluetoothListState::Loading
                } else if device_count == 0 {
                    BluetoothListState::Empty
                } else {
                    BluetoothListState::Ready
                }
            }
        }
    }

    /// Bağlan / kes sonucu için hata metni anahtarı; doğrulanmış başarıda `None`.
    pub fn error_key(result: BluetoothActionResult, connecting: bool) -> Option<&'static str> {
        match result {
            BluetoothActionResult::Confirmed => None,
            BluetoothActionResult::Unsupported => Some("BluetoothErrUnsupported"),
            _ => Some(if connecting { "BluetoothErrConnect" } else { "BluetoothErrDisconnect" }),
        }
    }
}

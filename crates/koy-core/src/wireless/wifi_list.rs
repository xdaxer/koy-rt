use super::wifi_models::{WifiAccess, WifiConnectStatus, WifiListState, WifiNetwork, WifiScanResult};
use crate::text;
use std::collections::{HashMap, HashSet};

/// Satıra tıklayınca ne olacağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WifiRowAction {
    None,
    /// Kayıtlı profille ya da açık ağa doğrudan bağlan.
    ConnectDirect,
    /// Güvenli ve kayıtsız: satırın içinde şifre kutusu açılır.
    AskPassword,
}

/// Wi-Fi listesinin saf mantığı: sinyal çubukları, birleştirme, sıralama, durum ve satır eylemleri.
pub struct WifiList;

impl WifiList {
    pub const MAX_BARS: i32 = 4;

    /// RSSI (dBm) değerinden 0-100 kalite: -100 dBm ve altı 0, -50 dBm ve üstü 100 (doğrusal dönüşüm).
    pub fn quality_from_rssi(dbm: f64) -> i32 {
        if dbm.is_nan() {
            return 0;
        }

        text::clamp(text::round(2.0 * (dbm + 100.0)), 0.0, 100.0) as i32
    }

    /// 0-100 kaliteden 0-4 çubuk. Ağ görünüyorsa (kalite > 0) en az 1 çubuk çizilir.
    pub fn signal_bars(quality: i32) -> i32 {
        if quality <= 0 {
            return 0;
        }
        if quality <= 25 {
            return 1;
        }
        if quality <= 50 {
            return 2;
        }
        if quality <= 75 {
            return 3;
        }
        Self::MAX_BARS
    }

    /// Ham tarama kayıtlarını (her erişim noktası ayrı) listeye çevirir: aynı SSID tek satır olur (en güçlü sinyal,
    /// güvenli/bağlı/kayıtlı bayrakları birleşir), gizli ağlar tek "Gizli ağ" satırında toplanır.
    /// Sıra: bağlı olan, sonra sinyal gücüne göre, gizli ağ en sonda.
    pub fn normalize(raw: &[WifiNetwork]) -> Vec<WifiNetwork> {
        // C# Dictionary (silme yok) ekleme sırasını korur; sıralama kararlı olduğundan eşit anahtarlarda bu sıra görünür.
        let mut by_ssid: Vec<WifiNetwork> = Vec::new();
        let mut index_by_key: HashMap<String, usize> = HashMap::new();
        for network in raw {
            let key = if network.is_hidden() { String::new() } else { network.ssid.clone() };
            match index_by_key.get(&key) {
                Some(&i) => {
                    let merged = Self::merge(&by_ssid[i], network);
                    by_ssid[i] = merged;
                }
                None => {
                    index_by_key.insert(key.clone(), by_ssid.len());
                    by_ssid.push(WifiNetwork { ssid: key, ..network.clone() });
                }
            }
        }

        // OrderBy(IsHidden).ThenByDescending(IsConnected).ThenByDescending(SignalQuality).ThenBy(Ssid, OrdinalIgnoreCase)
        by_ssid.sort_by(|a, b| {
            a.is_hidden()
                .cmp(&b.is_hidden())
                .then_with(|| b.is_connected.cmp(&a.is_connected))
                .then_with(|| b.signal_quality.cmp(&a.signal_quality))
                .then_with(|| text::cmp_ignore_case(&a.ssid, &b.ssid))
        });
        by_ssid
    }

    fn merge(a: &WifiNetwork, b: &WifiNetwork) -> WifiNetwork {
        WifiNetwork::new(
            a.ssid.clone(),
            a.signal_quality.max(b.signal_quality),
            a.is_secured || b.is_secured,
            a.is_connected || b.is_connected,
            a.has_profile || b.has_profile,
        )
    }

    /// Tarama sonucu ve radyo durumundan listenin hangi hâlde gösterileceği.
    pub fn resolve(result: Option<&WifiScanResult>, radio_on: bool) -> WifiListState {
        let Some(result) = result else {
            return WifiListState::Loading;
        };
        if !result.adapter_present {
            return WifiListState::NoAdapter;
        }
        if matches!(result.access, WifiAccess::DeniedByUser | WifiAccess::DeniedBySystem) {
            return WifiListState::NoAccess;
        }
        if !radio_on {
            return WifiListState::RadioOff;
        }
        if result.scan_failed {
            return WifiListState::ScanFailed;
        }

        let networks = Self::normalize(&result.networks);
        if networks.is_empty() {
            return WifiListState::Empty;
        }

        // Konum izni yokken Windows ağları adsız döndürür; bağlı ağın adı bile boş görünür.
        if networks.iter().all(|n| n.is_hidden()) { WifiListState::LocationBlocked } else { WifiListState::Ready }
    }

    pub fn action_for(network: &WifiNetwork) -> WifiRowAction {
        if network.is_hidden() || network.is_connected {
            return WifiRowAction::None;
        }

        if network.has_profile || !network.is_secured { WifiRowAction::ConnectDirect } else { WifiRowAction::AskPassword }
    }

    /// Satırın alt yazısı için kaynak anahtarı (Strings.resx); yazı gerekmiyorsa `None`.
    pub fn status_key(network: &WifiNetwork) -> Option<&'static str> {
        if network.is_connected {
            return Some(if network.is_secured { "WifiConnectedSecure" } else { "WifiConnectedOpen" });
        }

        if network.has_profile { Some("WifiSaved") } else { None }
    }

    /// Bağlanma sonucu için hata metni anahtarı; başarıda `None`.
    pub fn error_key(status: WifiConnectStatus) -> Option<&'static str> {
        match status {
            WifiConnectStatus::Connected => None,
            WifiConnectStatus::WrongPassword => Some("WifiErrPassword"),
            WifiConnectStatus::NotAvailable => Some("WifiErrNotAvailable"),
            WifiConnectStatus::Timeout => Some("WifiErrTimeout"),
            WifiConnectStatus::UnsupportedSecurity => Some("WifiErrUnsupported"),
            WifiConnectStatus::AccessDenied => Some("WifiErrAccess"),
            WifiConnectStatus::Failed => Some("WifiErrFailed"),
        }
    }

    /// Kayıtlı profil adları arasında bu ağ var mı (profil adı çoğu zaman SSID'dir).
    pub fn has_profile(profile_names: &HashSet<String>, ssid: &str) -> bool {
        !ssid.is_empty() && profile_names.contains(ssid)
    }
}

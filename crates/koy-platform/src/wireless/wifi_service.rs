//! `IWifiService` uygulaması: `Windows.Devices.WiFi.WiFiAdapter` (tarama, bağlanma, kesme) ve yalnız kayıtlı profil adları
//! için wlanapi. Servis Wi-Fi sayfası açıkken yaşar; kapanınca adaptör nesneleri bırakılır.

use std::collections::HashMap;
use std::sync::Mutex;

use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::wireless::{IWifiService, WifiAccess, WifiConnectStatus, WifiList, WifiNetwork, WifiScanResult};
use windows::Devices::WiFi::{WiFiAccessStatus, WiFiAdapter, WiFiAvailableNetwork, WiFiConnectionStatus, WiFiReconnectionKind};
use windows::Networking::Connectivity::NetworkAuthenticationType;
use windows::Security::Credentials::PasswordCredential;
use windows::core::HSTRING;

use super::settings_uri::SettingsUri;
use super::wlan_profiles::WlanProfiles;

fn log(source: &str, e: &windows::core::Error) {
    CrashLog::write(source, &CrashError::exception("System.Exception", &e.message(), ""));
}

struct State {
    adapter: Option<WiFiAdapter>,
    access: WifiAccess,
    access_checked: bool,
    // Son taramadaki ağ nesneleri (ConnectAsync nesne ister); SSID başına en güçlü erişim noktası.
    last_networks: HashMap<String, WiFiAvailableNetwork>,
    disposed: bool,
}

// SAFETY: WinRT nesneleri çevik (agile); yalnız kilit altında kullanılır.
unsafe impl Send for State {}

pub struct WifiService {
    gate: Mutex<State>,
}

impl Default for WifiService {
    fn default() -> Self {
        WifiService {
            gate: Mutex::new(State { adapter: None, access: WifiAccess::Unknown, access_checked: false, last_networks: HashMap::new(), disposed: false }),
        }
    }
}

fn map_access(status: WiFiAccessStatus) -> WifiAccess {
    match status {
        WiFiAccessStatus::Allowed => WifiAccess::Allowed,
        WiFiAccessStatus::DeniedByUser => WifiAccess::DeniedByUser,
        WiFiAccessStatus::DeniedBySystem => WifiAccess::DeniedBySystem,
        _ => WifiAccess::Unknown,
    }
}

fn ensure_adapter(st: &mut State) -> windows::core::Result<bool> {
    if !st.access_checked {
        st.access = map_access(WiFiAdapter::RequestAccessAsync()?.join()?);
        st.access_checked = true;
    }
    if st.access != WifiAccess::Allowed {
        return Ok(false);
    }
    if st.adapter.is_some() {
        return Ok(true);
    }
    let adapters = WiFiAdapter::FindAllAdaptersAsync()?.join()?;
    st.adapter = if adapters.Size()? > 0 { Some(adapters.GetAt(0)?) } else { None };
    Ok(st.adapter.is_some())
}

fn connected_ssid(adapter: &WiFiAdapter) -> Option<String> {
    // Bağlı ağ okunamazsa hiçbir satır bağlı gösterilmez; uydurma durum yok.
    let profile = adapter.NetworkAdapter().ok()?.GetConnectedProfileAsync().ok()?.join().ok()?;
    if !profile.IsWlanConnectionProfile().ok()? {
        return None;
    }
    let ssid = profile.WlanConnectionProfileDetails().ok()?.GetConnectedSsid().ok()?.to_string();
    (!ssid.is_empty()).then_some(ssid)
}

fn scan_core(st: &mut State) -> windows::core::Result<WifiScanResult> {
    if !ensure_adapter(st)? {
        let denied = matches!(st.access, WifiAccess::DeniedByUser | WifiAccess::DeniedBySystem);
        return Ok(WifiScanResult::new(st.access, denied, false, Vec::new()));
    }
    let adapter = st.adapter.clone().expect("adapter");
    adapter.ScanAsync()?.join()?;

    let connected = connected_ssid(&adapter);
    let profiles = WlanProfiles::get_names(&adapter.NetworkAdapter()?.NetworkAdapterId()?);

    st.last_networks.clear();
    let mut networks = Vec::new();
    for available in adapter.NetworkReport()?.AvailableNetworks()? {
        if available.IsWiFiDirect()? {
            continue;
        }
        let ssid = available.Ssid()?.to_string();
        let rssi = available.NetworkRssiInDecibelMilliwatts()?;
        let quality = WifiList::quality_from_rssi(rssi);
        let auth = available.SecuritySettings()?.NetworkAuthenticationType()?;
        let secured = !matches!(auth, NetworkAuthenticationType::None | NetworkAuthenticationType::Open80211);
        let is_connected = connected.as_deref().is_some_and(|c| !ssid.is_empty() && ssid == c);

        networks.push(WifiNetwork::new(ssid.clone(), quality, secured, is_connected, WifiList::has_profile(&profiles, &ssid)));

        if !ssid.is_empty() {
            let stronger = match st.last_networks.get(&ssid) {
                Some(existing) => rssi > existing.NetworkRssiInDecibelMilliwatts().unwrap_or(f64::MIN),
                None => true,
            };
            if stronger {
                st.last_networks.insert(ssid, available);
            }
        }
    }
    Ok(WifiScanResult::new(WifiAccess::Allowed, true, false, networks))
}

impl WifiService {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.gate.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn dispose(&self) {
        let mut st = self.lock();
        if st.disposed {
            return;
        }
        st.disposed = true;
        st.adapter = None;
        st.last_networks.clear();
    }
}

impl IWifiService for WifiService {
    fn scan(&self) -> WifiScanResult {
        let mut st = self.lock();
        if st.disposed {
            return WifiScanResult::new(WifiAccess::Unknown, true, true, Vec::new());
        }
        match scan_core(&mut st) {
            Ok(r) => r,
            Err(e) => {
                // Adaptör çıkarıldı ya da yığın yanıt vermiyor: bir sonraki taramada yeniden aranır.
                log("WifiService.ScanAsync", &e);
                st.adapter = None;
                st.access_checked = false;
                WifiScanResult::new(st.access, true, true, Vec::new())
            }
        }
    }

    fn connect(&self, ssid: &str, password: Option<&str>) -> WifiConnectStatus {
        let st = self.lock();
        let (Some(adapter), Some(network)) = (st.adapter.as_ref(), st.last_networks.get(ssid)) else {
            return WifiConnectStatus::NotAvailable;
        };
        if st.disposed {
            return WifiConnectStatus::NotAvailable;
        }
        // Şifre yalnız bu çağrıda kimlik bilgisi nesnesine konur; saklanmaz ve günlüğe yazılmaz.
        let result = (|| -> windows::core::Result<WiFiConnectionStatus> {
            let r = match password.filter(|p| !p.is_empty()) {
                None => adapter.ConnectAsync(network, WiFiReconnectionKind::Automatic)?.join()?,
                Some(p) => {
                    let credential = PasswordCredential::new()?;
                    credential.SetPassword(&HSTRING::from(p))?;
                    adapter.ConnectWithPasswordCredentialAsync(network, WiFiReconnectionKind::Automatic, &credential)?.join()?
                }
            };
            r.ConnectionStatus()
        })();
        match result {
            Ok(WiFiConnectionStatus::Success) => WifiConnectStatus::Connected,
            Ok(WiFiConnectionStatus::InvalidCredential) => WifiConnectStatus::WrongPassword,
            Ok(WiFiConnectionStatus::NetworkNotAvailable) => WifiConnectStatus::NotAvailable,
            Ok(WiFiConnectionStatus::Timeout) => WifiConnectStatus::Timeout,
            Ok(WiFiConnectionStatus::UnsupportedAuthenticationProtocol) => WifiConnectStatus::UnsupportedSecurity,
            Ok(WiFiConnectionStatus::AccessRevoked) => WifiConnectStatus::AccessDenied,
            Ok(_) => WifiConnectStatus::Failed,
            Err(e) => {
                // Hata iletisi şifre içermez; yine de parametreler günlüğe eklenmez.
                log("WifiService.ConnectAsync", &e);
                WifiConnectStatus::Failed
            }
        }
    }

    fn disconnect(&self) {
        if let Some(adapter) = self.lock().adapter.as_ref()
            && let Err(e) = adapter.Disconnect()
        {
            log("WifiService.Disconnect", &e);
        }
    }

    fn open_settings(&self) {
        SettingsUri::open("ms-settings:network-wifi");
    }

    fn open_location_settings(&self) {
        SettingsUri::open("ms-settings:privacy-location");
    }
}

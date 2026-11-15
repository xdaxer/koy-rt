//! Karşılığı: `Platform/Network/NetworkService.cs`.

use std::sync::{Arc, Mutex, Weak};

use koy_core::event::Event;
use koy_core::network::{INetworkService, NetworkKind, NetworkStatus};
use koy_core::text;
use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation, NetworkStatusChangedEventHandler};

struct Inner {
    current: Mutex<NetworkStatus>,
    changed: Event<NetworkStatus>,
}

/// İnternet bağlantısı profilini (WinRT NetworkInformation) izler. Yoklama yok: Windows değişince bildirir.
pub struct NetworkService {
    inner: Arc<Inner>,
    token: Mutex<Option<i64>>,
}

impl NetworkService {
    pub fn new() -> Self {
        let inner = Arc::new(Inner { current: Mutex::new(read()), changed: Event::new() });

        let weak: Weak<Inner> = Arc::downgrade(&inner);
        let handler = NetworkStatusChangedEventHandler::new(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.on_network_status_changed();
            }
            Ok(())
        });
        let token = NetworkInformation::NetworkStatusChanged(&handler).ok();

        NetworkService { inner, token: Mutex::new(token) }
    }
}

impl Default for NetworkService {
    fn default() -> Self {
        Self::new()
    }
}

impl Inner {
    fn on_network_status_changed(&self) {
        let next = read();
        {
            let mut current = self.current.lock().unwrap();
            if *current == next {
                return;
            }
            *current = next.clone();
        }
        self.changed.invoke(&next);
    }
}

/// Windows'un internet bağlantı profilinden durumu okur; her hata "bağlı değil" sayılır.
fn read() -> NetworkStatus {
    read_core().unwrap_or_else(|_| NetworkStatus::disconnected())
}

fn read_core() -> windows::core::Result<NetworkStatus> {
    // Profil yoksa (null) çağrı hata döner: C#'taki `profile is null → Disconnected`.
    let profile = NetworkInformation::GetInternetConnectionProfile()?;

    let has_internet = profile.GetNetworkConnectivityLevel()? == NetworkConnectivityLevel::InternetAccess;
    let bars = profile.GetSignalBars().ok().and_then(|r| r.Value().ok()).map_or(0, |b| b as i32);
    let profile_name = profile.ProfileName()?.to_string();
    let name = if text::is_null_or_white_space(Some(&profile_name)) { None } else { Some(profile_name) };

    if profile.IsWlanConnectionProfile()? {
        return Ok(NetworkStatus::with_name(NetworkKind::WiFi, has_internet, bars, name.as_deref()));
    }

    if profile.IsWwanConnectionProfile()? {
        return Ok(NetworkStatus::with_name(NetworkKind::Cellular, has_internet, bars, name.as_deref()));
    }

    Ok(NetworkStatus::with_name(NetworkKind::Ethernet, has_internet, 0, name.as_deref()))
}

impl INetworkService for NetworkService {
    fn current(&self) -> NetworkStatus {
        self.inner.current.lock().unwrap().clone()
    }

    fn changed(&self) -> &Event<NetworkStatus> {
        &self.inner.changed
    }
}

impl Drop for NetworkService {
    fn drop(&mut self) {
        if let Some(token) = self.token.lock().unwrap().take() {
            let _ = NetworkInformation::RemoveNetworkStatusChanged(token);
        }
    }
}

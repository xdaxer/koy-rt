use crate::event::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NetworkKind {
    #[default]
    None,
    Ethernet,
    WiFi,
    Cellular,
}

/// İnternete çıkan bağlantının türü, erişimi, (kablosuzda) 0-5 arası sinyal çubuğu ve ağ adı (Wi-Fi SSID'si /
/// bağlantı profili adı; bilinmiyorsa `None`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetworkStatus {
    pub kind: NetworkKind,
    pub has_internet: bool,
    pub signal_bars: i32,
    pub name: Option<String>,
}

impl NetworkStatus {
    /// `new NetworkStatus(kind, hasInternet, signalBars)`: ad verilmemiş (C# varsayılanı `null`).
    pub fn new(kind: NetworkKind, has_internet: bool, signal_bars: i32) -> Self {
        NetworkStatus { kind, has_internet, signal_bars, name: None }
    }

    /// `new NetworkStatus(kind, hasInternet, signalBars, name)`.
    pub fn with_name(kind: NetworkKind, has_internet: bool, signal_bars: i32, name: Option<&str>) -> Self {
        NetworkStatus { kind, has_internet, signal_bars, name: name.map(str::to_owned) }
    }

    /// `NetworkStatus.Disconnected`.
    pub fn disconnected() -> Self {
        NetworkStatus::new(NetworkKind::None, false, 0)
    }
}

/// Ağ bağlantısı durumu sözleşmesi. Dock'un durum alanı Windows görev çubuğundaki ağ simgesinin yerini tutar.
/// (`IDisposable` → uygulayıcının `Drop`'u.)
pub trait INetworkService: Send + Sync {
    fn current(&self) -> NetworkStatus;

    fn changed(&self) -> &Event<NetworkStatus>;
}

use crate::text;

/// Wi-Fi taramasına erişim durumu (Windows 11 24H2'den beri SSID görmek konum iznine bağlı).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WifiAccess {
    Unknown,
    Allowed,
    DeniedByUser,
    DeniedBySystem,
}

/// Tarama sonucunda listede ne gösterileceği.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WifiListState {
    Loading,
    Ready,
    /// Wi-Fi radyosu kapalı.
    RadioOff,
    NoAdapter,
    /// Wi-Fi erişimi reddedildi.
    NoAccess,
    /// Ağlar görünüyor ama adları gizli: masaüstü uygulamalarına konum izni verilmemiş.
    LocationBlocked,
    /// Tarama tamamlandı, çevrede ağ yok.
    Empty,
    /// Tarama hata verdi.
    ScanFailed,
}

/// Listedeki tek ağ. `ssid` boşsa gizli ağdır. `signal_quality` 0-100.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal_quality: i32,
    pub is_secured: bool,
    pub is_connected: bool,
    pub has_profile: bool,
}

impl WifiNetwork {
    pub fn new(ssid: impl Into<String>, signal_quality: i32, is_secured: bool, is_connected: bool, has_profile: bool) -> Self {
        WifiNetwork { ssid: ssid.into(), signal_quality, is_secured, is_connected, has_profile }
    }

    pub fn is_hidden(&self) -> bool {
        text::is_null_or_white_space(Some(&self.ssid))
    }
}

/// Bir taramanın ham sonucu; durum `WifiList::resolve` ile belirlenir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiScanResult {
    pub access: WifiAccess,
    pub adapter_present: bool,
    pub scan_failed: bool,
    pub networks: Vec<WifiNetwork>,
}

impl WifiScanResult {
    pub fn new(access: WifiAccess, adapter_present: bool, scan_failed: bool, networks: Vec<WifiNetwork>) -> Self {
        WifiScanResult { access, adapter_present, scan_failed, networks }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WifiConnectStatus {
    Connected,
    WrongPassword,
    NotAvailable,
    Timeout,
    UnsupportedSecurity,
    AccessDenied,
    Failed,
}

impl WifiConnectStatus {
    /// C# `Enum.GetValues<WifiConnectStatus>()` karşılığı (bildirim sırasıyla).
    pub const ALL: [WifiConnectStatus; 7] = [
        WifiConnectStatus::Connected,
        WifiConnectStatus::WrongPassword,
        WifiConnectStatus::NotAvailable,
        WifiConnectStatus::Timeout,
        WifiConnectStatus::UnsupportedSecurity,
        WifiConnectStatus::AccessDenied,
        WifiConnectStatus::Failed,
    ];
}

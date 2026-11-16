use super::i_network_service::{NetworkKind, NetworkStatus};

/// Arayüzde gösterilecek ağ durumunun kısa sınıfı (metni ve simgeyi bu belirler).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NetworkDisplay {
    #[default]
    Disconnected,
    NoInternet,
    WiFi,
    Ethernet,
    Cellular,
}

/// Ham `NetworkStatus` değerini çentikte gösterilecek duruma çevirir. Saf mantık: metin ve simge
/// seçimi App tarafında, karar burada (kural #11).
pub struct NetworkSummary;

impl NetworkSummary {
    pub fn describe(status: &NetworkStatus) -> NetworkDisplay {
        if status.kind == NetworkKind::None {
            NetworkDisplay::Disconnected
        } else if !status.has_internet {
            NetworkDisplay::NoInternet
        } else if status.kind == NetworkKind::WiFi {
            NetworkDisplay::WiFi
        } else if status.kind == NetworkKind::Cellular {
            NetworkDisplay::Cellular
        } else {
            NetworkDisplay::Ethernet
        }
    }

    /// Kablosuz ağın adı; boşluklar kırpılır, ad yoksa ya da Wi-Fi değilse `None`.
    pub fn wifi_name(status: &NetworkStatus) -> Option<String> {
        if status.kind != NetworkKind::WiFi {
            return None;
        }

        let name = status.name.as_deref().map(crate::text::trim);
        match name {
            Some(n) if !n.is_empty() => Some(n.to_owned()),
            _ => None,
        }
    }

    /// İnternete çıkabilen bir bağlantı var mı (simge normal mi, üstü çizili mi çizilecek).
    pub fn is_connected(status: &NetworkStatus) -> bool {
        !matches!(Self::describe(status), NetworkDisplay::Disconnected | NetworkDisplay::NoInternet)
    }
}

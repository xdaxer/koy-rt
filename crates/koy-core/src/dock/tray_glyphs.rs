use crate::network::{NetworkKind, NetworkStatus};
use crate::text;

/// Dock durum alanındaki simgeler için Segoe Fluent Icons karakterleri. Windows 11 görev çubuğuyla aynı yazı tipi.
pub struct TrayGlyphs;

impl TrayGlyphs {
    pub const WIFI_FULL: &'static str = "\u{E701}";
    pub const ETHERNET: &'static str = "\u{E839}";
    pub const OFFLINE: &'static str = "\u{F384}";
    pub const NOTIFICATIONS: &'static str = "\u{EA8F}";

    const MUTE: &'static str = "\u{E74F}";
    const BATTERY0: u32 = 0xEBA0; // EBA0..EBAA: %0..%100
    const BATTERY_CHARGING0: u32 = 0xEBAB; // EBAB..EBB5: şarjda %0..%100

    /// Ağ simgesi. Kablosuzda tam simge soluk çizilir, üstüne `wifi_level` dolu çubukları koyar.
    /// Bağlı ama internet yoksa Windows gibi "bağlantı yok" küresi gösterilir.
    pub fn network(status: &NetworkStatus) -> &'static str {
        if !status.has_internet {
            return Self::OFFLINE;
        }
        match status.kind {
            NetworkKind::WiFi | NetworkKind::Cellular => Self::WIFI_FULL,
            NetworkKind::Ethernet => Self::ETHERNET,
            _ => Self::OFFLINE,
        }
    }

    /// Kablosuz sinyalin dolu kısmı; kablosuz değilse ya da sinyal tamsa boş (tam simge yeterli).
    pub fn wifi_level(status: &NetworkStatus) -> &'static str {
        if !status.has_internet || !matches!(status.kind, NetworkKind::WiFi | NetworkKind::Cellular) {
            return "";
        }

        // Windows 0-5 çubuk verir; simgede 3 kademe + tam var.
        match status.signal_bars {
            i32::MIN..=1 => "\u{E872}",
            2 => "\u{E873}",
            3 | 4 => "\u{E874}",
            _ => "",
        }
    }

    pub fn volume(volume: f32, muted: bool) -> &'static str {
        if muted || volume <= 0.0 {
            return Self::MUTE;
        }

        if volume < 0.34 {
            "\u{E993}"
        } else if volume < 0.67 {
            "\u{E994}"
        } else {
            "\u{E995}"
        }
    }

    pub fn battery(percent: i32, charging: bool) -> String {
        // Math.Round(double): bankacı yuvarlaması.
        let level = text::clamp(text::round(percent as f64 / 10.0) as i32, 0, 10) as u32;
        let base = if charging { Self::BATTERY_CHARGING0 } else { Self::BATTERY0 };
        char::from_u32(base + level).map(String::from).unwrap_or_default()
    }
}

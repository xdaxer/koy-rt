use serde::{Deserialize, Serialize};

/// Yeni bağlanan (daha önce hiç görülmemiş) ekranda çentik ve dock'un durumu.
/// Ayar dosyasında metin olarak yazılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NewScreenMode {
    /// Çentik ve dock açık.
    #[default]
    NotchAndDock,
    /// Yalnız çentik açık.
    NotchOnly,
    /// İkisi de kapalı.
    None,
}

/// Tek bir monitörün ayarı. Anahtar `DisplayMonitor::id` (kararlı kimlik) olarak
/// `AppSettings.Monitors` sözlüğünde durur. Ana ekran bilgisi saklanmaz: salt bilgidir, her açılışta sorgulanır.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MonitorSettings {
    pub notch_enabled: bool,
    pub dock_enabled: bool,
    /// Son görülen dostça ad; ekran ayrıldığında da ayarın kime ait olduğu belli olsun diye.
    pub name: Option<String>,
}

impl Default for MonitorSettings {
    fn default() -> Self {
        MonitorSettings { notch_enabled: true, dock_enabled: true, name: None }
    }
}

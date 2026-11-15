#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BluetoothDeviceKind {
    #[default]
    Other,
    Headphones,
    /// Hoparlör ve diğer ses cihazları.
    Speaker,
    Mouse,
    Keyboard,
    Phone,
    Tablet,
}

/// Eşleşmiş bir Bluetooth cihazı. `container_id` aynı fiziksel cihazın klasik ve LE kayıtlarını birleştirir.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BluetoothDeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: BluetoothDeviceKind,
    pub is_connected: bool,
    pub battery_percent: Option<i32>,
    pub container_id: Option<String>,
    pub is_low_energy: bool,
}

impl BluetoothDeviceInfo {
    /// C# kurucusunun zorunlu parametreleri; `ContainerId` varsayılanı `null`, `IsLowEnergy` varsayılanı `false`.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: BluetoothDeviceKind,
        is_connected: bool,
        battery_percent: Option<i32>,
    ) -> Self {
        BluetoothDeviceInfo {
            id: id.into(),
            name: name.into(),
            kind,
            is_connected,
            battery_percent,
            container_id: None,
            is_low_energy: false,
        }
    }

    /// `ContainerId` verilmiş kurucu çağrısı.
    pub fn with_container_id(mut self, container_id: impl Into<String>) -> Self {
        self.container_id = Some(container_id.into());
        self
    }

    /// `IsLowEnergy` verilmiş kurucu çağrısı.
    pub fn with_low_energy(mut self, is_low_energy: bool) -> Self {
        self.is_low_energy = is_low_energy;
        self
    }

    /// Ses cihazı mı (kulaklık, hoparlör): Koy yalnız bunlar için bağlan / bağlantıyı kes sunar.
    pub fn is_audio(&self) -> bool {
        matches!(self.kind, BluetoothDeviceKind::Headphones | BluetoothDeviceKind::Speaker)
    }
}

/// Bağlan / kes isteğinin sonucu; "bağlandı" yalnız gerçek bağlantı durumu doğrulayınca söylenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BluetoothActionResult {
    /// İstenen duruma ulaşıldığı bağlantı durumundan okundu.
    #[default]
    Confirmed,
    /// İstek sürücüye iletildi ama süre içinde istenen duruma ulaşıldığı görülmedi.
    NotConfirmed,
    /// Bu cihaz için Koy'dan bağlanma yolu yok (ses uç noktası bulunamadı).
    Unsupported,
    Failed,
}

/// Bluetooth listesinin görünümü: radyo kapalı, bulunamadı, izin yok, boş ya da cihazlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BluetoothListState {
    #[default]
    Loading,
    Ready,
    /// Eşleşmiş cihaz yok.
    Empty,
    RadioOff,
    NotFound,
    NoAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BluetoothRowAction {
    #[default]
    Connect,
    Disconnect,
    /// Ses dışı cihaz (fare, klavye…): Koy bağlayamaz, Windows Bluetooth ayarları açılır.
    OpenSettings,
}

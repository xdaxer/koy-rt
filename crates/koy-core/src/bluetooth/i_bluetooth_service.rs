use super::bluetooth_models::{BluetoothActionResult, BluetoothDeviceInfo};
use crate::event::Event;

/// Eşleşmiş Bluetooth cihazlarının listesi, bağlantı durumu ve pili. Servis yaşadığı sürece cihaz izleyicisi çalışır
/// (olay tabanlı, yoklama yok). Çentik bağlan/kes göstergesi için servisi sürekli tutar; liste sayfaları yalnız görünürken alır.
/// (`IDisposable` karşılığı uygulayan tipin `Drop`'udur; `Task` dönen metotlar bloklayan çağrıdır.)
pub trait IBluetoothService: Send + Sync {
    /// Şu anki cihazlar (ham; birleştirme ve sıralama [`super::BluetoothList`] ile).
    fn devices(&self) -> Vec<BluetoothDeviceInfo>;

    /// Başlangıç taraması bitti mi (liste güvenilir).
    fn is_ready(&self) -> bool;

    /// Cihaz eklendi, çıktı ya da özelliği (bağlantı, pil) değişti. Arka plan iş parçacığından gelir.
    fn devices_changed(&self) -> &Event<()>;

    /// Ses cihazına bağlanmayı dener ve sonucu bağlantı durumundan okur. (C#: `ConnectAsync`)
    fn connect(&self, device_id: &str) -> BluetoothActionResult;

    /// Ses cihazının bağlantısını keser ve sonucu bağlantı durumundan okur. (C#: `DisconnectAsync`)
    fn disconnect(&self, device_id: &str) -> BluetoothActionResult;

    /// GATT pil okumasını açar ya da kapatır. Yalnız Bluetooth listesini izleyen bir sayfa (Ana, Kontrol) açıkken açık olmalı;
    /// servis çentikte kalıcı tutulsa da okuma yapılmaz (AGENTS.md kural #5).
    fn set_battery_reads_enabled(&self, enabled: bool);

    /// Windows'ta Bluetooth ayarlarını açar.
    fn open_settings(&self);
}

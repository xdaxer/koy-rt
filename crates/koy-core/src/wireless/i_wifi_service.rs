use super::wifi_models::{WifiConnectStatus, WifiScanResult};

/// Wi-Fi ağ listesi, bağlanma ve bağlantıyı kesme sözleşmesi. Servis yalnız Wi-Fi sayfası açıkken yaşar;
/// tarama zamanlayıcısı servisin değil arayüzün işidir (sayfa kapanınca durur).
///
/// C#'taki `Task` dönen metotlar burada bloklayan çağrıdır (Core'da zaman uyumsuz çalışma zamanı yok);
/// çağıran bunları bir çalışan iş parçacığında çalıştırır. `IDisposable` karşılığı uygulayan tipin `Drop`'udur.
pub trait IWifiService: Send + Sync {
    /// Bir tarama yapar ve görünen ağları döndürür. Aynı anda gelen çağrılar sıraya girer. (C#: `ScanAsync`)
    fn scan(&self) -> WifiScanResult;

    /// Ağa bağlanır. `password` `None` ise kayıtlı profil (ya da açık ağ) kullanılır.
    /// Şifre yalnız bu çağrı süresince bellekte durur; günlüğe ve diske yazılmaz. (C#: `ConnectAsync`)
    fn connect(&self, ssid: &str, password: Option<&str>) -> WifiConnectStatus;

    /// Bağlı ağdan ayrılır.
    fn disconnect(&self);

    /// Windows'ta Wi-Fi ayarları sayfasını açar.
    fn open_settings(&self);

    /// Windows'ta konum gizlilik sayfasını açar.
    fn open_location_settings(&self);
}

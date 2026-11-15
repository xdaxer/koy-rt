use super::BrightnessSourceKind;
use crate::event::Event;

/// Ekran parlaklığı kontrol sözleşmesi.
/// Dahili ekranlar (WMI) veya harici monitörler (DDC/CI) için.
/// Desteklenmiyorsa `is_supported` false döner. Kaynak ekran düzeni değişince [`IBrightnessService::refresh`] ile yeniden belirlenir.
/// C# `IDisposable` karşılığı uygulayan tipin `Drop`'udur.
pub trait IBrightnessService: Send + Sync {
    fn is_supported(&self) -> bool;
    fn brightness(&self) -> i32;

    /// Şu an kullanılan yol (`BrightnessSourceKind::None` ise `is_supported` false).
    fn source(&self) -> BrightnessSourceKind;

    /// Parlaklık Koy dışından değişti (Fn tuşları, Windows ayarları). `event EventHandler<int>`.
    fn brightness_changed(&self) -> &Event<i32>;

    /// Kaynak ya da destek durumu değişti (Refresh sonrası, ya da ayar çağrısı başarısız olunca).
    /// Arka plan iş parçacığından gelir. `event EventHandler`.
    fn availability_changed(&self) -> &Event<()>;

    /// Çağıranı bloklamaz: değer arka planda uygulanır, art arda gelenler birleşir (yalnız son değer yazılır).
    /// Koy içinden verilen değer için `brightness_changed` yükselmez.
    fn set_brightness(&self, brightness: i32);

    /// Kaynağı ve geçerli parlaklığı yeniden belirler. WMI/DDC sorgusu yapar: UI iş parçacığından çağrılmaz.
    fn refresh(&self);
}

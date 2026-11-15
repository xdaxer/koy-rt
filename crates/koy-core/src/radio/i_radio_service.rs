use crate::event::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadioKind {
    WiFi,
    Bluetooth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadioState {
    /// Radyolar henüz taranmadı.
    Unknown,
    On,
    Off,
    /// Donanım anahtarı ya da sürücü radyoyu kapattı; yazılımdan açılamaz.
    Disabled,
    /// Tarandı, bu türde radyo yok (adaptör takılı değil ya da erişim yok).
    NotPresent,
}

/// Windows'un "Uygulamaların radyoları denetlemesine izin ver" gizlilik iznine karşılık gelir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadioAccess {
    Unknown,
    Allowed,
    DeniedByUser,
    DeniedBySystem,
}

/// Wi-Fi ve Bluetooth radyo durumu ve açma/kapatma sözleşmesi.
///
/// C#'taki `Task` dönen metotlar burada bloklayan çağrıdır (Core'da zaman uyumsuz çalışma zamanı yok);
/// çağıran bunları bir çalışan iş parçacığında çalıştırır. `IDisposable` karşılığı uygulayan tipin `Drop`'udur.
pub trait IRadioService: Send + Sync {
    fn access(&self) -> RadioAccess;
    fn wifi_state(&self) -> RadioState;
    fn bluetooth_state(&self) -> RadioState;

    /// C# `event EventHandler<RadioStateChangedEventArgs>? StateChanged`.
    fn state_changed(&self) -> &Event<RadioStateChangedEventArgs>;

    /// İzni yeniden sorar, radyoları yeniden bulur ve her tür için `state_changed` yayınlar.
    /// USB adaptör takılıp çıkarıldığında ya da izin sonradan verildiğinde güncel durumu getirir.
    /// Aynı anda gelen çağrılar tek taramayı paylaşır. (C#: `RefreshAsync`)
    fn refresh(&self);

    /// C#: `SetStateAsync`.
    fn set_state(&self, kind: RadioKind, enable: bool) -> bool;

    /// C#: `ToggleAsync`.
    fn toggle(&self, kind: RadioKind) -> bool;

    /// Windows'ta ilgili radyonun ayar sayfasını açar (ağ listesi, cihaz eşleştirme orada).
    fn open_settings(&self, kind: RadioKind);

    /// Windows'ta radyo denetimi gizlilik sayfasını açar.
    fn open_privacy_settings(&self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RadioStateChangedEventArgs {
    pub kind: RadioKind,
    pub state: RadioState,
}

impl RadioStateChangedEventArgs {
    pub fn new(kind: RadioKind, state: RadioState) -> Self {
        RadioStateChangedEventArgs { kind, state }
    }
}

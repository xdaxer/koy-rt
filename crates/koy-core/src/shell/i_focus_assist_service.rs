use crate::event::Event;

/// Windows Odak Yardımı / Rahatsız Etme sözleşmesi. (`IDisposable` → uygulayıcının `Drop`'u.)
pub trait IFocusAssistService: Send + Sync {
    fn is_active(&self) -> bool;

    /// Odaklanma oturumu / Sessiz saatler / sunum modu açık mı. Windows 11 "Rahatsız etmeyin" düğmesi tek başına
    /// okunamaz (false döner). Tek seferlik okuma (zamanlayıcı yok); çağıran ihtiyaç anında sorar.
    fn is_do_not_disturb(&self) -> bool;

    /// Odaklanma oturumu açılıp kapanınca `is_do_not_disturb` değeriyle tetiklenir (olay tabanlı, arka plan iş
    /// parçacığından olabilir). Rahatsız etmeyin düğmesinin kendisi için olay yoktur: çağıran açılışta yeniden okur.
    fn state_changed(&self) -> &Event<bool>;

    fn open_settings(&self);
}

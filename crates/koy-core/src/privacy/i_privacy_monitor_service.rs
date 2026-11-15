//! Karşılığı: `Privacy/IPrivacyMonitorService.cs`.

use super::capture_usage_policy::PrivacyState;
use crate::event::Event;

/// Kamera ve mikrofon aktif kullanım durumunu izleyen servis sözleşmesi.
///
/// C#'taki `IDisposable`, uygulayan tipin `Drop`'udur.
pub trait IPrivacyMonitorService {
    fn current_state(&self) -> PrivacyState;

    /// `event Action<PrivacyState>? StateChanged`.
    fn state_changed(&self) -> &Event<PrivacyState>;

    fn start(&self);

    fn stop(&self);
}

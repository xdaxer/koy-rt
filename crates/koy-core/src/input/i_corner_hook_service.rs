//! Karşılığı: `Input/ICornerHookService.cs`.

use super::hot_corner_math::{CornerHit, ScreenRect};
use crate::event::Event;

/// Ekran köşesi takibi için fare kancası. Yoklama yok: imleç hareket ettikçe Windows haber verir,
/// abone yalnız imleç bir köşeye girince ya da köşeden çıkınca çağrılır.
pub trait ICornerHookService: Send + Sync {
    /// İmlecin bulunduğu köşe değişti; köşeden çıkınca `None`. Arayüz iş parçacığında çağrılır. (C#: `CornerChanged`)
    fn corner_changed(&self) -> &Event<Option<CornerHit>>;

    /// İzlenecek ekranlar (fiziksel piksel, `CornerHit::screen_index` bu sıraya göre). Liste boşsa kanca kaldırılır.
    /// Kanca kurulamazsa `false` döner.
    fn set_screens(&self, screens: &[ScreenRect], corner_size_px: i32) -> bool;

    /// C#: `IDisposable.Dispose`.
    fn dispose(&self);
}

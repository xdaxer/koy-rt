//! Karşılığı: `Input/InputAction.cs`.

use serde::{Deserialize, Serialize};

/// Kısayol, köşe ya da fare hareketiyle tetiklenebilen eylemler. Kapalı liste.
/// Ayar dosyasında ad olarak yazılır (sayı değil): mevcut adlar değiştirilmez, yeni eylem eklenebilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum InputAction {
    #[default]
    ShowDesktop,
    OpenSearch,
    OpenNotch,
    OpenKoyWindow,
    ToggleWifi,
    ToggleBluetooth,
    ToggleDoNotDisturb,
    SwitchProfile,
    MoveNotchToNextMonitor,
    MoveDockToNextMonitor,

    /// Kullanıcının tanımladığı bir `CustomAction`; hangisi olduğu binding'de tutulur.
    RunCustomAction,

    /// Dock'u göster/gizle (ayar da kaydedilir). Sona eklendi: mevcut adlar ve sıra değişmez.
    ToggleDock,
}

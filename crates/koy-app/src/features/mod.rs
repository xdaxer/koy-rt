//! Karşılığı: `koy/src/App/Features/`.

pub mod actions;
pub mod calendar;
pub mod clipboard_panel;
pub mod focus;
pub mod menu;
pub mod music;
pub mod shelf;
pub mod shortcuts;
pub mod status;
pub mod system_info;

/// Uygulama özelliği (Notch, Dock). C# `IFeature`.
pub trait Feature {
    fn start(&self);

    /// Pencereleri kapatır, servisleri durdurur, kancaları çözer. Birden fazla çağrılabilir.
    fn stop(&self);
}

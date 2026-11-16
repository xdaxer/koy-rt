//! Karşılığı: `koy/src/App/Features/`.

pub mod actions;
pub mod menu;
pub mod shortcuts;

/// Uygulama özelliği (Notch, Dock). C# `IFeature`.
pub trait Feature {
    fn start(&self);

    /// Pencereleri kapatır, servisleri durdurur, kancaları çözer. Birden fazla çağrılabilir.
    fn stop(&self);
}

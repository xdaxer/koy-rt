use super::dock_edge::DockEdge;

/// Dock penceresini Windows AppBar olarak kaydederek ekranın altında çalışma alanı ayırma sözleşmesi.
/// (`IDisposable` → uygulayıcının `Drop`'u.) `dpi_scale` C# varsayılanı 1.0, `edge` varsayılanı `DockEdge::Bottom`.
pub trait IAppBarService: Send + Sync {
    /// Pencereyi ekranın kenarında AppBar olarak kaydeder ve çalışma alanını ayırır.
    fn register(&self, window_handle: isize, height_dip: f64, dpi_scale: f64, edge: DockEdge);

    /// Ekran veya çözünürlük değiştiğinde AppBar konumunu günceller.
    fn update_position(&self, window_handle: isize, height_dip: f64, dpi_scale: f64, edge: DockEdge);

    /// Kaydı sıfırdan yapar. Explorer yeniden başlayınca eski kayıtlar kaybolur; çalışma alanı yeniden ayrılmalıdır.
    fn reregister(&self, window_handle: isize, height_dip: f64, dpi_scale: f64, edge: DockEdge);

    /// AppBar kaydını kaldırır ve masaüstü çalışma alanını eski haline getirir.
    fn unregister(&self, window_handle: isize);
}

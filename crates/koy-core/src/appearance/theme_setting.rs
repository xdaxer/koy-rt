use super::AppTheme;

/// Çözümlenmiş tema: ayar "Sistem" olsa bile ekrana bunlardan biri uygulanır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolvedTheme {
    Dark,
    Light,
}

/// Theme ayarının (`AppTheme`: Sistem / Koyu / Açık) çözümlenmesi. Eski sürümlerin yazdığı karışık değerler
/// ("Koyu", "sistem"…) ayar göçünde enum adına çevrilir.
pub struct ThemeSetting;

impl ThemeSetting {
    /// `value`: ayardaki tema. `system_uses_light_theme`: Windows uygulama teması açık mı (yalnız "Sistem" seçiliyken kullanılır).
    pub fn resolve(value: AppTheme, system_uses_light_theme: bool) -> ResolvedTheme {
        match value {
            AppTheme::System => {
                if system_uses_light_theme {
                    ResolvedTheme::Light
                } else {
                    ResolvedTheme::Dark
                }
            }
            AppTheme::Light => ResolvedTheme::Light,
            AppTheme::Dark => ResolvedTheme::Dark,
        }
    }
}

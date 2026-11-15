use serde::{Deserialize, Serialize};

/// Tam genişlik dock'ta uygulama ikonlarının yeri. Başlat düğmesi ve sağ bölüm yerinde kalır, yalnız ikon şeridi kayar.
/// Kapsülde kullanılmaz: kapsül içeriği kadar geniştir, hizalanacak boşluğu yoktur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockIconAlignment {
    #[default]
    Left,
    Center,
    Right,
}

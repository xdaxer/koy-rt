use serde::{Deserialize, Serialize};

// Ad settings.json'a yazılır (UseStringEnumConverter): yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ, yalnız yeni değer eklenir.

/// Dock'un ekranın hangi kenarına oturduğu. Varsayılan alt kenardır (bugünkü davranış).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockEdge {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

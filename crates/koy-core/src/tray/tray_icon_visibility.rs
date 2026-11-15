use serde::{Deserialize, Serialize};

// Bu dosyadaki enum adları settings.json'a yazılır: yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Tepsi simgesinin dock'taki yeri: doğrudan gösterilir, taşma menüsünde durur ya da hiç gösterilmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrayIconVisibility {
    Shown,
    Overflow,
    Hidden,
}

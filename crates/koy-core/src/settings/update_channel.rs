//! Karşılığı: `koy/src/Core/Settings/UpdateChannel.cs`.

// Enum adları settings.json'a yazılır: bir kez yayımlandıktan sonra ADLARI DEĞİŞTİRİLMEZ (yalnız yeni değer eklenir).

/// Güncelleme kanalı. Eski dosyalarda "Kararlı" / "Önizleme" metniydi (dile bağlı); göçte ada çevrilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UpdateChannel {
    #[default]
    Stable,
    Preview,
}

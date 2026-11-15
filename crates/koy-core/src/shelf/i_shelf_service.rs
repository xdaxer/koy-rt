use crate::event::Event;
use crate::time::DateTimeOffset;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShelfItemKind {
    File,
    Directory,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfItem {
    pub id: String,
    pub display_name: String,
    pub path_or_content: String,
    pub kind: ShelfItemKind,
    pub added_at: DateTimeOffset,
}

impl ShelfItem {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        path_or_content: impl Into<String>,
        kind: ShelfItemKind,
        added_at: DateTimeOffset,
    ) -> Self {
        ShelfItem { id: id.into(), display_name: display_name.into(), path_or_content: path_or_content.into(), kind, added_at }
    }
}

/// Notch raf (shelf) sözleşmesi.
/// Dosya ve metin yollarını saklar, dosyaları kopyalamaz.
///
/// C#'taki `IDisposable`, uygulayan tipin `Drop`'una karşılık gelir (trait'te ayrı metot yok).
pub trait IShelfService {
    fn items(&self) -> Vec<ShelfItem>;

    /// `ItemsChanged` olayı (`EventHandler` → argümansız `Event<()>`).
    fn items_changed(&self) -> &Event<()>;

    fn add_file(&self, path: &str);
    fn add_text(&self, text: &str);
    fn remove(&self, id: &str);
    fn clear(&self);
}

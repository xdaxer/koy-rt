use serde::{Deserialize, Serialize};

// Ad settings.json'a yazılır: yayımlandıktan sonra adları değiştirilmez (yalnız yeni değer eklenir).

/// Arayüz simge kaynağı. `Builtin`: uygulamayla gelen simgeler. `Folder`: kullanıcının seçtiği klasördeki
/// simgeler; klasörde olmayan anahtar yerleşik simgeye düşer. Dosyanın yüklenmesi Core dışında yapılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconPack {
    Builtin,
    Folder,
}

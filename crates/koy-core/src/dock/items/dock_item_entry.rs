use serde::{Deserialize, Serialize};

/// Ayar dosyasındaki ham dock öğesi. Tür bilerek metin: bilinmeyen bir tür (yeni sürümden ya da elle düzenlenmiş dosya)
/// JSON okumayı bozmasın diye enum'a çevrilmez; `DockItemRules::normalize` bunları atar.
///
/// settings.json'da C# özellik adlarıyla (`Kind`, `Id`, `Command`) PascalCase yazılır.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct DockItemEntry {
    pub kind: String,

    /// App: kısayol dosya adı (.lnk). Folder: klasör kimliği. Custom: düğmenin benzersiz kimliği. Clock, Trash, Separator, Spacer için yok sayılır.
    pub id: String,

    /// Yalnız Custom türünde: çalıştırılacak komut, URL ya da dosya. Core bunu asla çalıştırmaz.
    pub command: String,
}

impl DockItemEntry {
    pub fn new(kind: impl Into<String>, id: impl Into<String>, command: impl Into<String>) -> Self {
        DockItemEntry { kind: kind.into(), id: id.into(), command: command.into() }
    }
}

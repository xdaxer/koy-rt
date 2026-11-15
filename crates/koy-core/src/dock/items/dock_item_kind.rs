use std::fmt;

use serde::{Deserialize, Serialize};

/// Dock öğe türleri. Yeni tür eklenirse `DockItemRules` ayrıştırıcısı ve testleri de güncellenir.
/// Core yalnız türü ve kimliği tutar; öğenin nasıl çizildiği ya da çalıştığı App ve Platform katmanının işidir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DockItemKind {
    #[default]
    App,
    Folder,
    Separator,
    Spacer,
    Clock,
    Trash,
    Custom,
}

impl DockItemKind {
    /// C# `kind.ToString()`: enum adı.
    pub fn as_str(&self) -> &'static str {
        match self {
            DockItemKind::App => "App",
            DockItemKind::Folder => "Folder",
            DockItemKind::Separator => "Separator",
            DockItemKind::Spacer => "Spacer",
            DockItemKind::Clock => "Clock",
            DockItemKind::Trash => "Trash",
            DockItemKind::Custom => "Custom",
        }
    }
}

impl fmt::Display for DockItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

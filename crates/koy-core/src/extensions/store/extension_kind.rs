//! Karşılığı: `Extensions/Store/ExtensionKind.cs`.

use serde::{Deserialize, Serialize};

/// Mağazadaki eklenti türü. Küme kapalıdır ve hepsi veri paketidir: kod çalıştıran tür yoktur (AGENTS #4).
/// JSON'daki adlar küçük harf: theme, iconpack, background, profile.
/// (Kayıt dosyasında enum adı olarak yazılır: `"Theme"`, `"IconPack"`…)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub enum ExtensionKind {
    #[default]
    Theme,
    IconPack,
    Background,
    Profile,
}

impl ExtensionKind {
    /// Enum adı (`kind.ToString()`): kayıt dosyasındaki metin değeri.
    pub fn name(self) -> &'static str {
        match self {
            ExtensionKind::Theme => "Theme",
            ExtensionKind::IconPack => "IconPack",
            ExtensionKind::Background => "Background",
            ExtensionKind::Profile => "Profile",
        }
    }

    /// `ExtensionKinds.ToJsonName`: indeks ve manifestteki küçük harfli ad.
    pub fn to_json_name(self) -> &'static str {
        match self {
            ExtensionKind::Theme => "theme",
            ExtensionKind::IconPack => "iconpack",
            ExtensionKind::Background => "background",
            ExtensionKind::Profile => "profile",
        }
    }
}

pub struct ExtensionKinds;

impl ExtensionKinds {
    pub fn all() -> &'static [ExtensionKind] {
        &[ExtensionKind::Theme, ExtensionKind::IconPack, ExtensionKind::Background, ExtensionKind::Profile]
    }

    /// Birebir eşleşme (büyük/küçük harfe duyarlı): "Theme" ya da "theme " kabul edilmez.
    pub fn try_parse(value: Option<&str>) -> Option<ExtensionKind> {
        match value {
            Some("theme") => Some(ExtensionKind::Theme),
            Some("iconpack") => Some(ExtensionKind::IconPack),
            Some("background") => Some(ExtensionKind::Background),
            Some("profile") => Some(ExtensionKind::Profile),
            _ => None,
        }
    }
}

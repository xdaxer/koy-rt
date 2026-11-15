//! Karşılığı: `Extensions/ExtensionManifest.cs`.

use serde::{Deserialize, Serialize};

/// Eklentinin kendini tanıttığı veri. Kod içermez; yalnız eklentinin ne olduğunu ve hangi yetkilere
/// ihtiyaç duyduğunu anlatır. Bu modelin oluşturulması hiçbir şeyi çalıştırmaz.
/// Dışarıdan tek geçerli yol [`ExtensionManifestValidator::try_create`](super::ExtensionManifestValidator::try_create)'tir:
/// kurucu crate içindedir ve alanlar yalnız okunur (özel), bu yüzden doğrulanmamış bir kimlik ya da kopya oluşamaz.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtensionManifest {
    id: String,
    name: String,
    version: String,
    author: String,
    capabilities: Vec<ExtensionCapability>,
    entry_kind: ExtensionEntryKind,
}

impl ExtensionManifest {
    pub(crate) fn new(
        id: String,
        name: String,
        version: String,
        author: String,
        mut capabilities: Vec<ExtensionCapability>,
        entry_kind: ExtensionEntryKind,
    ) -> Self {
        // Sıralı tutulur: aynı yetki kümesi hangi sırayla yazılırsa yazılsın eşit sayılır.
        // (C#'taki kopya + salt okunur sarmalayıcı karşılığı: alan özel, dışarıya yalnız `&[T]` verilir.)
        capabilities.sort();
        ExtensionManifest { id, name, version, author, capabilities, entry_kind }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn capabilities(&self) -> &[ExtensionCapability] {
        &self.capabilities
    }

    pub fn entry_kind(&self) -> ExtensionEntryKind {
        self.entry_kind
    }
}

/// Eklentinin istediği yetkiler. Küme kapalıdır: listede olmayan bir yetki manifestte reddedilir.
/// Yeni yetki eklemek, kullanıcının kuruluma onay vermesi gereken bir karardır.
/// Sıralama bildirim sırasıdır (C# `Order()` sayısal değere göre sıralar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub enum ExtensionCapability {
    #[default]
    Network,
    Files,
    Clipboard,
}

/// Eklentinin türü. Şimdilik yalnız veri paketi (tema, simge paketi, dil) var. Dış süreçte çalışan
/// eklenti türü proje sahibinin kararını bekler (AGENTS #4), bu yüzden burada yoktur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub enum ExtensionEntryKind {
    #[default]
    Data,
}

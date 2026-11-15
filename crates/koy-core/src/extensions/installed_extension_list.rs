//! Karşılığı: `Extensions/InstalledExtensionList.cs`.

use std::collections::HashSet;

use super::ExtensionManifest;

/// Kurulu bir eklenti ve kullanıcının açma/kapama tercihi. Kapalı eklenti başlatılmaz (AGENTS #5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstalledExtension {
    pub manifest: ExtensionManifest,
    pub enabled: bool,
}

impl InstalledExtension {
    pub fn new(manifest: ExtensionManifest, enabled: bool) -> Self {
        InstalledExtension { manifest, enabled }
    }
}

/// Kurulu eklenti listesi işleminin hata nedeni. Hata yoksa `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InstalledExtensionListError {
    #[default]
    None,
    TooManyExtensions,
    DuplicateId,
    UnknownId,
}

/// Kurulu eklenti listesinin saf kuralları: sıralama, tekil kimlik, üst sınır ve açma/kapama.
/// Dosya okumaz, yazmaz ve hiçbir eklentiyi başlatmaz.
pub struct InstalledExtensionList;

impl InstalledExtensionList {
    /// Üst sınır. Her eklenti ayrı bir değerlendirme ve bellek maliyeti getirir.
    pub const MAX_INSTALLED: usize = 64;

    /// Girdiyi kimliğe göre (ordinal) sıralı bir listeye çevirir. Aynı kimlik iki kez gelirse ya da sınır
    /// aşılırsa reddeder. Sıralama girdinin sırasından bağımsızdır, bu yüzden sonuç her zaman aynıdır.
    /// (C# `out list` hatada boş listeydi; burada `Err`.)
    pub fn try_build(
        entries: impl IntoIterator<Item = InstalledExtension>,
    ) -> Result<Vec<InstalledExtension>, InstalledExtensionListError> {
        // En çok MAX_INSTALLED + 1 öğe okunur: sınır aşılınca numaralandırma durur, fazlası kopyalanmaz.
        let mut items: Vec<InstalledExtension> = Vec::new();
        for entry in entries {
            if items.len() == Self::MAX_INSTALLED {
                return Err(InstalledExtensionListError::TooManyExtensions);
            }

            items.push(entry);
        }

        // Kimlikler doğrulamadan geçtiği için büyük/küçük harf farkı yoktur; ordinal karşılaştırma yeterli.
        let mut seen: HashSet<&str> = HashSet::new();
        for item in &items {
            if !seen.insert(item.manifest.id()) {
                return Err(InstalledExtensionListError::DuplicateId);
            }
        }

        // OrderBy kararlıdır: sort_by (kararlı).
        items.sort_by(|a, b| a.manifest.id().cmp(b.manifest.id()));
        Ok(items)
    }

    /// Yalnız verilen kimliğin açık/kapalı durumunu değiştirir ve yeni bir liste döndürür. Girdi liste
    /// değişmez. Kimlik yoksa `UnknownId` döner (C#'ta `result` girdi listenin kendisiydi; burada `Err`).
    pub fn try_set_enabled(
        list: &[InstalledExtension],
        id: &str,
        enabled: bool,
    ) -> Result<Vec<InstalledExtension>, InstalledExtensionListError> {
        let Some(index) = list.iter().position(|e| e.manifest.id() == id) else {
            return Err(InstalledExtensionListError::UnknownId);
        };

        // Kimlik değişmediği için sıralama korunur.
        let mut copy = list.to_vec();
        copy[index] = InstalledExtension { enabled, ..list[index].clone() };
        Ok(copy)
    }
}

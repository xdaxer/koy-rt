//! Karşılığı: `Extensions/Store/ExtensionPaths.cs`.

use std::path::{Path, PathBuf};

use crate::extensions::ExtensionManifestValidator;
use crate::text;
use crate::win_path;

/// Eklenti dosyalarının yerleri: `%AppData%\Koy\extensions.json` ve `%AppData%\Koy\extensions\<id>\`.
pub struct ExtensionPaths;

impl ExtensionPaths {
    /// `KoyFolder`: `%AppData%\Koy` (ortam değişkeni yoksa C#'taki gibi boş klasör: `Koy`).
    pub fn koy_folder() -> PathBuf {
        let base = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(win_path::combine(&base, "Koy"))
    }

    /// `RegistryPath`: `%AppData%\Koy\extensions.json`.
    pub fn registry_path() -> PathBuf {
        PathBuf::from(win_path::combine(&Self::koy_folder().to_string_lossy(), "extensions.json"))
    }

    /// `ExtensionsFolder`: `%AppData%\Koy\extensions`.
    pub fn extensions_folder() -> PathBuf {
        PathBuf::from(win_path::combine(&Self::koy_folder().to_string_lossy(), "extensions"))
    }

    /// Kimliğin kurulum klasörü. Kimlik doğrulanmış olmalı (küçük harf, rakam, tire); aksi halde `None`.
    /// `extensions_folder`: `None` iken [`ExtensionPaths::extensions_folder`] (C# `null` varsayılanı).
    pub fn folder_for(id: Option<&str>, extensions_folder: Option<&Path>) -> Option<PathBuf> {
        if !ExtensionManifestValidator::is_valid_id(id) {
            return None;
        }

        let root = match extensions_folder {
            Some(folder) => folder.to_string_lossy().into_owned(),
            None => Self::extensions_folder().to_string_lossy().into_owned(),
        };
        Some(PathBuf::from(win_path::combine(&root, id?)))
    }

    /// `path` tam çözüldükten sonra `root`'un kesin altında mı (kökün kendisi sayılmaz).
    /// Silmeden önceki son güvenlik denetimi: ".." ya da simge bağlantısıyla köke çıkan yol yanlış klasörü silmesin.
    pub fn is_strictly_under(root: &Path, path: &Path) -> bool {
        // `Path.GetFullPath` boş ya da NUL içeren yolda fırlatır (ArgumentException): C# `false` döner.
        let (Some(full_root), Some(full_path)) =
            (win_path::get_full_path(&root.to_string_lossy()), win_path::get_full_path(&path.to_string_lossy()))
        else {
            return false;
        };

        let mut full_root = full_root.trim_end_matches(['\\', '/']).to_string();
        full_root.push('\\');
        // C# `Length`: UTF-16 birimi.
        full_path.encode_utf16().count() > full_root.encode_utf16().count() && text::starts_with_ignore_case(&full_path, &full_root)
    }
}

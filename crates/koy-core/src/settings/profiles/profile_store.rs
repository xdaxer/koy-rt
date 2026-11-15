//! Karşılığı: `koy/src/Core/Settings/Profiles/ProfileStore.cs`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{ProfileLibrary, SavedProfile};
use crate::settings::JsonListFile;
use crate::settings::json_list_file::in_settings_folder;

/// Dosyadaki ham kayıt: alanlar eksik/null olabilir (elle bozulmuş dosya); `Load` süzer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct RawProfile {
    id: Option<String>,
    name: Option<String>,
    json: Option<String>,
}

/// Kayıtlı profillerin dosyası (ayar klasöründe `profiles.json`). Okuma hatasında boş liste, yazma hatasında false
/// (bkz. `JsonListFile`); çağıran çökmez. Mantık `ProfileLibrary` içindedir.
pub struct ProfileStore {
    file: JsonListFile<Option<RawProfile>>,
}

impl ProfileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        ProfileStore { file: JsonListFile::new(path, None) }
    }

    /// Hata geri çağrısıyla (varsayılan: crash.log). Testler günlüğe yazmamak için kullanır.
    pub fn with_error_handler(path: impl Into<PathBuf>, on_error: Option<crate::settings::json_list_file::JsonErrorCallback>) -> Self {
        ProfileStore { file: JsonListFile::new(path, on_error) }
    }

    pub fn default_path() -> PathBuf {
        in_settings_folder("profiles.json")
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Boş kimlikli, adsız ya da yinelenen kimlikli kayıtlar atılır (elle bozulmuş dosya).
    pub fn load(&self) -> Vec<SavedProfile> {
        let mut seen: Vec<String> = Vec::new();
        let mut result = Vec::new();
        for raw in self.file.load().into_iter().flatten() {
            let (Some(id), Some(name), Some(json)) = (raw.id, raw.name, raw.json) else { continue };
            if id.is_empty() || seen.contains(&id) {
                continue;
            }
            seen.push(id.clone());
            result.push(SavedProfile { id, name, json });
            if result.len() >= ProfileLibrary::MAX_PROFILES as usize {
                break;
            }
        }
        result
    }

    pub fn save(&self, profiles: &[SavedProfile]) -> bool {
        let raw: Vec<Option<RawProfile>> = profiles
            .iter()
            .map(|p| Some(RawProfile { id: Some(p.id.clone()), name: Some(p.name.clone()), json: Some(p.json.clone()) }))
            .collect();
        self.file.save(&raw)
    }
}

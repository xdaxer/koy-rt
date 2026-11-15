//! Karşılığı: `koy/src/Core/Settings/JsonListFile.cs` (+ `DataJsonContext.cs`: Rust'ta tip bilgisi serde'den gelir).

use std::marker::PhantomData;
use std::sync::Arc;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::SettingsStore;
use super::json_tree;
use crate::diagnostics::{CrashError, CrashLog};

/// C# `Action<string, Exception>`: kaynak adı ve hata.
pub type JsonErrorCallback = Arc<dyn Fn(&str, &dyn std::error::Error) + Send + Sync>;

/// Ayar klasöründeki küçük bir JSON liste dosyası (sabit pano öğeleri, görevler). Okuma hatasında çökmez:
/// boş liste döner ve hata crash.log'a yazılır. Yazma geçici dosya + taşıma ile yapılır (yarım dosya kalmaz).
/// Biçim ayar dosyasıyla aynı (girintili, PascalCase alanlar serde sahibi tipin işidir).
pub struct JsonListFile<T> {
    path: PathBuf,
    on_error: JsonErrorCallback,
    _items: PhantomData<fn() -> T>,
}

impl<T: Serialize + DeserializeOwned> JsonListFile<T> {
    /// `on_error` verilmezse hata `CrashLog.Write` ile günlüğe yazılır.
    pub fn new(path: impl Into<PathBuf>, on_error: Option<JsonErrorCallback>) -> Self {
        let on_error = on_error.unwrap_or_else(|| {
            Arc::new(|source: &str, error: &dyn std::error::Error| {
                CrashLog::write(source, &CrashError::exception("Exception", &error.to_string(), ""));
            })
        });
        JsonListFile { path: path.into(), on_error, _items: PhantomData }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Ayar klasöründe (settings.json'ın yanı) dosya yolu (`JsonListFile.InSettingsFolder`).
    pub fn in_settings_folder(file_name: &str) -> PathBuf {
        in_settings_folder(file_name)
    }

    fn file_name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// Dosya yoksa, bozuksa ya da okunamazsa boş liste.
    pub fn load(&self) -> Vec<T> {
        if !self.path.exists() {
            return Vec::new();
        }

        let result: Result<Vec<T>, Box<dyn std::error::Error>> = (|| {
            let bytes = std::fs::read(&self.path)?;
            let tree = json_tree::parse_bytes(&bytes)?;
            if tree.is_null() {
                return Ok(Vec::new());
            }
            Ok(serde_json::from_value::<Vec<T>>(tree)?)
        })();

        match result {
            Ok(items) => items,
            Err(e) => {
                (self.on_error)(&format!("JsonListFile.Load {}", self.file_name()), e.as_ref());
                Vec::new()
            }
        }
    }

    /// Başarılıysa true. Yazılamazsa (disk dolu, kilit) hata loglanır, çağıran çökmez.
    pub fn save(&self, items: &[T]) -> bool {
        let result: std::io::Result<()> = (|| {
            if let Some(dir) = self.path.parent() {
                if !dir.as_os_str().is_empty() {
                    std::fs::create_dir_all(dir)?;
                }
            }
            let mut temp = self.path.clone().into_os_string();
            temp.push(".tmp");
            let temp = PathBuf::from(temp);
            let value: Value = serde_json::to_value(items).map_err(std::io::Error::other)?;
            std::fs::write(&temp, json_tree::to_pretty_string(&value))?;
            std::fs::rename(&temp, &self.path)
        })();

        match result {
            Ok(()) => true,
            Err(e) => {
                (self.on_error)(&format!("JsonListFile.Save {}", self.file_name()), &e);
                false
            }
        }
    }
}

/// Ayar klasöründe (settings.json'ın yanı) dosya yolu.
pub fn in_settings_folder(file_name: &str) -> PathBuf {
    SettingsStore::default_path().parent().map(Path::to_path_buf).unwrap_or_default().join(file_name)
}

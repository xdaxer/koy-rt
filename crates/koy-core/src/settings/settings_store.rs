//! Karşılığı: `koy/src/Core/Settings/SettingsStore.cs`.
//!
//! JSON biçimi C# `System.Text.Json` çıktısıyla birebir uyumludur (bkz. `json_tree`): PascalCase özellik adları,
//! enum'lar metin, girintili yazım, `Version: 4` ve bölümlere ayrılmış kayıtlar.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde_json::{Map, Value};

use super::json_tree::{self, JsonError, drop_at};
use super::setting_value::SettingValue;
use super::{AppSettings, SettingsMigration};
use crate::text;

/// Tanınmayan bir değer (örn. "Style": "Bogus") yalnız kendi alanını varsayılana döndürür; geri kalan ayarlar korunur.
const MAX_REPAIRS: i32 = 200;

pub struct SettingsStore {
    path: PathBuf,
    backup_path: PathBuf,
}

impl SettingsStore {
    /// `%AppData%\Koy\settings.json`.
    pub fn default_path() -> PathBuf {
        let app_data = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(app_data).join("Koy").join("settings.json")
    }

    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path: PathBuf = path.into();
        // v3 → v4 göçünün ilk çalışmasında eski dosyanın kopyası. v4 dosyasını eski sürüm (v0.2.6) okursa bütün düz alanları
        // eksik görüp varsayılana döner ve ilk kayıtta ayarlar kalıcı gider; geri dönüşte bu dosya geri konur.
        let backup_path = path.parent().map(Path::to_path_buf).unwrap_or_default().join("settings.v3.json");
        SettingsStore { path, backup_path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn backup_path(&self) -> &Path {
        &self.backup_path
    }

    /// Dosya yoksa ya da JSON'u bozuksa varsayılan ayarları döner. Okuma sırasındaki G/Ç hataları (kilitli dosya vb.)
    /// C#'ta yakalanmaz (istisna): burada panik; yakalamak isteyen [`SettingsStore::try_load`] kullanır.
    pub fn load(&self) -> AppSettings {
        self.try_load().unwrap_or_else(|e| panic!("ayar dosyası okunamadı (IOException): {e}"))
    }

    /// [`SettingsStore::load`]'un G/Ç hatasını döndüren hali.
    pub fn try_load(&self) -> io::Result<AppSettings> {
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }

        let bytes = std::fs::read(&self.path)?;
        // JsonException → varsayılan.
        let Ok(Value::Object(mut root)) = json_tree::parse_bytes(&bytes) else {
            return Ok(AppSettings::default());
        };

        // Göç tipli okumadan önce JSON ağacında yapılır: şema ya da değer biçimi değişse de dosya okunabilir kalır.
        let original_version = SettingsMigration::migrate(&mut root);
        if original_version < AppSettings::CURRENT_VERSION {
            self.backup_original_file();
        }

        let mut dropped = 0;
        let mut tree = Value::Object(root);
        Ok(match Self::parse_tolerant(&mut tree, &mut dropped) {
            Ok(settings) => settings.fill_missing_sections(),
            Err(_) => AppSettings::default(),
        })
    }

    /// Toleranslı okuma. `dropped` kaç alanın varsayılana döndüğünü verir; içe aktarma bunu kullanarak geçersiz değer
    /// içeren dosyayı reddeder (dosyadan yalnız bu sayı 0 ise kabul edilir). Hata yolu çözülemezse C#'taki gibi hata döner.
    pub(crate) fn parse_tolerant(root: &mut Value, dropped: &mut i32) -> Result<AppSettings, JsonError> {
        *dropped = 0;
        for _ in 0..MAX_REPAIRS {
            match AppSettings::decode(root) {
                Ok(settings) => return Ok(settings),
                Err(fail) => {
                    if drop_at(root, &fail.path) {
                        *dropped += 1;
                    } else {
                        return Err(JsonError("hata yolu çözülemedi".to_string()));
                    }
                }
            }
        }
        *dropped = MAX_REPAIRS;
        Ok(AppSettings::default())
    }

    // Yedek yazılamazsa (kilit, yetki) açılış durmasın; yedek bir kolaylıktır, ayarların kendisi değil.
    fn backup_original_file(&self) {
        if !self.backup_path.exists() {
            let _ = std::fs::copy(&self.path, &self.backup_path);
        }
    }

    /// Önce geçici dosyaya yazar, sonra yerine taşır; yazma yarıda kesilirse eski dosya bozulmaz.
    /// C# `Save` G/Ç hatasında istisna fırlatır: burada `Err` döner (çağıran isterse `unwrap` eder).
    pub fn save(&self, settings: &AppSettings) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)?;
            }
        }
        let mut temp = self.path.clone().into_os_string();
        temp.push(".tmp");
        let temp = PathBuf::from(temp);
        std::fs::write(&temp, json_tree::to_pretty_string(&settings.encode()))?;
        std::fs::rename(&temp, &self.path)?;
        remember_own_write(&self.path);
        Ok(())
    }

    /// Dosyanın şu anki hali Koy'un bu süreçte yaptığı son kayıt mı (dışarıdan düzenlenmemiş).
    pub fn is_own_write(&self) -> bool {
        if !self.path.exists() {
            return false;
        }
        let Some(key) = own_write_key(&self.path) else { return false };
        let Ok(modified) = std::fs::metadata(&self.path).and_then(|m| m.modified()) else { return false };
        own_writes().lock().ok().and_then(|m| m.get(&key).copied()) == Some(modified)
    }

    /// Katı okuma (canlı yeniden yükleme için): dosya yoksa, JSON bozuksa ya da tek bir alan bile tanınmayıp varsayılana
    /// dönecekse `None`; o durumda son geçerli ayarlar korunur. Göç gerektiren (eski sürüm) dosya da reddedilir: göç ve
    /// yedek açılıştaki [`SettingsStore::load`]'un işidir. Kilitli dosya gibi G/Ç hataları `None` döner (yazma sürerken okunabilir).
    pub fn try_read_strict(&self) -> Option<AppSettings> {
        if !self.path.exists() {
            return None;
        }
        let bytes = std::fs::read(&self.path).ok()?;
        let Ok(mut node @ Value::Object(_)) = json_tree::parse_bytes(&bytes) else { return None };

        let version = node.get("Version").and_then(Value::as_i64).and_then(|v| i32::try_from(v).ok()).unwrap_or(0);
        if version != AppSettings::CURRENT_VERSION {
            return None;
        }

        let mut dropped = 0;
        let parsed = Self::parse_tolerant(&mut node, &mut dropped).ok()?;
        if dropped > 0 {
            return None;
        }

        Some(parsed.fill_missing_sections())
    }

    /// `JsonNode` kökünü (nesne) toleranslı okur; `AdvancedSettings`/profil içe aktarma kullanır.
    pub(crate) fn parse_tolerant_object(root: Map<String, Value>, dropped: &mut i32) -> Result<AppSettings, JsonError> {
        let mut tree = Value::Object(root);
        Self::parse_tolerant(&mut tree, dropped)
    }
}

// Koy'un kendi yazdığı dosyanın son yazma zamanı (yol başına; aynı dosyaya birden çok SettingsStore örneği yazar).
// Canlı yeniden yükleme bununla kendi kaydını dışarıdan düzenlemeden ayırır (issue #69).
fn own_writes() -> &'static Mutex<HashMap<String, SystemTime>> {
    static OWN_WRITES: OnceLock<Mutex<HashMap<String, SystemTime>>> = OnceLock::new();
    OWN_WRITES.get_or_init(|| Mutex::new(HashMap::new()))
}

// Anahtar: tam yol, büyük/küçük harf duyarsız (OrdinalIgnoreCase).
fn own_write_key(path: &Path) -> Option<String> {
    let full = std::path::absolute(path).ok()?;
    Some(text::to_upper_invariant(&full.to_string_lossy()))
}

fn remember_own_write(path: &Path) {
    let Some(key) = own_write_key(path) else { return };
    let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) else { return };
    if let Ok(mut map) = own_writes().lock() {
        map.insert(key, modified);
    }
}

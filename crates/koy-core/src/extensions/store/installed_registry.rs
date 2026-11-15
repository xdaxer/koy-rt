//! Karşılığı: `Extensions/Store/InstalledRegistry.cs`.
//!
//! C# `StoreJsonContext` (kaynak üreticili System.Text.Json: girintili yazım, metin enum'lar, camelCase adlar) burada
//! elle yazılmış okuyucu/yazıcıdır: yazım `settings::json_tree` ile bayt uyumlu (aynı .NET kaçışları ve satır sonu),
//! okuma `JsonSerializer.Deserialize<InstalledRegistryFile>` kurallarını izler (bkz. [`read_registry_file`]).

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::iso_time;
use super::json_document::{self, Json};
use super::store_field_rules::StoreFieldRules;
use super::{ExtensionKind, ExtensionPaths};
use crate::diagnostics::{CrashError, CrashLog};
use crate::extensions::{ExtensionManifest, ExtensionManifestValidator, InstalledExtension, InstalledExtensionList, InstalledExtensionListError};
use crate::settings::json_tree::to_pretty_string;
use crate::time::DateTimeOffset;

/// Eklenti nereden kuruldu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ExtensionSource {
    #[default]
    Store,
    File,
}

impl ExtensionSource {
    /// Enum adı (`source.ToString()`): kayıt dosyasındaki metin değeri.
    pub fn name(self) -> &'static str {
        match self {
            ExtensionSource::Store => "Store",
            ExtensionSource::File => "File",
        }
    }
}

/// Kayıttaki bir kurulu eklenti (`extensions.json`). Kurulum klasörü kimlikten türer
/// ([`ExtensionPaths::folder_for`]), bu yüzden yol saklanmaz ve kayıttan yol okunmaz.
///
/// `payload_file`: uygulamada kullanılan ana dosya, kurulum klasörüne göre: theme.json, profile.json, "icons" ya da
/// background.&lt;uzantı&gt;. Arka planın uzantısı pakete göre değiştiği için kaydedilir (C# varsayılanı `""`).
/// `applied`: kullanıcı "Uygula"ya bastı ve henüz geri alınmadı (profil türünde hep false).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledEntry {
    pub id: String,
    pub version: String,
    pub kind: ExtensionKind,
    pub name: String,
    pub author: String,
    pub license: String,
    pub installed_at: DateTimeOffset,
    pub source: ExtensionSource,
    pub applied: bool,
    pub payload_file: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InstalledRegistryError {
    #[default]
    None,
    InvalidEntry,
    TooManyExtensions,
    DuplicateId,
}

/// Kurulu eklenti kaydının değişmez, saf modeli. Kimlik, ad, sürüm ve yazar doğrulaması ile en çok 64 kayıt ve
/// tekil kimlik kuralı mevcut [`ExtensionManifestValidator`] ve [`InstalledExtensionList`]
/// ile yapılır. Girdi sırası ne olursa olsun kimliğe göre sıralanır.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstalledRegistry {
    entries: Vec<InstalledEntry>,
}

impl InstalledRegistry {
    /// `InstalledRegistry.Empty`.
    pub fn empty() -> InstalledRegistry {
        InstalledRegistry { entries: Vec::new() }
    }

    pub fn entries(&self) -> &[InstalledEntry] {
        &self.entries
    }

    pub fn find(&self, id: Option<&str>) -> Option<&InstalledEntry> {
        self.entries.iter().find(|entry| Some(entry.id.as_str()) == id)
    }

    /// Bu türden "uygulandı" işaretli bir kayıt var mı (tema zinciri: ilk özgün yedeği korumak için).
    pub fn has_applied(&self, kind: ExtensionKind) -> bool {
        self.entries.iter().any(|e| e.kind == kind && e.applied)
    }

    pub fn try_build(entries: impl IntoIterator<Item = InstalledEntry>) -> Result<InstalledRegistry, InstalledRegistryError> {
        let mut wrapped: Vec<InstalledExtension> = Vec::new();
        let mut by_id: HashMap<String, InstalledEntry> = HashMap::new();
        for entry in entries {
            let Some(manifest) = Self::is_valid(&entry) else {
                return Err(InstalledRegistryError::InvalidEntry);
            };

            // Sınır aşıldığında numaralandırma durur: dev dosyadan fazlası kopyalanmaz.
            if wrapped.len() > InstalledExtensionList::MAX_INSTALLED {
                break;
            }

            wrapped.push(InstalledExtension::new(manifest, true));
            by_id.entry(entry.id.clone()).or_insert(entry);
        }

        let list = InstalledExtensionList::try_build(wrapped).map_err(|error| {
            if error == InstalledExtensionListError::DuplicateId {
                InstalledRegistryError::DuplicateId
            } else {
                InstalledRegistryError::TooManyExtensions
            }
        })?;

        // C# `byId[e.Manifest.Id]`: her kimlik tabloda vardır.
        Ok(InstalledRegistry { entries: list.iter().filter_map(|e| by_id.get(e.manifest.id()).cloned()).collect() })
    }

    /// Kayıt ekler ya da aynı kimlikli olanı değiştirir (güncelleme/yeniden kurulum). Yeni kimlik için sınır
    /// (64) dolmuşsa `TooManyExtensions`.
    pub fn try_upsert(&self, entry: InstalledEntry) -> Result<InstalledRegistry, InstalledRegistryError> {
        let mut all: Vec<InstalledEntry> = self.entries.iter().filter(|e| e.id != entry.id).cloned().collect();
        all.push(entry);
        Self::try_build(all)
    }

    pub fn remove(&self, id: &str) -> InstalledRegistry {
        InstalledRegistry { entries: self.entries.iter().filter(|e| e.id != id).cloned().collect() }
    }

    /// Kimliği uygulanmış işaretler; aynı türdeki öbürlerinin işaretini kaldırır (tema, simge paketi ve arka plan
    /// aynı anda tek olabilir). Profil türü "uygulanmış" tutulmaz, kayıt değişmez.
    pub fn with_applied(&self, id: &str) -> InstalledRegistry {
        let Some(target) = self.find(Some(id)) else {
            return self.clone();
        };
        if target.kind == ExtensionKind::Profile {
            return self.clone();
        }

        let kind = target.kind;
        InstalledRegistry {
            entries: self
                .entries
                .iter()
                .map(|e| if e.kind == kind { InstalledEntry { applied: e.id == id, ..e.clone() } } else { e.clone() })
                .collect(),
        }
    }

    pub fn without_applied(&self, id: &str) -> InstalledRegistry {
        InstalledRegistry {
            entries: self.entries.iter().map(|e| if e.id == id { InstalledEntry { applied: false, ..e.clone() } } else { e.clone() }).collect(),
        }
    }

    // `Enum.IsDefined` Rust enum'larında her zaman doğrudur (tanımsız değer üretilemez).
    fn is_valid(entry: &InstalledEntry) -> Option<ExtensionManifest> {
        if !StoreFieldRules::is_valid_license(Some(&entry.license)) {
            return None;
        }

        ExtensionManifestValidator::try_create(
            Some(&entry.id),
            Some(&entry.name),
            Some(&entry.version),
            Some(&entry.author),
            std::iter::empty(),
            Some("data"),
        )
        .ok()
    }
}

/// extensions.json'ın dosya biçimi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRegistryFile {
    pub version: i32,
    pub extensions: Vec<InstalledEntry>,
}

/// `crash.log`'a ve hata geri çağrısına giden hata: .NET istisna tür adı ve ileti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError {
    /// `IOException`, `UnauthorizedAccessException`, `JsonException`…
    pub type_name: &'static str,
    pub message: String,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.type_name, self.message)
    }
}

impl std::error::Error for RegistryError {}

impl RegistryError {
    fn from_io(error: &std::io::Error) -> RegistryError {
        let type_name = if error.kind() == std::io::ErrorKind::PermissionDenied { "UnauthorizedAccessException" } else { "IOException" };
        RegistryError { type_name, message: error.to_string() }
    }
}

/// `Action<string, Exception>`: (kaynak, hata).
pub type RegistryErrorHandler = Box<dyn Fn(&str, &RegistryError) + Send + Sync>;

/// Kayıt dosyasının okuma sonucu: yazmaya izin verip vermediğini belirler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegistryLoadState {
    /// Dosya okundu (geçerli ya da yalnız bozuk kayıtlar elendi).
    Ok,

    /// Dosya yok: boş kayıt, yazmak güvenli.
    Missing,

    /// Bozuk JSON ya da bilinmeyen eski/yanlış biçim: yazmadan önce `.bad` olarak kenara alınır.
    Corrupt,

    /// Okunamadı (G/Ç, yetki): dosya belki sağlam, üzerine yazılmaz.
    Unreadable,

    /// Daha yeni sürümün dosyası: bu sürüm asla ezmez.
    NewerVersion,
}

// Okuma-değiştirme-yazma süreç içinde tek sıra: kurulum ve "Uygula" işaretleri birbirinin kaydını ezmesin.
static GATE: Mutex<()> = Mutex::new(());

/// `%AppData%\Koy\extensions.json` okuma/yazma. Okuma hiç fırlatmaz (hata crash.log'a yazılır); okunamayan
/// dosya [`load`](InstalledRegistryStore::load)'da boş görünür ama [`update`](InstalledRegistryStore::update) onu ezmez:
/// dosya yoksa boş kayıttan yazılır, bozuksa önce `.bad` olarak kenara alınır, okunamıyorsa ya da daha yeni sürümse
/// hiç yazılmaz (false). Geçerli dosyada bozuk tek tek kayıtlar, tekrar eden kimlikler ve 64 sınırını aşanlar atılır.
/// Yazma geçici dosya + taşıma ile yapılır (yarım dosya kalmaz).
pub struct InstalledRegistryStore {
    path: PathBuf,
    on_error: RegistryErrorHandler,
}

impl InstalledRegistryStore {
    pub const CURRENT_VERSION: i32 = 1;

    /// `path`: `None` iken [`ExtensionPaths::registry_path`]. `on_error`: `None` iken `CrashLog::write`.
    pub fn new(path: Option<PathBuf>, on_error: Option<RegistryErrorHandler>) -> InstalledRegistryStore {
        InstalledRegistryStore {
            path: path.unwrap_or_else(ExtensionPaths::registry_path),
            on_error: on_error.unwrap_or_else(|| {
                Box::new(|source: &str, error: &RegistryError| {
                    CrashLog::write(source, &CrashError::exception(error.type_name, &error.message, ""));
                })
            }),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> InstalledRegistry {
        let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
        self.load_core().0
    }

    /// Kaydı olduğu gibi yazar; [`update`](InstalledRegistryStore::update) ile aynı koruma kuralları (okunamayan dosya ezilmez).
    pub fn save(&self, registry: &InstalledRegistry) -> bool {
        self.update(|_| Some(registry.clone()))
    }

    /// Kaydı okur, `change` ile değiştirir ve yazar; hepsi tek kilit altında. Dönüş `None` ise
    /// değişiklik yapılmaz. Yazılırsa true; dosya okunamıyorsa ya da daha yeni sürümse false (dosyaya dokunulmaz).
    pub fn update(&self, change: impl FnOnce(&InstalledRegistry) -> Option<InstalledRegistry>) -> bool {
        let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
        let (current, state) = self.load_core();
        match state {
            RegistryLoadState::Unreadable | RegistryLoadState::NewerVersion => return false,
            RegistryLoadState::Corrupt => {
                // Kullanıcı kurtarabilsin diye bozuk dosya silinmez, kenara alınır; alınamazsa yazılmaz.
                if !self.move_aside() {
                    return false;
                }
            }
            RegistryLoadState::Ok | RegistryLoadState::Missing => {}
        }

        match change(&current) {
            Some(next) => self.save_core(&next),
            None => false,
        }
    }

    fn move_aside(&self) -> bool {
        let mut target = append_to_path(&self.path, ".bad");
        if target.exists() {
            // Önceki yedek ezilmez.
            target = append_to_path(&self.path, &format!(".{}.bad", chrono::Utc::now().format("%Y%m%d%H%M%S")));
        }

        match std::fs::rename(&self.path, &target) {
            Ok(()) => true,
            Err(error) => {
                (self.on_error)("InstalledRegistryStore.MoveAside", &RegistryError::from_io(&error));
                false
            }
        }
    }

    fn load_core(&self) -> (InstalledRegistry, RegistryLoadState) {
        if !self.path.exists() {
            return (InstalledRegistry::empty(), RegistryLoadState::Missing);
        }

        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) => {
                (self.on_error)("InstalledRegistryStore.Load", &RegistryError::from_io(&error));
                return (InstalledRegistry::empty(), RegistryLoadState::Unreadable);
            }
        };

        let file = match read_registry_file(&bytes) {
            Ok(file) => file,
            Err(RegistryReadError::Json(message)) => {
                (self.on_error)("InstalledRegistryStore.Load", &RegistryError { type_name: "JsonException", message });
                return (InstalledRegistry::empty(), RegistryLoadState::Corrupt);
            }
            // `JsonSerializer.Deserialize` null döndü ("null" metni).
            Err(RegistryReadError::Null) => return (InstalledRegistry::empty(), RegistryLoadState::Corrupt),
        };

        if file.version > Self::CURRENT_VERSION {
            return (InstalledRegistry::empty(), RegistryLoadState::NewerVersion);
        }

        let Some(extensions) = file.extensions else {
            return (InstalledRegistry::empty(), RegistryLoadState::Corrupt);
        };
        if file.version != Self::CURRENT_VERSION {
            return (InstalledRegistry::empty(), RegistryLoadState::Corrupt);
        }

        // Tek tek ele: bir kaydın bozukluğu diğerlerinin kaydını silmesin.
        let mut good: Vec<InstalledEntry> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in extensions.into_iter().flatten() {
            if InstalledRegistry::try_build([entry.clone()]).is_err() || !seen.insert(entry.id.clone()) || good.len() == InstalledExtensionList::MAX_INSTALLED
            {
                continue;
            }

            good.push(entry);
        }

        match InstalledRegistry::try_build(good) {
            Ok(registry) => (registry, RegistryLoadState::Ok),
            Err(_) => (InstalledRegistry::empty(), RegistryLoadState::Ok),
        }
    }

    fn save_core(&self, registry: &InstalledRegistry) -> bool {
        let result = (|| -> std::io::Result<()> {
            if let Some(directory) = self.path.parent() {
                std::fs::create_dir_all(directory)?;
            }

            let temp = append_to_path(&self.path, ".tmp");
            let file = InstalledRegistryFile { version: Self::CURRENT_VERSION, extensions: registry.entries.clone() };
            std::fs::write(&temp, write_registry_file(&file))?;
            std::fs::rename(&temp, &self.path)
        })();

        match result {
            Ok(()) => true,
            Err(error) => {
                (self.on_error)("InstalledRegistryStore.Save", &RegistryError::from_io(&error));
                false
            }
        }
    }
}

fn append_to_path(path: &Path, suffix: &str) -> PathBuf {
    let mut text = path.as_os_str().to_os_string();
    text.push(suffix);
    PathBuf::from(text)
}

// ---------------------------------------------------------------- JSON yazımı

/// `JsonSerializer.Serialize(InstalledRegistryFile)`: girintili, camelCase, enum'lar metin (özellik sırası = kurucu sırası).
fn write_registry_file(file: &InstalledRegistryFile) -> String {
    let extensions: Vec<Value> = file
        .extensions
        .iter()
        .map(|e| {
            let mut m = Map::new();
            m.insert("id".into(), Value::String(e.id.clone()));
            m.insert("version".into(), Value::String(e.version.clone()));
            m.insert("kind".into(), Value::String(e.kind.name().to_string()));
            m.insert("name".into(), Value::String(e.name.clone()));
            m.insert("author".into(), Value::String(e.author.clone()));
            m.insert("license".into(), Value::String(e.license.clone()));
            m.insert("installedAt".into(), Value::String(iso_time::format_json_date_time_offset(&e.installed_at)));
            m.insert("source".into(), Value::String(e.source.name().to_string()));
            m.insert("applied".into(), Value::Bool(e.applied));
            m.insert("payloadFile".into(), Value::String(e.payload_file.clone()));
            Value::Object(m)
        })
        .collect();

    let mut root = Map::new();
    root.insert("version".into(), Value::from(file.version));
    root.insert("extensions".into(), Value::Array(extensions));
    to_pretty_string(&Value::Object(root))
}

// ---------------------------------------------------------------- JSON okuması

enum RegistryReadError {
    /// `JsonException`.
    Json(String),
    /// Kök `null`: `Deserialize` null döner.
    Null,
}

/// Okunmuş dosya. `extensions` `None` ise listeydi `null` (alan yok ya da `null`); öğe `None` ise `null` kayıt ya da
/// tanımsız enum sayısı taşıyan kayıttır (C#'ta `Enum.IsDefined` başarısızlığı: ikisi de elenir).
struct RawRegistryFile {
    version: i32,
    extensions: Option<Vec<Option<InstalledEntry>>>,
}

/// `JsonSerializer.Deserialize<InstalledRegistryFile>` kuralları: yorum ve sondaki virgül yok; kök nesne olmalı;
/// özellik adları camelCase ve büyük/küçük harfe duyarlı; bilinmeyen özellik yok sayılır; yinelenen özellikte sonuncusu;
/// tür uyuşmazlığı (sayı yerine metin, null olamayan değerde null…) `JsonException`; eksik özellik varsayılan değerini
/// alır (`PayloadFile` için `""`, enum için ilk üye, tarih için `0001-01-01`); metin alanında `null` boş sayılır
/// (boş metin de doğrulamadan geçemeyeceği için kayıt aynı biçimde elenir).
fn read_registry_file(bytes: &[u8]) -> Result<RawRegistryFile, RegistryReadError> {
    // Akıştan okuma UTF-8 BOM'u atlar.
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let root = json_document::parse(bytes, 64).map_err(|()| RegistryReadError::Json("The JSON is invalid or empty.".to_string()))?;
    if root.is_null() {
        return Err(RegistryReadError::Null);
    }

    if !root.is_object() {
        return Err(type_error("The JSON value could not be converted to InstalledRegistryFile."));
    }

    let version = match root.try_get_property("version") {
        None => 0,
        Some(value) => value.try_get_i32().ok_or_else(|| type_error("$.version could not be converted to System.Int32."))?,
    };

    let extensions = match root.try_get_property("extensions") {
        None | Some(Json::Null) => None,
        Some(Json::Array(items)) => {
            let mut list = Vec::with_capacity(items.len());
            for item in items {
                list.push(read_entry(item)?);
            }
            Some(list)
        }
        Some(_) => return Err(type_error("$.extensions could not be converted to List<InstalledEntry>.")),
    };

    Ok(RawRegistryFile { version, extensions })
}

fn type_error(message: &str) -> RegistryReadError {
    RegistryReadError::Json(message.to_string())
}

fn read_entry(item: &Json) -> Result<Option<InstalledEntry>, RegistryReadError> {
    match item {
        Json::Null => return Ok(None),
        Json::Object(_) => {}
        _ => return Err(type_error("The JSON value could not be converted to InstalledEntry.")),
    }

    let text = |name: &str| -> Result<String, RegistryReadError> {
        match item.try_get_property(name) {
            None | Some(Json::Null) => Ok(String::new()),
            Some(Json::String(s)) => Ok(s.clone()),
            Some(_) => Err(type_error(&format!("The JSON value for {name} could not be converted to System.String."))),
        }
    };

    let kind = read_enum(item.try_get_property("kind"), &[ExtensionKind::Theme, ExtensionKind::IconPack, ExtensionKind::Background, ExtensionKind::Profile], |k| k.name())?;
    let source = read_enum(item.try_get_property("source"), &[ExtensionSource::Store, ExtensionSource::File], |s| s.name())?;
    let applied = match item.try_get_property("applied") {
        None => false,
        Some(Json::True) => true,
        Some(Json::False) => false,
        Some(_) => return Err(type_error("The JSON value for applied could not be converted to System.Boolean.")),
    };
    let installed_at = match item.try_get_property("installedAt") {
        None => NaiveDate::from_ymd_opt(1, 1, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .and_then(|naive| chrono::FixedOffset::east_opt(0).and_then(|o| o.from_local_datetime(&naive).single())),
        Some(Json::String(s)) => iso_time::parse_json_date_time_offset(s),
        Some(_) => None,
    }
    .ok_or_else(|| type_error("The JSON value for installedAt could not be converted to System.DateTimeOffset."))?;

    let (Some(kind), Some(source)) = (kind, source) else {
        return Ok(None);
    };

    Ok(Some(InstalledEntry {
        id: text("id")?,
        version: text("version")?,
        kind,
        name: text("name")?,
        author: text("author")?,
        license: text("license")?,
        installed_at,
        source,
        applied,
        payload_file: text("payloadFile")?,
    }))
}

/// `JsonStringEnumConverter` (tamsayı değerlere de izin verir): metin adı büyük/küçük harf duyarsız ya da sayı
/// (metin olarak yazılmış sayı da). Bilinmeyen ad `JsonException` (`Err`); tanımsız sayı `Ok(None)` (ardından
/// `Enum.IsDefined` kaydı eler). Eksik özellik: ilk üye.
fn read_enum<T: Copy>(value: Option<&Json>, members: &[T], name_of: impl Fn(T) -> &'static str) -> Result<Option<T>, RegistryReadError> {
    let by_number = |text: &str| -> Option<Option<T>> {
        text.parse::<i64>().ok().map(|n| usize::try_from(n).ok().and_then(|i| members.get(i).copied()))
    };

    match value {
        None => Ok(members.first().copied()),
        Some(Json::Number(raw)) => by_number(raw).ok_or_else(|| type_error("The JSON value could not be converted to an enum.")),
        Some(Json::String(text)) => {
            let trimmed = text.trim();
            if let Some(member) = members.iter().copied().find(|m| crate::text::eq_ignore_case(name_of(*m), trimmed)) {
                return Ok(Some(member));
            }

            by_number(trimmed).ok_or_else(|| type_error("The JSON value could not be converted to an enum."))
        }
        Some(_) => Err(type_error("The JSON value could not be converted to an enum.")),
    }
}

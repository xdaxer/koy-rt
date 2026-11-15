use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Duration;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::calendar_event::{CalendarEvent, CalendarEventSource};
use super::calendar_oauth::{CalendarProvider, OAuthClient};
use super::iso_time;
use crate::text;
use crate::time::DateTimeOffset;

/// `null` JSON değerini varsayılan (boş metin) sayar: C#'taki `settings.X ??= ""`.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// System.Text.Json `DateTimeOffset` alanı (`2026-11-15T10:00:00.1234567+00:00`).
mod json_date_time_offset {
    use super::*;

    pub fn serialize<S: Serializer>(value: &DateTimeOffset, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&iso_time::format_date_time_offset(value))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DateTimeOffset, D::Error> {
        let text = String::deserialize(deserializer)?;
        iso_time::parse_iso(&text).map(|p| iso_time::to_date_time_offset(&p)).ok_or_else(|| serde::de::Error::custom("geçersiz DateTimeOffset"))
    }
}

mod json_optional_date_time_offset {
    use super::*;

    pub fn serialize<S: Serializer>(value: &Option<DateTimeOffset>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(v) => serializer.serialize_str(&iso_time::format_date_time_offset(v)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<DateTimeOffset>, D::Error> {
        match Option::<String>::deserialize(deserializer)? {
            None => Ok(None),
            Some(text) => iso_time::parse_iso(&text)
                .map(|p| Some(iso_time::to_date_time_offset(&p)))
                .ok_or_else(|| serde::de::Error::custom("geçersiz DateTimeOffset")),
        }
    }
}

/// Şifreli olarak diskte saklanan OAuth belirteç kümesi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OAuthTokenSet {
    #[serde(default, deserialize_with = "null_as_default")]
    pub access_token: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub refresh_token: String,
    #[serde(with = "json_date_time_offset")]
    pub expires_at_utc: DateTimeOffset,
    #[serde(default, deserialize_with = "null_as_default")]
    pub account_label: String,
}

impl OAuthTokenSet {
    pub fn new(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at_utc: DateTimeOffset,
        account_label: impl Into<String>,
    ) -> Self {
        OAuthTokenSet {
            access_token: access_token.into(),
            refresh_token: refresh_token.into(),
            expires_at_utc,
            account_label: account_label.into(),
        }
    }

    /// Belirtecin süresi dolmuşsa veya 60 sn içinde dolacaksa true döner.
    pub fn is_expired(&self, now_utc: DateTimeOffset) -> bool {
        self.expires_at_utc <= now_utc + Duration::seconds(60)
    }
}

/// Çevrimdışı önbellekte saklanan son bilinen harici etkinlikler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CalendarCacheSnapshot {
    #[serde(default, with = "json_optional_date_time_offset")]
    pub last_sync_utc: Option<DateTimeOffset>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub events: Vec<CalendarEvent>,
}

impl CalendarCacheSnapshot {
    pub fn new(last_sync_utc: Option<DateTimeOffset>, events: Vec<CalendarEvent>) -> Self {
        CalendarCacheSnapshot { last_sync_utc, events }
    }
}

/// Harici takvim (Google / Outlook) OAuth ayarları. Token burada tutulmaz. Kimlik ve sır alanları yalnız kendi
/// istemcisini kullanmak isteyen için; boşsa sürümün yerleşik istemcisi kullanılır.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct CalendarSettings {
    pub google_enabled: bool,
    #[serde(deserialize_with = "null_as_default")]
    pub google_client_id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub google_client_secret: String,
    #[serde(deserialize_with = "null_as_default")]
    pub google_account_label: String,
    pub outlook_enabled: bool,
    #[serde(deserialize_with = "null_as_default")]
    pub outlook_client_id: String,
    #[serde(deserialize_with = "null_as_default")]
    pub outlook_account_label: String,
    pub last_sync_unix: i64,
}

fn app_data_koy_dir() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("Koy")
}

fn temp_path(path: &Path) -> PathBuf {
    let mut temp = path.to_path_buf().into_os_string();
    temp.push(".tmp");
    PathBuf::from(temp)
}

/// Önce geçici dosyaya yazar, sonra yerine taşır (`File.Move(overwrite: true)`).
fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = temp_path(path);
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)
}

/// Harici takvim hesap ayarlarını `%AppData%\Koy\calendar-settings.json` dosyasında saklar.
pub struct CalendarSettingsStore {
    path: PathBuf,
}

impl CalendarSettingsStore {
    pub fn new(path: Option<PathBuf>) -> Self {
        CalendarSettingsStore { path: path.unwrap_or_else(Self::default_path) }
    }

    pub fn default_path() -> PathBuf {
        app_data_koy_dir().join("calendar-settings.json")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> CalendarSettings {
        if !self.path.exists() {
            return CalendarSettings::default();
        }

        std::fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Option<CalendarSettings>>(&bytes).ok())
            .flatten()
            .unwrap_or_default()
    }

    pub fn save(&self, settings: &CalendarSettings) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
        write_atomically(&self.path, json.as_bytes())
    }
}

/// Platform bağımsız veri koruma sözleşmesi (Windows'ta DPAPI CurrentUser).
pub trait ITokenProtector: Send + Sync {
    fn protect(&self, plain_bytes: &[u8]) -> Vec<u8>;
    fn unprotect(&self, protected_bytes: &[u8]) -> Vec<u8>;
}

/// OAuth belirteçlerini `%AppData%\Koy\` altında DPAPI ile şifreli saklar.
pub struct CalendarTokenStore {
    protector: Arc<dyn ITokenProtector>,
    directory: PathBuf,
}

impl CalendarTokenStore {
    pub fn new(protector: Arc<dyn ITokenProtector>, directory: Option<PathBuf>) -> Self {
        CalendarTokenStore { protector, directory: directory.unwrap_or_else(Self::default_directory) }
    }

    pub fn default_directory() -> PathBuf {
        app_data_koy_dir()
    }

    pub fn get_token_path(&self, provider: CalendarProvider) -> PathBuf {
        self.directory.join(format!("calendar-token-{}.dat", provider.lower_name()))
    }

    pub fn exists(&self, provider: CalendarProvider) -> bool {
        self.get_token_path(provider).exists()
    }

    pub fn load(&self, provider: CalendarProvider) -> Option<OAuthTokenSet> {
        let path = self.get_token_path(provider);
        // C#'ta her istisna (dosya yok, çözülemedi, bozuk JSON) null döner.
        if !path.exists() {
            return None;
        }

        let encrypted = std::fs::read(&path).ok()?;
        let plain = self.protector.unprotect(&encrypted);
        serde_json::from_slice::<Option<OAuthTokenSet>>(&plain).ok().flatten()
    }

    pub fn save(&self, provider: CalendarProvider, tokens: &OAuthTokenSet) -> io::Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        let path = self.get_token_path(provider);
        let plain = serde_json::to_vec_pretty(tokens).map_err(io::Error::other)?;
        let encrypted = self.protector.protect(&plain);
        write_atomically(&path, &encrypted)
    }

    pub fn delete(&self, provider: CalendarProvider) {
        let path = self.get_token_path(provider);
        // IOException ve UnauthorizedAccessException yutulur.
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Harici takvim etkinliklerini çevrimdışı kullanım için `%AppData%\Koy\calendar-cache.json` dosyasında tutar.
pub struct CalendarCacheStore {
    path: PathBuf,
}

impl CalendarCacheStore {
    pub fn new(path: Option<PathBuf>) -> Self {
        CalendarCacheStore { path: path.unwrap_or_else(Self::default_path) }
    }

    pub fn default_path() -> PathBuf {
        app_data_koy_dir().join("calendar-cache.json")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> CalendarCacheSnapshot {
        let empty = || CalendarCacheSnapshot::new(None, Vec::new());
        if !self.path.exists() {
            return empty();
        }

        let Some(snap) = std::fs::read(&self.path).ok().and_then(|bytes| serde_json::from_slice::<Option<CalendarCacheSnapshot>>(&bytes).ok()).flatten() else {
            return empty();
        };

        let valid = snap
            .events
            .into_iter()
            .filter(|e| {
                !text::is_null_or_white_space(Some(&e.id)) && !text::is_null_or_white_space(Some(&e.title)) && e.source != CalendarEventSource::Local
            })
            .collect();
        CalendarCacheSnapshot::new(snap.last_sync_utc, valid)
    }

    pub fn save(&self, snapshot: &CalendarCacheSnapshot) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(snapshot).map_err(io::Error::other)?;
        write_atomically(&self.path, json.as_bytes())
    }

    pub fn remove_provider(&self, source: CalendarEventSource) -> io::Result<()> {
        let current = self.load();
        let remaining = current.events.into_iter().filter(|e| e.source != source).collect();
        self.save(&CalendarCacheSnapshot::new(current.last_sync_utc, remaining))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthConnectResult {
    pub success: bool,
    pub account_label: String,
    pub error: Option<String>,
}

impl OAuthConnectResult {
    pub fn new(success: bool, account_label: impl Into<String>, error: Option<String>) -> Self {
        OAuthConnectResult { success, account_label: account_label.into(), error }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarSyncResult {
    pub success: bool,
    pub events: Vec<CalendarEvent>,
    pub from_cache: bool,
    pub error: Option<String>,
}

impl CalendarSyncResult {
    pub fn new(success: bool, events: Vec<CalendarEvent>, from_cache: bool, error: Option<String>) -> Self {
        CalendarSyncResult { success, events, from_cache, error }
    }
}

/// `System.Threading.CancellationToken` için asgari karşılık: kopyalar aynı bayrağı paylaşır.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// `CancellationToken.None` / `default`: hiç iptal edilmez (iptal edilemeyen bir bayrak paylaşmaz).
    pub fn none() -> Self {
        Self::default()
    }

    /// `CancellationTokenSource` karşılığı: iptal edilebilen yeni belirteç.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancellation_requested(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Eşzamansız yöntemlerin dönüş tipi (`Task<T>` karşılığı; yürütücü Platform katmanında seçilir).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Platform katmanının uyguladığı OAuth 2.0 PKCE + salt okunur takvim eşitleme sözleşmesi.
pub trait ICalendarSyncService: Send + Sync {
    fn has_token(&self, provider: CalendarProvider) -> bool;

    fn connect_async<'a>(&'a self, provider: CalendarProvider, client: &'a OAuthClient, ct: CancellationToken) -> BoxFuture<'a, OAuthConnectResult>;

    fn disconnect_async<'a>(&'a self, provider: CalendarProvider, ct: CancellationToken) -> BoxFuture<'a, ()>;

    fn sync_async<'a>(
        &'a self,
        provider: CalendarProvider,
        client: &'a OAuthClient,
        from_utc: DateTimeOffset,
        to_utc: DateTimeOffset,
        ct: CancellationToken,
    ) -> BoxFuture<'a, CalendarSyncResult>;
}

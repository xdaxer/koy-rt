//! `IExtensionStoreClient` uygulaması: indeksi ve paketleri paylaşılan HTTP istemcisiyle alır, boyut sınırlı akışla
//! geçici dosyaya indirir (25 MB bellekte tutulmaz), SHA256'yı doğrular ve açma/kurma işini Core'daki
//! `ExtensionPackageInstaller`'a verir. Hata fırlatmaz, hata kodu döner. Kurulumlar tek sıradadır.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use koy_core::cancel::CancellationToken;
use koy_core::extensions::store::uri::Uri;
use koy_core::extensions::{
    ExtensionInstallError, ExtensionPackageInstaller, ExtensionPaths, ExtensionSource, IExtensionStoreClient, InstallResult,
    InstalledRegistryStore, PackageEntryPolicy, StoreIndexError, StoreIndexParser, StoreIndexResult, StoreItem, StoreUrlPolicy,
};
use sha2::{Digest, Sha256};

use crate::net::{HttpClient, HttpError};

pub struct ExtensionStoreClient {
    http: &'static HttpClient,
    index_url: String,
    installer: ExtensionPackageInstaller,
    install_gate: Mutex<()>,
    registry: Arc<InstalledRegistryStore>,
    extensions_folder: PathBuf,
}

impl ExtensionStoreClient {
    pub const INDEX_URL: &'static str = "https://raw.githubusercontent.com/koyhq/koy/main/store/index.json";

    /// İndeks için süre sınırı (saniye).
    pub const INDEX_TIMEOUT_SECONDS: u64 = 20;

    /// Paket indirme için süre sınırı (saniye), HTTP istemcisinin 10 sn'lik başlık süresinden ayrı.
    pub const PACKAGE_TIMEOUT_SECONDS: u64 = 60;

    pub fn new(
        registry: Option<Arc<InstalledRegistryStore>>,
        extensions_folder: Option<PathBuf>,
        http: Option<&'static HttpClient>,
        index_url: Option<String>,
    ) -> Self {
        let registry = registry.unwrap_or_else(|| Arc::new(InstalledRegistryStore::new(None, None)));
        let extensions_folder = extensions_folder.unwrap_or_else(ExtensionPaths::extensions_folder);
        ExtensionStoreClient {
            http: http.unwrap_or_else(HttpClient::shared),
            index_url: index_url.unwrap_or_else(|| Self::INDEX_URL.to_string()),
            installer: ExtensionPackageInstaller::new(extensions_folder.clone(), Arc::clone(&registry), None),
            install_gate: Mutex::new(()),
            registry,
            extensions_folder,
        }
    }

    /// Kurulu eklenti kaydı; App "Uygula" işaretini de buradan `update` ile yazar.
    pub fn registry(&self) -> &Arc<InstalledRegistryStore> {
        &self.registry
    }

    pub fn extensions_folder(&self) -> &Path {
        &self.extensions_folder
    }

    fn gate(&self) -> std::sync::MutexGuard<'_, ()> {
        self.install_gate.lock().unwrap_or_else(|e| e.into_inner())
    }

    // İndirme: Content-Length ve okunan bayt sayısı iki ayrı sınır; SHA256 akarken hesaplanır.
    fn download(
        &self,
        item: &StoreItem,
        destination: &Path,
        progress: Option<&(dyn Fn(f64) + Send + Sync)>,
        deadline: Instant,
        ct: &CancellationToken,
    ) -> Result<(ExtensionInstallError, i64), DownloadFailure> {
        // Bildirilen boyut hem 25 MB'ı hem indeks öğesini aşamaz (indeks ayrıştırıcısı da sınırlar).
        let limit = PackageEntryPolicy::MAX_DOWNLOAD_BYTES.min(item.package.size);

        let mut response = self.http.get(&item.package.url, ct)?;
        // HTTP istemcisi yönlendirmeleri kendiliğinden izler: son adres güvenilir küme dışına çıktıysa içerik kabul
        // edilmez.
        let final_uri = response.final_url().and_then(|u| Uri::new(&u).ok());
        if !response.is_success() || !StoreUrlPolicy::is_trusted_asset_uri(final_uri.as_ref()) {
            return Ok((ExtensionInstallError::Network, 0));
        }

        let declared = response.content_length().map(|l| l as i64);
        if declared.is_some_and(|l| l > limit) {
            return Ok((ExtensionInstallError::DownloadTooLarge, 0));
        }

        let mut hash = Sha256::new();
        let mut total: i64 = 0;
        let mut output = File::create_new(destination).map_err(|_| DownloadFailure::Io)?;
        let mut buffer = vec![0u8; 81920];
        loop {
            if Instant::now() >= deadline {
                return Err(DownloadFailure::Http(HttpError::Timeout));
            }
            let read = std::io::Read::read(&mut response, &mut buffer).map_err(|e| {
                match e.get_ref().and_then(|inner| inner.downcast_ref::<HttpError>()) {
                    Some(h) => DownloadFailure::Http(h.clone()),
                    None => DownloadFailure::Io,
                }
            })?;
            if read == 0 {
                break;
            }
            total += read as i64;
            if total > limit {
                return Ok((ExtensionInstallError::DownloadTooLarge, 0));
            }
            hash.update(&buffer[..read]);
            output.write_all(&buffer[..read]).map_err(|_| DownloadFailure::Io)?;
            if let Some(p) = progress {
                p((total as f64 / declared.unwrap_or(item.package.size) as f64).min(1.0));
            }
        }

        let actual: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        Ok(if koy_core::text::eq_ignore_case(&actual, &item.package.sha256) {
            (ExtensionInstallError::None, total)
        } else {
            (ExtensionInstallError::HashMismatch, 0)
        })
    }
}

/// İndirme sırasında yakalanan hatalar (C#'ta istisnalar).
enum DownloadFailure {
    Http(HttpError),
    Io,
}

impl From<HttpError> for DownloadFailure {
    fn from(e: HttpError) -> Self {
        DownloadFailure::Http(e)
    }
}

fn try_delete(path: Option<&Path>) {
    // En iyi çaba: yetim .part dosyasını sonraki kurulum `cleanup_stale` ile siler.
    if let Some(p) = path {
        let _ = std::fs::remove_file(p);
    }
}

fn new_guid_n() -> String {
    koy_core::random::get_bytes(16).iter().map(|b| format!("{b:02x}")).collect()
}

impl IExtensionStoreClient for ExtensionStoreClient {
    fn fetch_index(&self, ct: &CancellationToken) -> StoreIndexResult {
        let deadline = Instant::now() + Duration::from_secs(Self::INDEX_TIMEOUT_SECONDS);
        let fail = |e: HttpError| {
            StoreIndexResult::new(if e == HttpError::Canceled { StoreIndexError::Cancelled } else { StoreIndexError::Network }, None, 0)
        };
        let mut response = match self.http.get(&self.index_url, ct) {
            Ok(r) => r,
            Err(e) => return fail(e),
        };
        if !response.is_success() {
            return StoreIndexResult::new(StoreIndexError::Network, None, 0);
        }
        if response.content_length().is_some_and(|l| l > StoreIndexParser::MAX_BYTES as u64) {
            return StoreIndexResult::new(StoreIndexError::TooLarge, None, 0);
        }
        match response.read_to_end_limited(Some(StoreIndexParser::MAX_BYTES), Some(deadline), None) {
            Ok(bytes) => StoreIndexParser::parse_bytes(&bytes),
            Err(HttpError::TooLarge) => StoreIndexResult::new(StoreIndexError::TooLarge, None, 0),
            Err(e) => fail(e),
        }
    }

    fn install_from_store(&self, item: &StoreItem, progress: Option<&(dyn Fn(f64) + Send + Sync)>, ct: &CancellationToken) -> InstallResult {
        let _gate = self.gate();
        let mut part: Option<PathBuf> = None;
        let result = (|| {
            std::fs::create_dir_all(&self.extensions_folder).map_err(|_| DownloadFailure::Io)?;
            let path = self.extensions_folder.join(format!("{}{}.part", ExtensionPackageInstaller::TEMP_PREFIX, new_guid_n()));
            part = Some(path.clone());

            // Süre sınırı bütün indirmeyi kapsar (HTTP zaman aşımı yalnız başlıklara kadar).
            let deadline = Instant::now() + Duration::from_secs(Self::PACKAGE_TIMEOUT_SECONDS);
            let (error, size) = self.download(item, &path, progress, deadline, ct)?;
            if error != ExtensionInstallError::None {
                return Ok(InstallResult::fail(error));
            }
            let stream = File::open(&path).map_err(|_| DownloadFailure::Io)?;
            Ok(self.installer.install(stream, size, Some(item), ExtensionSource::Store))
        })();
        try_delete(part.as_deref());

        match result {
            Ok(r) => r,
            // Kullanıcı iptali ile 60 sn dolması ayrılır.
            Err(DownloadFailure::Http(HttpError::Canceled)) => InstallResult::fail(ExtensionInstallError::Cancelled),
            Err(DownloadFailure::Http(HttpError::Timeout)) => {
                InstallResult::fail(if ct.is_cancellation_requested() { ExtensionInstallError::Cancelled } else { ExtensionInstallError::Timeout })
            }
            Err(DownloadFailure::Http(_)) => InstallResult::fail(ExtensionInstallError::Network),
            Err(DownloadFailure::Io) => InstallResult::fail(ExtensionInstallError::IoError),
        }
    }

    fn install_from_file(&self, path: &Path, ct: &CancellationToken) -> InstallResult {
        let Ok(meta) = std::fs::metadata(path) else { return InstallResult::fail(ExtensionInstallError::FileNotFound) };
        // FileInfo.Exists: klasör "yok" sayılır.
        if !meta.is_file() {
            return InstallResult::fail(ExtensionInstallError::FileNotFound);
        }
        let size = meta.len() as i64;
        if size > PackageEntryPolicy::MAX_DOWNLOAD_BYTES {
            return InstallResult::fail(ExtensionInstallError::DownloadTooLarge);
        }
        if ct.is_cancellation_requested() {
            return InstallResult::fail(ExtensionInstallError::Cancelled);
        }
        let _gate = self.gate();
        match File::open(path) {
            Ok(stream) => self.installer.install(stream, size, None, ExtensionSource::File),
            Err(_) => InstallResult::fail(ExtensionInstallError::IoError),
        }
    }

    fn uninstall(&self, id: &str) -> bool {
        let _gate = self.gate();
        self.installer.uninstall(id)
    }
}

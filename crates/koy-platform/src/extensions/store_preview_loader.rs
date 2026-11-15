//! Mağaza kartı önizlemesini indirir: https, en çok 1 MB, SHA256 indeksteki ile eşleşmezse atılır. Doğrulanmış bayt
//! dizisi `%LocalAppData%\Koy\store-cache\<sha256>.img` olarak saklanır; dosya adı özet olduğu için içerik değişirse
//! başka ada düşer. Önbellekteki dosya da her okunuşta yeniden doğrulanır (elle bozulmuş dosya kullanılmaz).
//! Çağıran yalnız kart görünürken ister (AGENTS #5). Asla fırlatmaz: olmadı ise `None`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use koy_core::cancel::CancellationToken;
use koy_core::extensions::store::uri::Uri;
use koy_core::extensions::{StorePreview, StoreUrlPolicy};
use sha2::{Digest, Sha256};

use crate::net::HttpClient;

pub struct StorePreviewLoader {
    http: &'static HttpClient,
    cache_folder: PathBuf,
}

impl Default for StorePreviewLoader {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl StorePreviewLoader {
    /// İndirilebilecek en büyük önizleme (bayt).
    pub const MAX_BYTES: u64 = 1024 * 1024;

    /// Önbellekte tutulan en çok dosya; aşılınca en eskiler silinir.
    pub const MAX_CACHED_FILES: usize = 200;

    pub const TIMEOUT_SECONDS: u64 = 15;

    pub fn new(http: Option<&'static HttpClient>, cache_folder: Option<PathBuf>) -> Self {
        StorePreviewLoader {
            http: http.unwrap_or_else(HttpClient::shared),
            cache_folder: cache_folder.unwrap_or_else(Self::default_cache_folder),
        }
    }

    pub fn default_cache_folder() -> PathBuf {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("Koy").join("store-cache")
    }

    pub fn cache_folder(&self) -> &Path {
        &self.cache_folder
    }

    pub fn load(&self, preview: &StorePreview, ct: &CancellationToken) -> Option<Vec<u8>> {
        let path = self.cache_path_of(&preview.sha256)?;
        if let Some(cached) = read_cached(&path, &preview.sha256) {
            return Some(cached);
        }
        let bytes = self.download(preview, ct)?;
        self.write_cache(&path, &bytes);
        Some(bytes)
    }

    // Özet yalnız 64 onaltılık karakter olabilir: yol ayırıcı sızdıramaz.
    fn cache_path_of(&self, sha256: &str) -> Option<PathBuf> {
        (sha256.len() == 64 && sha256.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| self.cache_folder.join(format!("{}.img", sha256.to_ascii_lowercase())))
    }

    fn download(&self, preview: &StorePreview, ct: &CancellationToken) -> Option<Vec<u8>> {
        let deadline = Instant::now() + Duration::from_secs(Self::TIMEOUT_SECONDS);
        let mut response = self.http.get(&preview.url, ct).ok()?;
        let final_uri = response.final_url().and_then(|u| Uri::new(&u).ok());
        if !response.is_success()
            || !StoreUrlPolicy::is_trusted_asset_uri(final_uri.as_ref())
            || response.content_length().is_some_and(|l| l > Self::MAX_BYTES)
        {
            return None;
        }
        let bytes = response.read_to_end_limited(Some(Self::MAX_BYTES as usize), Some(deadline), None).ok()?;
        matches(&bytes, &preview.sha256).then_some(bytes)
    }

    fn write_cache(&self, path: &Path, bytes: &[u8]) {
        // Önbellek en iyi çaba: yazılamazsa görsel yine gösterilir, yalnız bir sonraki seferde yeniden iner.
        if std::fs::create_dir_all(&self.cache_folder).is_err() {
            return;
        }
        let suffix: String = koy_core::random::get_bytes(16).iter().map(|b| format!("{b:02x}")).collect();
        let temp = PathBuf::from(format!("{}.{suffix}.tmp", path.display()));
        if std::fs::write(&temp, bytes).is_err() || std::fs::rename(&temp, path).is_err() {
            let _ = std::fs::remove_file(&temp);
            return;
        }
        self.prune();
    }

    fn prune(&self) {
        // Temizlik de en iyi çaba.
        let Ok(entries) = std::fs::read_dir(&self.cache_folder) else { return };
        let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("img")))
            .filter_map(|p| std::fs::metadata(&p).and_then(|m| m.modified()).ok().map(|t| (t, p)))
            .collect();
        if files.len() <= Self::MAX_CACHED_FILES {
            return;
        }
        files.sort_by_key(|f| f.0);
        let excess = files.len() - Self::MAX_CACHED_FILES;
        for (_, p) in files.into_iter().take(excess) {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn read_cached(path: &Path, sha256: &str) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    if meta.len() > StorePreviewLoader::MAX_BYTES {
        let _ = std::fs::remove_file(path);
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if matches(&bytes, sha256) {
        return Some(bytes);
    }
    let _ = std::fs::remove_file(path);
    None
}

fn matches(bytes: &[u8], sha256: &str) -> bool {
    let actual: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    koy_core::text::eq_ignore_case(&actual, sha256)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_accepts_only_hex_digests() {
        let loader = StorePreviewLoader::new(None, Some(PathBuf::from("C:\\cache")));
        assert!(loader.cache_path_of("..\\..\\evil").is_none());
        let digest = "A".repeat(64);
        assert_eq!(loader.cache_path_of(&digest), Some(PathBuf::from(format!("C:\\cache\\{}.img", "a".repeat(64)))));
    }

    #[test]
    fn cached_file_is_revalidated() {
        let dir = std::env::temp_dir().join(format!("koy-preview-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let data = b"png-bytes";
        let sha: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
        let path = dir.join(format!("{sha}.img"));
        std::fs::write(&path, data).unwrap();
        assert_eq!(read_cached(&path, &sha).as_deref(), Some(&data[..]));
        std::fs::write(&path, b"tampered").unwrap();
        assert!(read_cached(&path, &sha).is_none());
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}

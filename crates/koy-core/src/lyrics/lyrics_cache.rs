//! Karşılığı: `Lyrics/LyricsCache.cs`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use chrono::{DateTime, FixedOffset, Utc};
use sha2::{Digest, Sha256};

use super::file_text::read_all_text;
use super::lyrics_query::LyricsQuery;
use crate::time::{SystemTimeProvider, TimeProvider, TimeSpan, span_between};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LyricsCacheStatus {
    Miss,
    Hit,

    /// Yakın zamanda arandı, söz yoktu: hizmete boşuna tekrar gidilmez.
    KnownMissing,
}

/// Çevrim içi bulunan sözlerin disk önbelleği. Parça başına bir dosya; boş dosya "söz yok" kaydıdır ve süresi dolar.
/// Dosya sayısı sınırlıdır, en eskiler silinir. Önbellek hatası özelliği durdurmaz: en kötü ihtimalle yeniden indirilir.
pub struct LyricsCache {
    directory: PathBuf,
    time: Arc<dyn TimeProvider>,
    max_files: i32,
}

impl LyricsCache {
    /// "Söz yok" kaydının ömrü: veritabanına sonradan söz eklenebilir.
    pub const MISSING_LIFETIME: TimeSpan = TimeSpan::from_days(7);

    /// `new LyricsCache(directory)`: gerçek saat, en çok 1000 dosya.
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self::with_options(directory, None, 1000)
    }

    /// `new LyricsCache(directory, timeProvider, maxFiles)`.
    pub fn with_options(directory: impl Into<PathBuf>, time_provider: Option<Arc<dyn TimeProvider>>, max_files: i32) -> Self {
        LyricsCache {
            directory: directory.into(),
            time: time_provider.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
            max_files,
        }
    }

    /// `DefaultDirectory`: `%LOCALAPPDATA%\Koy\lyrics`.
    pub fn default_directory() -> PathBuf {
        let base = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
        base.join("Koy").join("lyrics")
    }

    /// Okunan durum ve (isabette) LRC metni. G/Ç hatası: `Miss`.
    pub fn read(&self, query: &LyricsQuery) -> (LyricsCacheStatus, Option<String>) {
        self.try_read(query).unwrap_or((LyricsCacheStatus::Miss, None))
    }

    fn try_read(&self, query: &LyricsQuery) -> io::Result<(LyricsCacheStatus, Option<String>)> {
        let path = self.path_for(query);
        let meta = match fs::metadata(&path) {
            Ok(m) if m.is_file() => m,
            // FileInfo.Exists: dosya yok (ya da erişilemiyor) → Miss.
            _ => return Ok((LyricsCacheStatus::Miss, None)),
        };

        if meta.len() > 0 {
            return Ok((LyricsCacheStatus::Hit, Some(read_all_text(&path)?)));
        }

        let modified: DateTime<Utc> = meta.modified()?.into();
        let written: DateTime<FixedOffset> = modified.fixed_offset();
        let age = span_between(self.time.utc_now(), written);
        Ok(if age < Self::MISSING_LIFETIME { (LyricsCacheStatus::KnownMissing, None) } else { (LyricsCacheStatus::Miss, None) })
    }

    pub fn write(&self, query: &LyricsQuery, lrc: &str) {
        self.store(query, lrc);
    }

    pub fn write_missing(&self, query: &LyricsQuery) {
        self.store(query, "");
    }

    fn store(&self, query: &LyricsQuery, content: &str) {
        // Önbellek isteğe bağlı: yazılamazsa bir sonraki açılışta yeniden indirilir.
        let _ = self.try_store(query, content);
    }

    fn try_store(&self, query: &LyricsQuery, content: &str) -> io::Result<()> {
        fs::create_dir_all(&self.directory)?;
        let path = self.path_for(query);
        let mut temp = path.clone().into_os_string();
        temp.push(".tmp");
        let temp = PathBuf::from(temp);
        fs::write(&temp, content)?;
        fs::rename(&temp, &path)?;
        self.prune()
    }

    fn prune(&self) -> io::Result<()> {
        let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !crate::text::ends_with_ignore_case(&name, ".lrc") || !entry.file_type()?.is_file() {
                continue;
            }
            files.push((entry.path(), entry.metadata()?.modified()?));
        }
        // GetFiles sırası: ad (NTFS dizin sırası); OrderBy kararlı.
        files.sort_by_key(|(path, _)| path.file_name().map(|n| crate::text::to_upper_invariant(&n.to_string_lossy())));

        if files.len() as i32 <= self.max_files {
            return Ok(());
        }

        // Her yazmada tek tek silmemek için sınırın altına belirgin bir pay bırakılır.
        files.sort_by_key(|(_, modified)| *modified);
        let take = files.len() as i32 - self.max_files * 4 / 5;
        for (path, _) in files.iter().take(take.max(0) as usize) {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    // Parça adları dosya adına uygun olmayabilir ve diskte okunur halde durmasın.
    fn path_for(&self, query: &LyricsQuery) -> PathBuf {
        let hash = Sha256::digest(query.key().as_bytes());
        // Convert.ToHexStringLower(hash)[..32]: ilk 16 bayt.
        let hex: String = hash.iter().take(16).map(|b| format!("{b:02x}")).collect();
        Path::new(&self.directory).join(format!("{hex}.lrc"))
    }
}

//! Karşılığı: `Lyrics/LocalLyricsSource.cs`.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use super::cancellation::{CancellationToken, LyricsError};
use super::file_text::read_all_text;
use super::i_lyrics_source::{ILyricsSource, LyricsLookup};
use super::lyrics_query::LyricsQuery;
use crate::text;

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_SCANNED_FILES: usize = 5000;

// EnumerationOptions: RecurseSubdirectories, MaxRecursionDepth = 5, IgnoreInaccessible, MatchCasing = CaseInsensitive,
// AttributesToSkip = Hidden | System | ReparsePoint.
const MAX_RECURSION_DEPTH: usize = 5;

/// Kullanıcının klasöründeki `.lrc` dosyaları. Önce "Sanatçı - Başlık.lrc" adı denenir, bulunamazsa alt klasörler
/// taranır. Her zaman arka plan iş parçacığında çağrılır (`LyricsSession`).
pub struct LocalLyricsSource {
    directory: PathBuf,
}

impl LocalLyricsSource {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        LocalLyricsSource { directory: directory.into() }
    }

    /// `DefaultDirectory`: Müzik klasörü altında `Lyrics` (`%USERPROFILE%\Music\Lyrics`).
    pub fn default_directory() -> PathBuf {
        let music = std::env::var("USERPROFILE").map(|p| PathBuf::from(p).join("Music")).unwrap_or_default();
        music.join("Lyrics")
    }

    fn find_file(&self, query: &LyricsQuery) -> Option<PathBuf> {
        for name in candidate_names(query) {
            let path = self.directory.join(name);
            if path.is_file() {
                return Some(path);
            }
        }

        // Sanatçısız gevşek arama alakasız dosya getirir ("Love" adı onlarca dosyada geçer).
        if query.artist.is_empty() {
            return None;
        }

        let primary = query.primary_artist();
        enumerate_lrc_files(&self.directory).take(MAX_SCANNED_FILES).find(|path| {
            let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            // Path.GetFileNameWithoutExtension: "*.lrc" eşleşmesi olduğu için son 4 karakter ".lrc".
            let name = &file_name[..file_name.len() - 4];
            text::contains_ignore_case(name, &query.title) && text::contains_ignore_case(name, &primary)
        })
    }
}

impl ILyricsSource for LocalLyricsSource {
    fn find(&self, query: &LyricsQuery, _cancellation_token: &CancellationToken) -> Result<LyricsLookup, LyricsError> {
        // IOException / UnauthorizedAccessException → NotFound.
        if !self.directory.is_dir() {
            return Ok(LyricsLookup::not_found());
        }
        let Some(path) = self.find_file(query) else { return Ok(LyricsLookup::not_found()) };
        match fs::metadata(&path) {
            Ok(meta) if meta.len() <= MAX_FILE_BYTES => {}
            _ => return Ok(LyricsLookup::not_found()),
        }

        match read_all_text(&path) {
            Ok(text) => Ok(LyricsLookup::found(text)),
            Err(_) => Ok(LyricsLookup::not_found()),
        }
    }
}

fn candidate_names(query: &LyricsQuery) -> Vec<String> {
    if query.artist.is_empty() {
        return vec![format!("{}.lrc", sanitize(&query.title))];
    }

    let mut names = vec![format!("{} - {}.lrc", sanitize(&query.artist), sanitize(&query.title))];
    let primary = query.primary_artist();
    if primary != query.artist {
        names.push(format!("{} - {}.lrc", sanitize(&primary), sanitize(&query.title)));
    }
    names
}

/// `Path.GetInvalidFileNameChars()` (Windows): `" < > |`, NUL ve 1-31 denetim karakterleri, `: * ? \ /`.
fn is_invalid_file_name_char(c: char) -> bool {
    matches!(c, '"' | '<' | '>' | '|' | ':' | '*' | '?' | '\\' | '/') || (c as u32) < 32
}

fn sanitize(name: &str) -> String {
    name.chars().map(|c| if is_invalid_file_name_char(c) { '_' } else { c }).collect()
}

/// `Directory.EnumerateFiles(directory, "*.lrc", ScanOptions)`: önce klasörün kendi dosyaları, sonra alt klasörler
/// (.NET özyinelemeli sıralayıcısı alt klasörleri kuyruğa alır). Girdiler ad sırasında okunur (NTFS dizin sırası).
/// Gizli / sistem / yeniden ayrıştırma noktası (bağlantı) öznitelikli dosya ve klasörler atlanır; erişilemeyenler yok sayılır.
fn enumerate_lrc_files(root: &Path) -> impl Iterator<Item = PathBuf> {
    let mut pending: VecDeque<(PathBuf, usize)> = VecDeque::new();
    pending.push_back((root.to_path_buf(), 0));
    let mut ready: VecDeque<PathBuf> = VecDeque::new();

    std::iter::from_fn(move || {
        loop {
            if let Some(file) = ready.pop_front() {
                return Some(file);
            }
            let (dir, depth) = pending.pop_front()?;
            let Ok(read) = fs::read_dir(&dir) else { continue };
            let mut entries: Vec<(String, PathBuf, bool)> = Vec::new();
            for entry in read.flatten() {
                let Ok(meta) = fs::symlink_metadata(entry.path()) else { continue };
                if is_skipped(&meta) {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                entries.push((name, entry.path(), meta.is_dir()));
            }
            entries.sort_by_key(|(name, _, _)| text::to_upper_invariant(name));

            for (name, path, is_dir) in entries {
                if is_dir {
                    if depth < MAX_RECURSION_DEPTH {
                        pending.push_back((path, depth + 1));
                    }
                } else if text::ends_with_ignore_case(&name, ".lrc") {
                    ready.push_back(path);
                }
            }
        }
    })
}

#[cfg(windows)]
fn is_skipped(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM | FILE_ATTRIBUTE_REPARSE_POINT) != 0
}

#[cfg(not(windows))]
fn is_skipped(meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

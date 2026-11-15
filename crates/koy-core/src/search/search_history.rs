use super::search_util::{split_lines, utf16_len};
use super::{SearchCommandSafety, SearchText};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Arama geçmişi kipi. Adlar settings.json'a metin olarak yazılır; değiştirilmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum SearchHistoryMode {
    /// Geçmiş tutulmaz.
    #[default]
    Disabled,

    /// Yazarken geçmişten gri öneri görünür, sağ ok ya da Tab ile alınır.
    Suggestion,

    /// Geçmişten gelen öneri seçili metin olarak yazıya eklenir.
    ImmediateCompletion,
}

/// Çalıştırılan aramalar; dosyada satır başına bir kayıt, en yenisi başta.
pub struct SearchHistory {
    file_path: String,
    /// Tembel yüklenir (`_entries is null` iken henüz okunmadı).
    entries: Mutex<Option<Vec<String>>>,
}

impl SearchHistory {
    pub const MAX_ENTRIES: i32 = 50;

    /// Listede gösterilen kayıt; fazlası yalnız öneride kullanılır.
    pub const VISIBLE_ENTRIES: i32 = 20;

    /// Bundan uzun arama geçmişe yazılmaz (yapıştırılmış metin olabilir).
    pub const MAX_ENTRY_LENGTH: i32 = 256;

    pub fn new(file_path: impl Into<String>) -> Self {
        SearchHistory { file_path: file_path.into(), entries: Mutex::new(None) }
    }

    /// `Entries` (en yenisi başta).
    pub fn entries(&self) -> Vec<String> {
        let mut guard = self.entries.lock().unwrap();
        self.load(&mut guard).clone()
    }

    pub fn add(&self, term: &str) {
        // Satır sonu dosyayı bozar (satır başına bir kayıt); çok satırlı metin hiç yazılmaz.
        let len = utf16_len(term);
        if len == 0 || len > Self::MAX_ENTRY_LENGTH as usize || SearchCommandSafety::has_control_characters(term) {
            return;
        }

        let mut guard = self.entries.lock().unwrap();
        let entries = self.load(&mut guard);
        if !entries.is_empty() && entries[0] == term {
            return;
        }

        if let Some(i) = entries.iter().position(|e| e == term) {
            entries.remove(i);
        }
        entries.insert(0, term.to_string());
        if entries.len() > Self::MAX_ENTRIES as usize {
            entries.truncate(Self::MAX_ENTRIES as usize);
        }
        self.save(entries);
    }

    pub fn remove_at(&self, index: i32) {
        let mut guard = self.entries.lock().unwrap();
        let entries = self.load(&mut guard);
        if index < 0 || index as usize >= entries.len() {
            return;
        }

        entries.remove(index as usize);
        self.save(entries);
    }

    pub fn clear(&self) {
        let mut guard = self.entries.lock().unwrap();
        let entries = self.load(&mut guard);
        entries.clear();
        self.save(entries);
    }

    /// Yazılanla başlayan en yeni kayıt; yoksa `None`.
    pub fn suggest(&self, typed: &str) -> Option<String> {
        if typed.is_empty() {
            return None;
        }

        let folded = SearchText::fold_str(typed);
        let typed_len = utf16_len(typed);
        let mut guard = self.entries.lock().unwrap();
        self.load(&mut guard)
            .iter()
            .find(|e| utf16_len(e) >= typed_len && SearchText::fold_str(e).starts_with(&folded))
            .cloned()
    }

    fn load<'a>(&self, slot: &'a mut Option<Vec<String>>) -> &'a mut Vec<String> {
        if slot.is_none() {
            let loaded = match std::fs::read(&self.file_path) {
                Ok(bytes) => split_lines(&String::from_utf8_lossy(&bytes))
                    .into_iter()
                    .filter(|l| {
                        let n = utf16_len(l);
                        n > 0 && n <= Self::MAX_ENTRY_LENGTH as usize && !SearchCommandSafety::has_control_characters(l)
                    })
                    .take(Self::MAX_ENTRIES as usize)
                    .collect(),
                // Dosya yok ya da okunamadı (IOException / UnauthorizedAccessException): boş liste.
                Err(_) => Vec::new(),
            };
            *slot = Some(loaded);
        }
        slot.as_mut().unwrap()
    }

    fn save(&self, entries: &[String]) {
        // Geçmiş yazılamasa da arama çalışır: hatalar yutulur.
        let _ = self.try_save(entries);
    }

    fn try_save(&self, entries: &[String]) -> std::io::Result<()> {
        if let Some(directory) = std::path::Path::new(&self.file_path).parent() {
            if !directory.as_os_str().is_empty() {
                std::fs::create_dir_all(directory)?;
            }
        }

        // Geçici dosya + taşıma: yarım yazılmış geçmiş kalmaz. `File.WriteAllLines` satırları "\r\n" ile bitirir.
        let temp = format!("{}.tmp", self.file_path);
        let mut text = String::new();
        for e in entries {
            text.push_str(e);
            text.push_str("\r\n");
        }
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, &self.file_path)
    }
}

use std::collections::HashSet;

use chrono::NaiveDateTime;

use crate::text;
use crate::win_path;

/// Son kullanılanlar klasöründeki ham kayıt (kısayol dosyası yolu ve son yazılma zamanı, UTC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentEntry {
    pub shortcut_path: String,
    pub last_write_utc: NaiveDateTime,
}

impl RecentEntry {
    pub fn new(shortcut_path: impl Into<String>, last_write_utc: NaiveDateTime) -> Self {
        RecentEntry { shortcut_path: shortcut_path.into(), last_write_utc }
    }
}

/// Dock'ta gösterilen son kullanılan dosya: kullanıcıya görünen ad ve açmak için verilecek yol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentFile {
    pub name: String,
    pub open_path: String,
    pub last_used_utc: NaiveDateTime,
}

impl RecentFile {
    pub fn new(name: impl Into<String>, open_path: impl Into<String>, last_used_utc: NaiveDateTime) -> Self {
        RecentFile { name: name.into(), open_path: open_path.into(), last_used_utc }
    }
}

/// Son kullanılan dosyaları okuma sözleşmesi. Yalnız dock'taki yığın açılınca çağrılır; sürekli izleme yoktur.
pub trait IRecentFilesService: Send + Sync {
    /// `max` C# varsayılanı: `RecentFilesPolicy::MAX_ITEMS`.
    fn get_recent(&self, max: i32) -> Vec<RecentFile>;
}

/// Son kullanılanlar listesini süzer ve sıralar (saf mantık).
pub struct RecentFilesPolicy;

impl RecentFilesPolicy {
    /// Dock'ta gösterilen en çok öğe sayısı.
    pub const MAX_ITEMS: i32 = 10;

    /// Ham kayıtlardan görünecek listeyi üretir: yeniden eskiye sıralı, .lnk olmayan ve gizli/sistem dosyaları
    /// (desktop.ini) atılmış, aynı adlılar teke indirilmiş, en çok `max` öğe (üst sınır `MAX_ITEMS`; C# varsayılanı
    /// `MAX_ITEMS`). Ad, ".lnk" uzantısı atılmış dosya adıdır; açmak için kısayolun kendisi verilir (Windows hedefi açar).
    pub fn select(entries: &[RecentEntry], max: i32) -> Vec<RecentFile> {
        let limit = text::clamp(max, 0, Self::MAX_ITEMS);
        if limit == 0 {
            return Vec::new();
        }

        let mut seen: HashSet<String> = HashSet::new();
        let mut result: Vec<RecentFile> = Vec::new();

        // OrderByDescending kararlıdır.
        let mut ordered: Vec<&RecentEntry> = entries.iter().collect();
        ordered.sort_by(|a, b| b.last_write_utc.cmp(&a.last_write_utc));

        for entry in ordered {
            if text::is_null_or_white_space(Some(&entry.shortcut_path)) {
                continue;
            }

            let file_name = win_path::get_file_name(&entry.shortcut_path);
            if !text::ends_with_ignore_case(file_name, ".lnk") {
                continue;
            }

            let chars: Vec<char> = file_name.chars().collect();
            let name: String = chars[..chars.len() - 4].iter().collect();
            if text::is_null_or_white_space(Some(&name)) || !seen.insert(text::to_upper_invariant(&name)) {
                continue;
            }

            result.push(RecentFile::new(name, entry.shortcut_path.clone(), entry.last_write_utc));
            if result.len() as i32 == limit {
                break;
            }
        }

        result
    }
}

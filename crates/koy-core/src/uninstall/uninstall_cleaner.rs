//! Karşılığı: `Uninstall/UninstallCleaner.cs`.

use std::fs;
use std::path::Path;

use super::uninstall_plan::UninstallPlan;

/// Silme sonucu: kaç dosya silindi, hangileri silinemedi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallCleanResult {
    pub deleted: i32,
    pub failed: Vec<String>,
}

/// `UninstallPlan`'ın listesini siler. Hiçbir koşulda hata fırlatmaz: kaldırma akışı
/// kilitli bir dosya yüzünden durmamalı.
pub struct UninstallCleaner;

impl UninstallCleaner {
    pub fn run(roaming_dir: &str, local_dir: &str, keep_settings: bool) -> UninstallCleanResult {
        let mut deleted = 0;
        let mut failed: Vec<String> = Vec::new();

        for file in UninstallPlan::files_to_delete(roaming_dir, local_dir, keep_settings) {
            // File.Exists: dosya yoksa (ya da klasörse) atla.
            if !Path::new(&file).is_file() {
                continue;
            }
            match fs::remove_file(&file) {
                Ok(()) => deleted += 1,
                Err(_) => failed.push(file),
            }
        }

        // Ayarlar silindiyse boşalan %AppData%\Koy klasörü de gider. Yalnız boşsa: Directory.Delete özyinelemesiz,
        // başka bir şey kaldıysa (kullanıcının dosyası) dokunmaz.
        if !keep_settings {
            try_remove_empty_directory(roaming_dir);
        }

        UninstallCleanResult { deleted, failed }
    }
}

fn try_remove_empty_directory(dir: &str) {
    // Boş klasör kalması zararsız: hata yutulur.
    let path = Path::new(dir);
    if !path.is_dir() {
        return;
    }
    let is_empty = match fs::read_dir(path) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => return,
    };
    if is_empty {
        let _ = fs::remove_dir(path);
    }
}

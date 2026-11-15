//! Raf: dosya, klasör ve metin parçalarını `%AppData%\Koy\shelf.json` içinde tutar.

use std::path::PathBuf;
use std::sync::Mutex;

use koy_core::event::Event;
use koy_core::shelf::{IShelfService, ShelfItem, ShelfItemKind};
use koy_core::{text, win_path};

use super::shelf_json_context::ShelfJsonContext;

pub struct ShelfService {
    file_path: PathBuf,
    items: Mutex<Vec<ShelfItem>>,
    items_changed: Event<()>,
}

fn default_path() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("Koy").join("shelf.json")
}

fn new_id() -> String {
    koy_core::random::get_bytes(16).iter().map(|b| format!("{b:02x}")).collect()
}

impl ShelfService {
    pub fn new(file_path: Option<PathBuf>) -> Self {
        let service = ShelfService { file_path: file_path.unwrap_or_else(default_path), items: Mutex::new(Vec::new()), items_changed: Event::new() };
        service.load();
        service
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<ShelfItem>> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn load(&self) {
        let mut items = self.lock();
        let Ok(bytes) = std::fs::read(&self.file_path) else { return };
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
        match ShelfJsonContext::deserialize(&String::from_utf8_lossy(bytes)) {
            Ok(Some(loaded)) => *items = loaded,
            Ok(None) => {}
            Err(_) => items.clear(),
        }
    }

    fn save(&self, items: &[ShelfItem]) {
        if let Some(dir) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&self.file_path, ShelfJsonContext::serialize(items));
    }

    fn mutate(&self, change: impl FnOnce(&mut Vec<ShelfItem>) -> bool) {
        let changed = {
            let mut items = self.lock();
            let changed = change(&mut items);
            if changed {
                self.save(&items);
            }
            changed
        };
        if changed {
            self.items_changed.invoke(&());
        }
    }
}

impl IShelfService for ShelfService {
    fn items(&self) -> Vec<ShelfItem> {
        self.lock().clone()
    }

    fn items_changed(&self) -> &Event<()> {
        &self.items_changed
    }

    fn add_file(&self, path: &str) {
        if path.trim().is_empty() {
            return;
        }
        let Some(full_path) = win_path::get_full_path(path) else { return };
        let is_dir = std::path::Path::new(&full_path).is_dir();
        let mut name = if is_dir {
            win_path::get_file_name(full_path.trim_end_matches('\\')).to_string()
        } else {
            win_path::get_file_name(&full_path).to_string()
        };
        if name.is_empty() {
            name = full_path.clone();
        }
        let kind = if is_dir { ShelfItemKind::Directory } else { ShelfItemKind::File };
        let item = ShelfItem::new(new_id(), name, full_path.clone(), kind, chrono::Utc::now().fixed_offset());

        self.mutate(|items| {
            // Zaten aynı yol varsa önce eskiyi kaldırıp en başa ekle.
            items.retain(|i| !text::eq_ignore_case(&i.path_or_content, &full_path));
            items.insert(0, item);
            true
        });
    }

    fn add_text(&self, content: &str) {
        if content.trim().is_empty() {
            return;
        }
        let mut display_name = content.trim().to_string();
        // C#: Length > 30 (UTF-16) → ilk 30 birim + "...".
        let units: Vec<u16> = display_name.encode_utf16().collect();
        if units.len() > 30 {
            display_name = String::from_utf16_lossy(&units[..30]) + "...";
        }
        let item = ShelfItem::new(new_id(), display_name, content, ShelfItemKind::Text, chrono::Utc::now().fixed_offset());
        self.mutate(|items| {
            items.insert(0, item);
            true
        });
    }

    fn remove(&self, id: &str) {
        self.mutate(|items| {
            let before = items.len();
            items.retain(|i| i.id != id);
            items.len() != before
        });
    }

    fn clear(&self) {
        self.mutate(|items| {
            if items.is_empty() {
                return false;
            }
            items.clear();
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_text_file_remove_and_persist() {
        let dir = std::env::temp_dir().join(format!("koy-shelf-test-{}", std::process::id()));
        let path = dir.join("shelf.json");
        let _ = std::fs::remove_dir_all(&dir);
        let shelf = ShelfService::new(Some(path.clone()));
        shelf.add_text("  bir iki üç dört beş altı yedi sekiz dokuz on  ");
        shelf.add_file(&std::env::temp_dir().to_string_lossy());
        let items = shelf.items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, ShelfItemKind::Directory);
        assert!(items[1].display_name.ends_with("..."));

        let reloaded = ShelfService::new(Some(path));
        assert_eq!(reloaded.items(), items);
        reloaded.remove(&items[0].id);
        assert_eq!(reloaded.items().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

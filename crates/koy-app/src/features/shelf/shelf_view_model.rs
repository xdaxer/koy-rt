//! Raf: bırakılan dosya, klasör ve metinler. Karşılığı: `Features/Shelf/ShelfViewModel.cs` ve `ShelfBadgeConverter.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use koy_core::event::Subscription;
use koy_core::shelf::{IShelfService, ShelfItem, ShelfItemKind};

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ole_dnd::{self, DropData};
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;

pub type SharedShelf = Arc<dyn IShelfService + Send + Sync>;

pub struct ShelfViewModel {
    shelf: SharedShelf,
    items: RefCell<Vec<ShelfItem>>,
    /// Başlıktaki "3 öğe".
    count_text: RefCell<String>,
    pub changed: Notifier,
    sub: Cell<Option<Subscription>>,
    disposed: Cell<bool>,
}

impl ShelfViewModel {
    pub fn new(shelf: SharedShelf) -> Rc<Self> {
        let vm = Rc::new(ShelfViewModel {
            shelf,
            items: RefCell::new(Vec::new()),
            count_text: RefCell::new(String::new()),
            changed: Notifier::default(),
            sub: Cell::new(None),
            disposed: Cell::new(false),
        });
        vm.update_items();
        let me = UiRef::new(&vm);
        let sub = vm.shelf.items_changed().subscribe(move |_| {
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && !vm.disposed.get()
                {
                    vm.update_items();
                    vm.changed.notify();
                }
            });
        });
        vm.sub.set(Some(sub));
        vm
    }

    pub fn items(&self) -> std::cell::Ref<'_, Vec<ShelfItem>> {
        self.items.borrow()
    }

    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    pub fn count_text(&self) -> String {
        self.count_text.borrow().clone()
    }

    fn update_items(&self) {
        let items = self.shelf.items();
        *self.count_text.borrow_mut() = Strings::format("ShelfCount", &[&items.len().to_string()]);
        *self.items.borrow_mut() = items;
    }

    pub fn remove_item(&self, id: &str) {
        self.shelf.remove(id);
    }

    pub fn clear_all(&self) {
        self.shelf.clear();
    }

    /// Çift tıklama: dosya ya da klasör varsayılan uygulamada açılır.
    pub fn open_item(&self, index: usize) {
        let item = self.items.borrow().get(index).cloned();
        if let Some(item) = item
            && matches!(item.kind, ShelfItemKind::File | ShelfItemKind::Directory)
        {
            koy_platform::wireless::SettingsUri::open(&item.path_or_content);
        }
    }

    /// Öğeyi başka uygulamaya sürükler: var olan dosya / klasör ya da metin.
    pub fn drag_item(&self, index: usize) {
        let Some(item) = self.items.borrow().get(index).cloned() else { return };
        match item.kind {
            ShelfItemKind::File | ShelfItemKind::Directory => {
                if std::path::Path::new(&item.path_or_content).exists() {
                    ole_dnd::drag_file(&item.path_or_content);
                }
            }
            ShelfItemKind::Text => ole_dnd::drag_text(&item.path_or_content),
        }
    }

    pub fn add_files<'a>(&self, paths: impl IntoIterator<Item = &'a str>) {
        for p in paths {
            if !p.trim().is_empty() {
                self.shelf.add_file(p);
            }
        }
    }

    pub fn handle_drop(&self, data: &DropData) {
        match data {
            DropData::Files(files) if !files.is_empty() => self.add_files(files.iter().map(String::as_str)),
            DropData::Text(text) if !text.trim().is_empty() => self.shelf.add_text(text),
            _ => {}
        }
    }

    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        if let Some(sub) = self.sub.take() {
            self.shelf.items_changed().unsubscribe(sub);
        }
    }
}

/// Raf kutucuğundaki kısa etiket: dosyada uzantı ("PDF"), klasörde "DIR", metinde "TXT".
pub fn badge(item: &ShelfItem) -> String {
    match item.kind {
        ShelfItemKind::Directory => "DIR".into(),
        ShelfItemKind::Text => "TXT".into(),
        ShelfItemKind::File => {
            // C# Path.GetExtension: son ayraçtan sonraki addaki son noktadan sonrası (".gitignore" → "gitignore").
            let name = item.path_or_content.rsplit(['\\', '/']).next().unwrap_or("");
            let ext = name.rfind('.').map(|i| name[i + 1..].to_uppercase()).unwrap_or_default();
            match ext.chars().count() {
                0 => "DOSYA".into(),
                n if n > 4 => ext.chars().take(4).collect(),
                _ => ext,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, kind: ShelfItemKind) -> ShelfItem {
        ShelfItem::new("1", "x", path, kind, chrono::Utc::now().fixed_offset())
    }

    #[test]
    fn badges() {
        assert_eq!(badge(&item(r"C:\a\b.pdf", ShelfItemKind::File)), "PDF");
        assert_eq!(badge(&item(r"C:\a\b.jpeg2000", ShelfItemKind::File)), "JPEG");
        assert_eq!(badge(&item(r"C:\a\b", ShelfItemKind::File)), "DOSYA");
        assert_eq!(badge(&item(r"C:\a\.gitignore", ShelfItemKind::File)), "GITI");
        assert_eq!(badge(&item(r"C:\a.b\c", ShelfItemKind::File)), "DOSYA");
        assert_eq!(badge(&item(r"C:\a", ShelfItemKind::Directory)), "DIR");
        assert_eq!(badge(&item("merhaba", ShelfItemKind::Text)), "TXT");
    }
}

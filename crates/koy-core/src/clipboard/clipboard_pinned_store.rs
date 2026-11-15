use super::ClipboardItem;
use crate::settings::json_list_file::JsonErrorCallback;
use crate::settings::{in_settings_folder, JsonListFile};
use std::path::{Path, PathBuf};

/// Yalnız sabitlenen pano öğelerinin dosyası (%AppData%\Koy\clipboard-pinned.json).
pub struct ClipboardPinnedStore {
    file: JsonListFile<ClipboardItem>,
}

impl ClipboardPinnedStore {
    /// `path` verilmezse ayar klasöründeki `clipboard-pinned.json`. `on_error`: C#'taki `Action<string, Exception>`.
    pub fn new(path: Option<PathBuf>, on_error: Option<JsonErrorCallback>) -> Self {
        let path = path.unwrap_or_else(|| in_settings_folder("clipboard-pinned.json"));
        ClipboardPinnedStore { file: JsonListFile::new(path, on_error) }
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    pub fn load(&self) -> Vec<ClipboardItem> {
        self.file.load()
    }

    pub fn save<'a>(&self, pinned: impl IntoIterator<Item = &'a ClipboardItem>) -> bool {
        let list: Vec<ClipboardItem> = pinned.into_iter().filter(|i| i.pinned).cloned().collect();
        self.file.save(&list)
    }
}

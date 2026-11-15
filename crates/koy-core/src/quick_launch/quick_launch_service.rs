use std::sync::Arc;

use crate::event::Event;
use crate::shell::IShellActionService;
use crate::text;

/// `IQuickLaunchService.MaxItems`: listedeki en çok öğe sayısı. (Rust'ta ilişkili sabit içeren trait nesne olarak
/// kullanılamayacağından modül sabiti.)
pub const MAX_ITEMS: usize = 8;

/// Hızlı başlat listesi: kullanıcının eklediği uygulama / dosya / klasör yolları. Sıra korunur, en çok
/// [`MAX_ITEMS`], aynı yol (büyük/küçük harf duyarsız) bir kez. Kalıcılık AppSettings.QuickLaunchItems'tır:
/// servis her değişiklikte kaydetme geri çağrısını çağırır (App ayarları yazar). Simge: Platform'daki
/// IconHelper.GetIconBytes(path). UI iş parçacığından kullanılır; `changed` değişikliği yapan iş
/// parçacığında tetiklenir.
pub trait IQuickLaunchService {
    fn items(&self) -> &[String];

    fn changed(&self) -> &Event<()>;

    /// Yol eklenir (tırnaklar ve boşluklar temizlenir). Boş, bulunamayan, tekrar ya da liste doluysa false.
    fn add(&mut self, path: &str) -> bool;

    fn remove(&mut self, path: &str) -> bool;

    /// Öğeyi `from_index`'ten `to_index`'e taşır. Geçersiz indeks için false.
    fn move_item(&mut self, from_index: i32, to_index: i32) -> bool;

    /// Kabuk servisi ile başlatır. Başlatılamazsa (silinmiş yol, izin) false; çökme yok.
    fn launch(&self, path: &str) -> bool;
}

/// Kaydetme geri çağrısı: güncel listenin kopyası.
pub type SaveCallback = Box<dyn Fn(&[String]) + Send + Sync>;

/// Yol var mı denetimi (testlerde sahte).
pub type ExistsCallback = Box<dyn Fn(&str) -> bool + Send + Sync>;

/// Hata günlüğü: `(kaynak, hata iletisi)`. C#'ta `Diagnostics.CrashLog.Write(source, ex)`; `quick_launch`
/// özelliği `diagnostics`'e bağlı olmadığından dışarıdan verilir (varsayılan: günlüksüz).
pub type ErrorLog = Box<dyn Fn(&str, &str) + Send + Sync>;

pub struct QuickLaunchService {
    items: Vec<String>,
    save: SaveCallback,
    shell: Arc<dyn IShellActionService>,
    exists: ExistsCallback,
    error_log: Option<ErrorLog>,
    changed: Event<()>,
}

impl QuickLaunchService {
    /// `exists` verilmezse dosya ya da klasör var mı (`File.Exists || Directory.Exists`) bakılır.
    pub fn new<I>(initial: I, save: SaveCallback, shell: Arc<dyn IShellActionService>, exists: Option<ExistsCallback>) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let exists = exists.unwrap_or_else(|| Box::new(|p: &str| std::path::Path::new(p).is_file() || std::path::Path::new(p).is_dir()));

        let mut service = QuickLaunchService { items: Vec::new(), save, shell, exists, error_log: None, changed: Event::new() };

        // Elle bozulmuş ayar dosyasına karşı: temizle, tekrarları at, sınırı uygula (var olmayanlar kalır, kullanıcı kaldırır).
        for raw in initial {
            let path = Self::normalize(Some(raw.as_ref()));
            if let Some(path) = path
                && !service.contains(&path)
                && service.items.len() < MAX_ITEMS
            {
                service.items.push(path);
            }
        }
        service
    }

    /// Başlatma hatalarının yazılacağı günlüğü bağlar (C#'taki `CrashLog.Write("QuickLaunchService.Launch", ex)`).
    pub fn set_error_log(&mut self, error_log: ErrorLog) {
        self.error_log = Some(error_log);
    }

    /// Listede gösterilecek ad: dosyada uzantısız ad, klasörde klasör adı.
    pub fn display_name(path: &str) -> String {
        let trimmed = path.trim_end_matches(['\\', '/']);
        let name = crate::win_path::get_file_name(trimmed).to_owned();
        if name.is_empty() {
            return trimmed.to_owned();
        }
        let without_ext = crate::win_path::get_file_name_without_extension(&name).to_owned();
        if !without_ext.is_empty() { without_ext } else { name }
    }

    fn normalize(path: Option<&str>) -> Option<String> {
        if text::is_null_or_white_space(path) {
            return None;
        }
        let p = text::trim(path?).trim_matches('"');
        let p = text::trim(p);
        if p.is_empty() { None } else { Some(p.to_owned()) }
    }

    fn same(a: &str, b: &str) -> bool {
        text::eq_ignore_case(a.trim_end_matches(['\\', '/']), b.trim_end_matches(['\\', '/']))
    }

    fn contains(&self, path: &str) -> bool {
        self.items.iter().any(|p| Self::same(p, path))
    }

    fn commit(&self) {
        (self.save)(&self.items.clone());
        self.changed.invoke(&());
    }
}

impl IQuickLaunchService for QuickLaunchService {
    fn items(&self) -> &[String] {
        &self.items
    }

    fn changed(&self) -> &Event<()> {
        &self.changed
    }

    fn add(&mut self, path: &str) -> bool {
        let Some(normalized) = Self::normalize(Some(path)) else {
            return false;
        };
        if self.items.len() >= MAX_ITEMS || self.contains(&normalized) || !(self.exists)(&normalized) {
            return false;
        }

        self.items.push(normalized);
        self.commit();
        true
    }

    fn remove(&mut self, path: &str) -> bool {
        let normalized = Self::normalize(Some(path));
        let index = normalized.and_then(|n| self.items.iter().position(|p| Self::same(p, &n)));
        let Some(index) = index else {
            return false;
        };

        self.items.remove(index);
        self.commit();
        true
    }

    fn move_item(&mut self, from_index: i32, to_index: i32) -> bool {
        let count = self.items.len() as i32;
        if from_index < 0 || from_index >= count || to_index < 0 || to_index >= count {
            return false;
        }
        if from_index == to_index {
            return true;
        }

        let item = self.items.remove(from_index as usize);
        self.items.insert(to_index as usize, item);
        self.commit();
        true
    }

    fn launch(&self, path: &str) -> bool {
        match self.shell.launch(path, None) {
            Ok(()) => true,
            Err(message) => {
                if let Some(log) = &self.error_log {
                    log("QuickLaunchService.Launch", &message);
                }
                false
            }
        }
    }
}

// --- .NET `System.IO.Path` (Windows) karşılıkları: yalnız `DisplayName` için gerekenler ---


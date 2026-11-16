//! Kullanıcı stil dosyasını (%AppData%\Koy\user-theme.json) izler (C# `UserStyleWatcher`, FileSystemWatcher
//! karşılığı: klasör değişiklik bildirimi). Olaylar 300 ms birleştirilir, sonra arayüz iş parçacığında yeniden yükleme
//! çalışır. Yoklama yok: iş parçacığı bildirim beklerken uyur.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use koy_core::appearance::UserStyleFile;
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE_CREATION, FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE, FindCloseChangeNotification,
    FindFirstChangeNotificationW, FindNextChangeNotification,
};
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::HSTRING;

use crate::infra::dispatch;

const DEBOUNCE: Duration = Duration::from_millis(300);

pub struct UserStyleWatcher {
    disposed: Arc<AtomicBool>,
}

struct ChangeHandle(HANDLE);

// SAFETY: bildirim tutamacı yalnız izleme iş parçacığında kullanılır.
unsafe impl Send for ChangeHandle {}

impl UserStyleWatcher {
    /// `reload` arayüz iş parçacığında çağrılır.
    pub fn new(reload: fn()) -> UserStyleWatcher {
        Self::watch_folder(UserStyleFile::default_folder(), reload)
    }

    /// Verilen klasörü izler (Koy penceresi settings.json için de kullanır: C# FileSystemWatcher).
    pub fn watch_folder(folder: std::path::PathBuf, reload: fn()) -> UserStyleWatcher {
        let disposed = Arc::new(AtomicBool::new(false));
        // İzlenecek klasör yoksa oluşturulur (yalnız klasör; dosya kullanıcıya aittir ve Koy yazmaz).
        let _ = std::fs::create_dir_all(&folder);
        let filter = FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_SIZE | FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_CREATION;
        // SAFETY: klasör yolu çağrı süresince geçerli.
        let handle = unsafe { FindFirstChangeNotificationW(&HSTRING::from(folder.as_os_str()), false, filter) };
        // İzleme başarısızsa "Yeniden yükle" düğmesi yine çalışır.
        if let Ok(h) = handle {
            let h = ChangeHandle(h);
            let stop = Arc::clone(&disposed);
            let _ = std::thread::Builder::new().name("Koy stil izleyici".into()).spawn(move || {
                let h = h;
                while !stop.load(Ordering::SeqCst) {
                    // SAFETY: tutamaç bu iş parçacığına ait; 500 ms dilimlerle beklenir ki bırakma fark edilsin.
                    if unsafe { WaitForSingleObject(h.0, 500) } != WAIT_OBJECT_0 {
                        continue;
                    }
                    // Birleştirme: sessizlik 300 ms sürene dek gelen olaylar tek yeniden yüklemeye iner.
                    loop {
                        // SAFETY: bildirim yeniden kurulur.
                        if unsafe { FindNextChangeNotification(h.0) }.is_err() {
                            break;
                        }
                        if unsafe { WaitForSingleObject(h.0, DEBOUNCE.as_millis() as u32) } != WAIT_OBJECT_0 {
                            break;
                        }
                    }
                    if !stop.load(Ordering::SeqCst) {
                        let stop2 = Arc::clone(&stop);
                        dispatch::post(move || {
                            if !stop2.load(Ordering::SeqCst) {
                                reload();
                            }
                        });
                    }
                }
                // SAFETY: tutamaç bir kez kapatılır.
                let _ = unsafe { FindCloseChangeNotification(h.0) };
            });
        }
        UserStyleWatcher { disposed }
    }

    pub fn dispose(&self) {
        self.disposed.store(true, Ordering::SeqCst);
    }
}

impl Drop for UserStyleWatcher {
    fn drop(&mut self) {
        self.dispose();
    }
}

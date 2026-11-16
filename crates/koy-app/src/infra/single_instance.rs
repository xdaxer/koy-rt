//! Oturum başına tek Koy süreci. İki Koy aynı anda görev çubuğunu gizleyip geri getirmeye ve sistem tepsisini
//! (Shell_TrayWnd) barındırmaya çalışır; biri çökünce öteki görev çubuğunu gizli bırakabilir. İkinci açılış ilk
//! sürece Koy penceresini açtırır ve çıkar.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateEventW, CreateMutexW, ReleaseMutex, SetEvent, WaitForSingleObject};
use windows::core::w;

struct Handle(HANDLE);

// SAFETY: çekirdek nesne tutamaçları iş parçacıkları arasında taşınabilir.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: tutamaç bir kez kapatılır.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub struct SingleInstance {
    mutex: Handle,
    activate: Arc<Handle>,
    stop: Arc<AtomicBool>,
}

impl SingleInstance {
    /// İlk süreçse sahipliği döndürür; başka Koy çalışıyorsa ona haber verir ve `None` döner.
    pub fn try_acquire() -> Option<SingleInstance> {
        // SAFETY: adlı çekirdek nesneler; hata kodu hemen okunur.
        unsafe {
            let mutex = CreateMutexW(None, true, w!(r"Local\Koy.SingleInstance")).ok()?;
            let created_new = GetLastError() != ERROR_ALREADY_EXISTS;
            let mutex = Handle(mutex);
            let activate = Handle(CreateEventW(None, false, false, w!(r"Local\Koy.SingleInstance.Activate")).ok()?);
            if created_new {
                return Some(SingleInstance { mutex, activate: Arc::new(activate), stop: Arc::new(AtomicBool::new(false)) });
            }
            let _ = SetEvent(activate.0);
            None
        }
    }

    /// Başka bir açılış denemesinde çağrılır (arka plan iş parçacığından; çağıran arayüze kendisi taşır).
    pub fn listen_for_activation(&self, on_activate: impl Fn() + Send + 'static) {
        let event = Arc::clone(&self.activate);
        let stop = Arc::clone(&self.stop);
        let _ = std::thread::Builder::new().name("Koy tek örnek".into()).spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                // SAFETY: olay tutamacı Arc ile yaşar; 500 ms dilimlerle beklenir ki durdurma fark edilsin.
                if unsafe { WaitForSingleObject(event.0, 500) } == WAIT_OBJECT_0 && !stop.load(Ordering::SeqCst) {
                    on_activate();
                }
            }
        });
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Sahiplik başka iş parçacığındaysa sistem süreç bitince bırakır.
        // SAFETY: kendi muteksimiz.
        let _ = unsafe { ReleaseMutex(self.mutex.0) };
    }
}

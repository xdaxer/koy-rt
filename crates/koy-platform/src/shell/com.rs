//! İş parçacığı başına COM başlatma. C#'ta thread-pool iş parçacıkları otomatik MTA'dır; Rust'ta çağıran iş
//! parçacığı COM'u başlatmadıysa kabuk nesneleri (IShellItem, ShellLink) çalışmaz. Başka kipte zaten
//! başlatılmışsa (STA arayüz iş parçacığı) `RPC_E_CHANGED_MODE` yok sayılır; o iş parçacığında COM kullanılabilir.

use std::cell::Cell;

use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

thread_local! {
    static INITIALIZED: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn ensure_initialized() {
    INITIALIZED.with(|done| {
        if !done.get() {
            // SAFETY: iş parçacığı başına bir kez; çift başlatma zararsızdır. Kapatma süreç sonunda (iş parçacığı
            // ömrü boyunca açık kalması istenir).
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            done.set(true);
        }
    });
}

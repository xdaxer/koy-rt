//! Bu klasörlerin (Bluetooth, Wireless, Radio, Network, Power, Display) ortak küçük yardımcıları.
//! C#'ta karşılığı yok: orada iş parçacığı havuzu COM'u kendisi başlatır, `CrashLog.Write(kaynak, ex)` istisnayı doğrudan alır.

use koy_core::diagnostics::{CrashError, CrashLog};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

/// İş parçacığı başına `CoInitializeEx(MTA)`; düşünce `CoUninitialize`. İş parçacığı zaten başka modda
/// başlatılmışsa (`RPC_E_CHANGED_MODE`) COM yine kullanılabilir olduğundan sessizce geçilir, kapatma yapılmaz.
pub(crate) struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    pub(crate) fn enter_mta() -> Self {
        // SAFETY: parametresiz çağrı; başarılıysa Drop'ta eşleşen CoUninitialize yapılır.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        ComApartment { initialized: hr.is_ok() }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: enter_mta içindeki başarılı CoInitializeEx ile eşleşir.
            unsafe { CoUninitialize() };
        }
    }
}

/// `CrashLog.Write(kaynak, ex)` karşılığı: WinRT / COM hatasını günlüğe yazar.
pub(crate) fn log_error(source: &str, error: &windows::core::Error) {
    let type_name = if error.code().0 < 0 { "System.Runtime.InteropServices.COMException" } else { "System.Exception" };
    CrashLog::write(source, &CrashError::from_error(error, type_name, ""));
}

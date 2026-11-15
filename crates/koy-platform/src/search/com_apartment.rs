//! Karşılığı: C#'taki `Thread.SetApartmentState` / iş parçacığı havuzunun örtük MTA'sı (ortak yardımcı, dosya karşılığı yok).
//!
//! COM iş parçacığı başına başlatılır (PORTING.md: "iş parçacığı başına `CoInitializeEx`"). Arama, atlama listesi ve
//! başlatma işleri kendi kısa ömürlü iş parçacıklarında bu korumayı kullanır.

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED};

/// Oluşturulduğu iş parçacığında COM'u başlatır, düşünce (başarılı başlatmışsa) `CoUninitialize` çağırır.
/// Bu koruma, ondan sonra oluşturulan tüm COM nesnelerinden SONRA düşecek biçimde (önce bildirilerek) tutulmalıdır.
pub(crate) struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    /// Tek iş parçacıklı apartman (kabuk nesneleri için).
    pub(crate) fn sta() -> Self {
        Self::init(COINIT_APARTMENTTHREADED)
    }

    /// Çok iş parçacıklı apartman (iş parçacığı havuzunun varsayılanı).
    pub(crate) fn mta() -> Self {
        Self::init(COINIT_MULTITHREADED)
    }

    fn init(mode: COINIT) -> Self {
        // SAFETY: iş parçacığı başına çağrı; başarılıysa Drop'ta eşleşen CoUninitialize yapılır.
        // RPC_E_CHANGED_MODE (iş parçacığı zaten başka modda) başarısız sayılır ve uninit çağrılmaz.
        let hr = unsafe { CoInitializeEx(None, mode) };
        ComApartment { initialized: hr.is_ok() }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: yukarıdaki başarılı CoInitializeEx ile eşleşir.
            unsafe { CoUninitialize() };
        }
    }
}

thread_local! {
    static MTA: ComApartment = ComApartment::mta();
}

/// Çağıran iş parçacığında (henüz yoksa) MTA başlatır; iş parçacığı bitince kendiliğinden kapanır. Hangi iş
/// parçacığından çağrılacağı bilinmeyen servisler için (C#'ta iş parçacığı havuzu MTA'dır).
pub(crate) fn ensure_mta() {
    MTA.with(|_| {});
}

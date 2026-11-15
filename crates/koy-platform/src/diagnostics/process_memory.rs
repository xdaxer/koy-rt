//! Süreç bellek ölçümü. Karşılığı: `koy/src/Platform/Diagnostics/ProcessMemory.cs`.

use std::mem::size_of;

use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2};
use windows::Win32::System::Threading::GetCurrentProcess;

/// Süreç bellek ölçümü.
pub struct ProcessMemory;

impl ProcessMemory {
    /// Koy'un özel çalışma kümesi (private working set), bayt. Görev Yöneticisi'nin "Bellek" sütunuyla aynı ölçü
    /// (`PrivateMemorySize64` ayrılmış/commit belleği verir, bu değil). Tek ucuz çağrı, süreç taraması yok.
    /// Ölçülemezse 0.
    pub fn get_private_working_set_bytes() -> i64 {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX2 { cb: size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32, ..Default::default() };
        // SAFETY: sahte süreç tanıtıcısı (-1) kapatılmaz; counters geçerli boyutlu, EX2 yapısı PROCESS_MEMORY_COUNTERS ile
        // başlayan bir düzene sahiptir ve cb gerçek boyutu bildirir.
        let ok = unsafe {
            K32GetProcessMemoryInfo(GetCurrentProcess(), (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX2).cast::<PROCESS_MEMORY_COUNTERS>(), counters.cb)
        };
        if !ok.as_bool() {
            return 0;
        }

        counters.PrivateWorkingSetSize as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_working_set_is_positive_for_this_process() {
        assert!(ProcessMemory::get_private_working_set_bytes() > 0);
    }
}

//! Koy'un görev çubuğu bekçisi. `koy-watchdog.exe --pid N`: Koy süreci N'i bekler.
//! Koy düzgün kapanırsa (temiz çıkış olayı) hiçbir şey yapmaz; Koy ölürse Windows görev çubuğunu geri getirir
//! (AGENTS.md #7). Penceresi, konsolu ve ayar dosyası yoktur; durumu Koy'un yazdığı taskbar-state.json'dan okur.

#![windows_subsystem = "windows"]

#[cfg(windows)]
fn main() {
    std::process::exit(app::run());
}

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
mod app {
    use koy_core::shell::{WatchdogAction, WatchdogPolicy, WatchdogWaitResult};
    use koy_platform::shell::taskbar_service;
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        CreateMutexW, INFINITE, OpenEventW, OpenProcess, PROCESS_SYNCHRONIZE, SYNCHRONIZATION_SYNCHRONIZE,
        WaitForMultipleObjects,
    };
    use windows::core::HSTRING;

    pub fn run() -> i32 {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let Some(koy_pid) = WatchdogPolicy::try_parse_process_id(&args) else { return 2 };

        // Aynı anda tek gözcü. Tutamaç süreç bitene kadar açık kalır.
        // SAFETY: ad geçerli HSTRING.
        let singleton = unsafe { CreateMutexW(None, true, &HSTRING::from(WatchdogPolicy::SINGLETON_MUTEX_NAME)) };
        // SAFETY: CreateMutexW'den hemen sonra okunur.
        if singleton.is_err() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            return 0;
        }

        let result = wait(koy_pid);
        if WatchdogPolicy::decide(result) == WatchdogAction::RestoreTaskbar {
            // Gözcü hiçbir koşulda hata penceresi göstermemeli.
            // Koy'un kendi Shell_TrayWnd penceresi süreçle birlikte gider, yine de PID ile atlanır.
            if std::panic::catch_unwind(|| taskbar_service::restore(koy_pid as u32, false)).is_err() {
                return 1;
            }
        }
        0
    }

    fn wait(koy_pid: i32) -> WatchdogWaitResult {
        // SAFETY: yalnız SYNCHRONIZE hakkıyla açılır, sonunda kapatılır.
        let Ok(process) = (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, koy_pid as u32) }) else {
            return WatchdogWaitResult::ProcessNotFound;
        };

        // Koy olayı gözcüyü başlatmadan önce oluşturur. Yoksa yalnız süreç beklenir.
        let name = HSTRING::from(WatchdogPolicy::stop_event_name(koy_pid));
        // SAFETY: ad geçerli HSTRING.
        let stop_event: Option<HANDLE> = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, false, &name) }.ok();

        // Olay dizide önce: ikisi aynı anda hazırsa (Koy olayı kurup çıktı) temiz çıkış sayılır.
        let handles: Vec<HANDLE> = match stop_event {
            Some(e) => vec![e, process],
            None => vec![process],
        };
        // SAFETY: tutamaçlar açık.
        let wait = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
        // WAIT_OBJECT_0 = 0; zaman aşımı/başarısızlık değerleri (WAIT_FAILED = 0xFFFFFFFF) dizin sınırının dışında kalır.
        let index = wait.0.wrapping_sub(WAIT_OBJECT_0.0);
        let result = if index >= handles.len() as u32 {
            WatchdogWaitResult::Failed
        } else if stop_event.is_some() && index == 0 {
            WatchdogWaitResult::StopRequested
        } else {
            WatchdogWaitResult::ProcessExited
        };

        // SAFETY: tutamaçlar bizim ve bir kez kapatılır.
        unsafe {
            if let Some(e) = stop_event {
                let _ = CloseHandle(e);
            }
            let _ = CloseHandle(process);
        }
        result
    }
}

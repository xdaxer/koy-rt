//! Koy.Watchdog.exe'yi yönetir: Koy görev çubuğunu gizlediği sürece tek bir gözcü çalışır. Görev Yöneticisi
//! TerminateProcess ile Koy'u öldürünce hiçbir Koy kodu çalışmaz; gözcü sürecin kapandığını görüp görev çubuğunu
//! geri getirir. Gözcü exe'si yoksa (geliştirme çalıştırması) sessizce atlanır.

use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use std::sync::Mutex;

use koy_core::shell::WatchdogPolicy;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, CreateEventW, ResetEvent, SetEvent, WaitForSingleObject};
use windows::core::HSTRING;

pub const EXE_NAME: &str = "koy-watchdog.exe";

struct State {
    process: Option<Child>,
    stop_event: Option<HANDLE>,
}

// SAFETY: HANDLE yalnız kilit altında kullanılır; çekirdek nesne tutamaçları iş parçacıkları arasında geçerlidir.
unsafe impl Send for State {}

static GATE: Mutex<State> = Mutex::new(State { process: None, stop_event: None });

pub fn ensure_running() {
    let mut st = GATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = st.process.as_mut()
        && matches!(p.try_wait(), Ok(None))
    {
        return;
    }

    let Some(path) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(EXE_NAME))) else { return };
    if !path.exists() {
        return;
    }

    release_handles(&mut st);

    let pid = std::process::id() as i32;
    // Olay gözcüden önce kurulur ve Koy tutar: gözcü açtığında temiz çıkış işareti kaçmaz.
    let name = HSTRING::from(WatchdogPolicy::stop_event_name(pid));
    // SAFETY: ad geçerli HSTRING; elle sıfırlanan, başlangıçta kapalı olay.
    let Ok(event) = (unsafe { CreateEventW(None, true, false, &name) }) else {
        // Gözcü başlatılamazsa dock çalışmaya devam eder; açılıştaki restore() korumaları yerinde.
        return;
    };
    // SAFETY: event az önce oluşturuldu.
    let _ = unsafe { ResetEvent(event) };
    st.stop_event = Some(event);

    match Command::new(&path)
        .args(WatchdogPolicy::build_arguments(pid).split(' '))
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn()
    {
        Ok(child) => st.process = Some(child),
        Err(_) => release_handles(&mut st),
    }
}

/// Temiz çıkış: gözcü görev çubuğuna dokunmadan kapanır.
pub fn stop() {
    let mut st = GATE.lock().unwrap_or_else(|e| e.into_inner());
    if st.process.is_none() {
        return;
    }

    if let Some(event) = st.stop_event {
        // SAFETY: olay tutamacı bizim; kapatılana kadar geçerli.
        let _ = unsafe { SetEvent(event) };
    }

    // Kısa bekleme: gözcü olayı alıp çıksın ki Koy'un kapanışında yetim kalmasın.
    if let Some(p) = st.process.as_ref() {
        let handle = HANDLE(p.as_raw_handle());
        // SAFETY: Child tutamacı Child yaşadıkça geçerli.
        unsafe { WaitForSingleObject(handle, 300) };
    }

    release_handles(&mut st);
}

fn release_handles(st: &mut State) {
    // Child bırakılınca tutamacı kapanır; süreç öldürülmez (C#'taki Process.Dispose gibi).
    st.process = None;
    if let Some(event) = st.stop_event.take() {
        // SAFETY: tutamaç bizim ve bir kez kapatılır.
        let _ = unsafe { CloseHandle(event) };
    }
}

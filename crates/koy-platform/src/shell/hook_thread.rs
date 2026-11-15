//! Kendi iş parçacığında çalışan düşük düzey kanca (WH_KEYBOARD_LL / WH_MOUSE_LL) için ortak iskelet.
//! C#'ta `KeyboardHookService`, `CornerHookService` ve `OutsideClickHook` aynı `Run` gövdesini kopyalıyordu
//! (iş parçacığı kurulumu, `SetWindowsHookEx`, `GetMessage` döngüsü, `WM_QUIT` ile durdurma); Rust'ta tek yerde.
//! Kanca geri çağrısı bağlam taşımaz: her servis kancayı kuran iş parçacığının `thread_local` alanına kendi durumunu
//! koyar (`setup` geri çağrısı kanca iş parçacığında, kancadan önce çalışır).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

use koy_core::diagnostics::{CrashError, CrashLog};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HHOOK, MSG, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, WINDOWS_HOOK_ID, WM_QUIT,
};

/// Kanca iş parçacığı. `stop` mesaj döngüsünü `WM_QUIT` ile bitirir ve kısa süre bekler.
pub(crate) struct HookThread {
    thread_id: Arc<AtomicU32>,
    installed: Arc<AtomicBool>,
    done: Receiver<()>,
    ready: Receiver<()>,
}

impl HookThread {
    /// Kanca iş parçacığını başlatır. `setup`: kanca iş parçacığında, kancadan önce çalışır (geri çağrının
    /// kullanacağı `thread_local` durumu kurar). `source`: `CrashLog` kaynak adı.
    pub fn spawn(
        name: &str,
        hook_id: WINDOWS_HOOK_ID,
        proc: unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT,
        source: &'static str,
        run_source: &'static str,
        setup: impl FnOnce() + Send + 'static,
    ) -> HookThread {
        let thread_id = Arc::new(AtomicU32::new(0));
        let installed = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel::<()>();

        let tid = Arc::clone(&thread_id);
        let inst = Arc::clone(&installed);
        let spawned = std::thread::Builder::new().name(name.to_string()).spawn(move || {
            let ready_tx2 = ready_tx.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // SAFETY: yalnız geçerli iş parçacığı kimliğini okur.
                tid.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
                setup();
                // SAFETY: null ad, çağıran sürecin modülünü verir.
                let module = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
                // SAFETY: proc geçerli bir `extern "system"` işlev; kanca bu iş parçacığına bağlanır ve aşağıda sökülür.
                let hook = unsafe { SetWindowsHookExW(hook_id, Some(proc), Some(HINSTANCE(module.0)), 0) };
                let hook = match hook {
                    Ok(h) if !h.is_invalid() => Some(h),
                    _ => None,
                };
                inst.store(hook.is_some(), Ordering::SeqCst);
                let _ = ready_tx.send(());

                let Some(hook) = hook else {
                    CrashLog::write(source, &win32_error());
                    return;
                };

                let mut msg = MSG::default();
                // SAFETY: msg yerel değişken; döngü WM_QUIT ya da hata ile biter.
                while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
                    // SAFETY: GetMessage'ın doldurduğu geçerli ileti.
                    unsafe {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }

                unpin(hook);
            }));
            if let Err(payload) = result {
                let message = panic_message(&payload);
                CrashLog::write(run_source, &CrashError::exception("Panic", &message, ""));
                let _ = ready_tx2.send(());
            }
            let _ = done_tx.send(());
        });

        // İş parçacığı açılamadıysa gönderici düşer; `wait_ready` hemen döner ve kurulmadı sayılır.
        drop(spawned);

        HookThread { thread_id, installed, done: done_rx, ready: ready_rx }
    }

    /// Kanca kurulana (ya da başarısız olana) dek en çok `timeout` bekler; kurulu mu döner.
    pub fn wait_ready(&self, timeout: Duration) -> bool {
        let _ = self.ready.recv_timeout(timeout);
        self.is_installed()
    }

    pub fn is_installed(&self) -> bool {
        self.installed.load(Ordering::SeqCst)
    }

    /// `WM_QUIT` gönderir ve iş parçacığının bitmesini en çok `timeout` bekler (C# `Join(timeout)`).
    pub fn stop(self, timeout: Duration) {
        let id = self.thread_id.load(Ordering::SeqCst);
        if id != 0 {
            // SAFETY: yalnız ileti gönderir; iş parçacığı kimliği geçersizse çağrı başarısız olur.
            let _ = unsafe { PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
        let _ = self.done.recv_timeout(timeout);
    }

}

fn unpin(hook: HHOOK) {
    // SAFETY: hook bu iş parçacığında SetWindowsHookEx ile kuruldu.
    let _ = unsafe { UnhookWindowsHookEx(hook) };
}

/// `CallNextHookEx(HHOOK.Null, ...)`.
pub(crate) fn call_next(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: kanca zincirinin sonraki halkasına olduğu gibi iletilir.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// C# `new Win32Exception()` karşılığı: son Win32 hatasından.
pub(crate) fn win32_error() -> CrashError {
    let error = windows::core::Error::from_thread();
    CrashError::exception("Win32Exception", &error.message(), "")
}

pub(crate) fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "(panik iletisi yok)".to_string()
    }
}

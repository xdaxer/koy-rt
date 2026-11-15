//! Win32 pano dinleyicisi (AddClipboardFormatListener, WM_CLIPBOARDUPDATE). Yoklama yok: pano değişmedikçe iş parçacığı
//! GetMessage'da uyur (CPU %0). Gizli mesaj penceresi kendi iş parçacığında çalışır; arayüz iş parçacığı pano sahibi
//! uygulamayı beklemez. Dinleyici yalnız `start` ile kurulur, `stop` ile pencere ve iş parçacığı kalkar. `changed` ARKA
//! PLAN iş parçacığından gelebilir.

use std::cell::RefCell;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use koy_core::clipboard::{ClipboardFilter, ClipboardHistory, ClipboardItem, ClipboardPinnedStore, Guid, IClipboardHistoryService};
use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::event::Event;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
    OpenClipboard, RegisterClipboardFormatW, RemoveClipboardFormatListener, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG, PostMessageW, PostQuitMessage,
    RegisterClassExW, SMTO_ABORTIFHUNG, SendMessageTimeoutW, TranslateMessage, UnregisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_DESTROY, WNDCLASSEXW,
};
use windows::core::{PCWSTR, w};

const CLASS_NAME: PCWSTR = w!("KoyClipboardListener");
const WM_CLIPBOARDUPDATE: u32 = 0x031D;
const WM_COPY_BACK: u32 = 0x8000 + 1; // WM_APP + 1
const CF_UNICODETEXT: u32 = 13;
const OPEN_RETRIES: i32 = 8;

struct Inner {
    history: ClipboardHistory,
    store: ClipboardPinnedStore,
    fmt_exclude: u32,
    fmt_can_include: u32,
    fmt_viewer_ignore: u32,
    pending_copy: Mutex<Option<Guid>>,
    // Kendi yazdığımız değişikliğin pano sıra numarası aralığı: bu aralıktaki güncellemeler geçmişe eklenmez.
    own_from: AtomicI64,
    own_to: AtomicI64,
    changed: Event<()>,
}

struct Runtime {
    thread: Option<JoinHandle<()>>,
    hwnd: isize,
    pinned_loaded: bool,
    disposed: bool,
}

pub struct ClipboardHistoryService {
    inner: Arc<Inner>,
    runtime: Mutex<Runtime>,
}

thread_local! {
    // Pencere yordamı bağlam taşımaz: dinleyici iş parçacığı kendi servisini burada tutar.
    static CURRENT: RefCell<Option<Arc<Inner>>> = const { RefCell::new(None) };
}

fn register(name: PCWSTR) -> u32 {
    // SAFETY: sabit, sıfırla biten ad.
    unsafe { RegisterClipboardFormatW(name) }
}

fn try_open(hwnd: HWND) -> bool {
    // Pano başka uygulamada kilitliyse kısa aralıklarla birkaç kez dener (dinleyici iş parçacığında, arayüzü tutmaz).
    for _ in 0..OPEN_RETRIES {
        // SAFETY: pencere bizim (ya da geçersizse çağrı başarısız olur).
        if unsafe { OpenClipboard(Some(hwnd)) }.is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

impl Inner {
    fn available(&self, format: u32) -> bool {
        // SAFETY: pano açıkken çağrılır.
        unsafe { IsClipboardFormatAvailable(format) }.is_ok()
    }

    /// Metni okur; metin yoksa ya da parola yöneticisi dışlama işareti varsa `None`.
    fn read_text(&self, hwnd: HWND) -> Option<String> {
        if !try_open(hwnd) {
            return None;
        }
        // SAFETY: pano açık; genel bellek kilitlenip boyutu kadar okunur ve bırakılır.
        let result = unsafe {
            (|| {
                if !self.available(CF_UNICODETEXT) {
                    return None;
                }
                // Parola yöneticileri bu biçimlerle "izleme/geçmiş dışı" der.
                if self.available(self.fmt_exclude) || self.available(self.fmt_viewer_ignore) {
                    return None;
                }
                if self.available(self.fmt_can_include) && read_dword(self.fmt_can_include) == Some(0) {
                    return None;
                }
                let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
                let global = HGLOBAL(handle.0);
                let size = GlobalSize(global);
                let ptr = GlobalLock(global) as *const u16;
                if ptr.is_null() {
                    return None;
                }
                let max = size / 2;
                let slice = std::slice::from_raw_parts(ptr, max);
                let len = slice.iter().position(|&c| c == 0).unwrap_or(max);
                let text = String::from_utf16_lossy(&slice[..len]);
                let _ = GlobalUnlock(global);
                Some(text)
            })()
        };
        // SAFETY: açtığımız pano kapatılır.
        let _ = unsafe { CloseClipboard() };
        result
    }

    fn on_clipboard_update(&self, hwnd: HWND) {
        // SAFETY: yalnız sıra numarası sorgusu.
        let seq = unsafe { GetClipboardSequenceNumber() } as i64;
        let (from, to) = (self.own_from.load(Ordering::SeqCst), self.own_to.load(Ordering::SeqCst));
        if from >= 0 && seq >= from && seq <= to {
            return; // kendi yazdığımız değişiklik
        }
        let Some(text) = self.read_text(hwnd) else { return };
        if self.history.add(Some(&text)).is_some() {
            self.changed.invoke(&());
        }
    }

    fn write_back(&self, hwnd: HWND) -> bool {
        let Some(id) = *self.pending_copy.lock().unwrap_or_else(|e| e.into_inner()) else { return false };
        let Some(item) = self.history.get(id) else { return false };
        // SAFETY: yalnız sıra numarası sorgusu.
        let before = unsafe { GetClipboardSequenceNumber() } as i64;
        if !try_open(hwnd) {
            return false;
        }
        let units: Vec<u16> = item.text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: pano açık; bellek ayrılır, doldurulur ve panoya verilir (başarısızsa serbest bırakılır).
        let success = unsafe {
            (|| {
                let _ = EmptyClipboard();
                let mem = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2).ok()?;
                let ptr = GlobalLock(mem) as *mut u16;
                if ptr.is_null() {
                    let _ = GlobalFree(Some(mem));
                    return None;
                }
                std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
                let _ = GlobalUnlock(mem);
                // Başarılı olursa bellek panoya geçer.
                if SetClipboardData(CF_UNICODETEXT, Some(HANDLE(mem.0))).is_err() {
                    let _ = GlobalFree(Some(mem));
                    return None;
                }
                Some(())
            })()
            .is_some()
        };
        // SAFETY: açtığımız pano kapatılır.
        let _ = unsafe { CloseClipboard() };
        if success {
            // Aralık: yazmadan önceki numaranın sonrasından kapattıktan sonraki numaraya kadar.
            self.own_from.store(before + 1, Ordering::SeqCst);
            self.own_to.store(unsafe { GetClipboardSequenceNumber() } as i64, Ordering::SeqCst);
        }
        success
    }
}

unsafe fn read_dword(format: u32) -> Option<u32> {
    // SAFETY: pano açıkken çağrılır; boyut denetlenir.
    unsafe {
        let handle = GetClipboardData(format).ok()?;
        let global = HGLOBAL(handle.0);
        if GlobalSize(global) < 4 {
            return None;
        }
        let ptr = GlobalLock(global) as *const u32;
        if ptr.is_null() {
            return None;
        }
        let v = std::ptr::read_unaligned(ptr);
        let _ = GlobalUnlock(global);
        Some(v)
    }
}

fn log(source: &str, message: &str) {
    CrashLog::write(source, &CrashError::exception("System.Exception", message, ""));
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let inner = CURRENT.with(|c| c.borrow().clone());
    let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Option<LRESULT> {
        let inner = inner.as_ref()?;
        match msg {
            WM_CLIPBOARDUPDATE => {
                inner.on_clipboard_update(hwnd);
                Some(LRESULT(0))
            }
            WM_COPY_BACK => Some(LRESULT(if inner.write_back(hwnd) { 1 } else { 0 })),
            WM_CLOSE => {
                // SAFETY: kendi penceremiz.
                unsafe {
                    let _ = RemoveClipboardFormatListener(hwnd);
                    let _ = DestroyWindow(hwnd);
                }
                Some(LRESULT(0))
            }
            WM_DESTROY => {
                // SAFETY: bu iş parçacığının mesaj döngüsünü bitirir.
                unsafe { PostQuitMessage(0) };
                Some(LRESULT(0))
            }
            _ => None,
        }
    }));
    match handled {
        Ok(Some(r)) => r,
        Ok(None) => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        Err(_) => {
            log("ClipboardHistoryService.WndProc", "panic");
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

fn run(inner: Arc<Inner>, ready: mpsc::Sender<isize>) {
    CURRENT.with(|c| *c.borrow_mut() = Some(inner));
    // SAFETY: sınıf kaydı, mesaj penceresi ve mesaj döngüsü bu iş parçacığında; çıkışta sınıf kaydı silinir.
    unsafe {
        let Ok(module) = GetModuleHandleW(PCWSTR::null()) else {
            let _ = ready.send(0);
            return;
        };
        let instance = module.into();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        // HWND_MESSAGE: yalnız ileti alan, görünmez, Z-sırasında olmayan pencere.
        let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), CLASS_NAME, PCWSTR::null(), WINDOW_STYLE(0), 0, 0, 0, 0, Some(HWND_MESSAGE), None, Some(instance), None);
        let Ok(hwnd) = hwnd else {
            let _ = ready.send(0);
            return;
        };
        if AddClipboardFormatListener(hwnd).is_err() {
            let _ = DestroyWindow(hwnd);
            let _ = ready.send(0);
            return;
        }
        let _ = ready.send(hwnd.0 as isize);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnregisterClassW(CLASS_NAME, Some(instance));
    }
    CURRENT.with(|c| *c.borrow_mut() = None);
}

impl ClipboardHistoryService {
    pub fn new(store: Option<ClipboardPinnedStore>) -> Self {
        ClipboardHistoryService {
            inner: Arc::new(Inner {
                history: ClipboardHistory::new(None),
                store: store.unwrap_or_else(|| ClipboardPinnedStore::new(None, None)),
                fmt_exclude: register(w!("ExcludeClipboardContentFromMonitorProcessing")),
                fmt_can_include: register(w!("CanIncludeInClipboardHistory")),
                fmt_viewer_ignore: register(w!("Clipboard Viewer Ignore")),
                pending_copy: Mutex::new(None),
                own_from: AtomicI64::new(-1),
                own_to: AtomicI64::new(-1),
                changed: Event::new(),
            }),
            runtime: Mutex::new(Runtime { thread: None, hwnd: 0, pinned_loaded: false, disposed: false }),
        }
    }

    fn rt(&self) -> std::sync::MutexGuard<'_, Runtime> {
        self.runtime.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn dispose(&self) {
        {
            let mut rt = self.rt();
            if rt.disposed {
                return;
            }
            rt.disposed = true;
        }
        self.stop();
    }
}

impl Drop for ClipboardHistoryService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IClipboardHistoryService for ClipboardHistoryService {
    fn is_running(&self) -> bool {
        let rt = self.rt();
        rt.thread.as_ref().is_some_and(|t| !t.is_finished()) && rt.hwnd != 0
    }

    fn start(&self) {
        let previous = {
            let mut rt = self.rt();
            if rt.disposed || (rt.thread.as_ref().is_some_and(|t| !t.is_finished()) && rt.hwnd != 0) {
                return;
            }
            rt.thread.take()
        };
        // Önceki pencere hâlâ kapanıyorsa kısa bekle (sınıf adı çakışmasın).
        if let Some(t) = previous {
            let deadline = std::time::Instant::now() + Duration::from_secs(1);
            while !t.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        {
            let mut rt = self.rt();
            if !rt.pinned_loaded {
                self.inner.history.load_pinned(self.inner.store.load());
                rt.pinned_loaded = true;
            }
        }
        let (tx, rx) = mpsc::channel();
        let inner = Arc::clone(&self.inner);
        let thread = std::thread::Builder::new().name("Koy pano".into()).spawn(move || run(inner, tx)).ok();
        let hwnd = rx.recv_timeout(Duration::from_secs(2)).unwrap_or(0);
        let mut rt = self.rt();
        rt.thread = thread;
        rt.hwnd = hwnd;
    }

    fn stop(&self) {
        let hwnd = std::mem::take(&mut self.rt().hwnd);
        // Arayüz iş parçacığını bekletme: pencere kendi iş parçacığında kapanır.
        if hwnd != 0 {
            // SAFETY: yalnız mesaj gönderilir.
            let _ = unsafe { PostMessageW(Some(HWND(hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
    }

    fn changed(&self) -> &Event<()> {
        &self.inner.changed
    }

    fn query(&self, filter: ClipboardFilter, search: Option<&str>) -> Vec<ClipboardItem> {
        self.inner.history.query(filter, search)
    }

    fn set_pinned(&self, id: Guid, pinned: bool) -> bool {
        if !self.inner.history.set_pinned(id, pinned) {
            return false;
        }
        self.inner.store.save(&self.inner.history.pinned_snapshot());
        self.inner.changed.invoke(&());
        true
    }

    fn remove(&self, id: Guid) -> bool {
        let was_pinned = self.inner.history.get(id).is_some_and(|i| i.pinned);
        if !self.inner.history.remove(id) {
            return false;
        }
        if was_pinned {
            self.inner.store.save(&self.inner.history.pinned_snapshot());
        }
        self.inner.changed.invoke(&());
        true
    }

    fn clear(&self) {
        if self.inner.history.clear() > 0 {
            self.inner.changed.invoke(&());
        }
    }

    fn copy_back(&self, id: Guid) -> bool {
        let hwnd = self.rt().hwnd;
        *self.inner.pending_copy.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
        // Pano yazımı bir pencere sahibi ister; bu yüzden iş dinleyici penceresinin iş parçacığında yapılır.
        if hwnd == 0 {
            return false;
        }
        let mut result = 0usize;
        // SAFETY: zaman aşımlı eşzamanlı mesaj.
        let ok = unsafe {
            SendMessageTimeoutW(HWND(hwnd as *mut _), WM_COPY_BACK, WPARAM(0), LPARAM(0), SMTO_ABORTIFHUNG, 2000, Some(&mut result))
        };
        ok.0 != 0 && result == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_and_stops_listener() {
        let dir = std::env::temp_dir().join(format!("koy-clip-test-{}", std::process::id()));
        let store = ClipboardPinnedStore::new(Some(dir.join("pinned.json")), None);
        let service = ClipboardHistoryService::new(Some(store));
        service.start();
        assert!(service.is_running());
        service.stop();
        std::thread::sleep(Duration::from_millis(200));
        assert!(!service.is_running());
        let _ = std::fs::remove_dir_all(dir);
    }
}

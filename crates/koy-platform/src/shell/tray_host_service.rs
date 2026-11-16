//! Koy'un kendi sistem tepsisi. Dock Windows görev çubuğunu gizlediği için tepsi simgelerine başka yoldan ulaşılamıyor.
//!
//! Shell_NotifyIcon, simge kaydını "Shell_TrayWnd" sınıflı ilk pencereye WM_COPYDATA ile yollar. Koy bu sınıfla gizli,
//! en üstte bir pencere açar; böylece kayıtlar önce Koy'a gelir. Her mesaj ayrıca Windows görev çubuğuna iletilir: Koy
//! kapanır ya da çökerse pencere süreçle birlikte gider ve Windows tepsisi hiçbir şey kaybetmemiş olur. Görev çubuğuna
//! gelen diğer mesajlar (AppBar, WM_COMMAND, WM_USER+) olduğu gibi Windows'a aktarılır (RetroBar / ManagedShell yöntemi).
//!
//! Pencere kendi iş parçacığında çalışır: arayüz meşgulken uygulamaların Shell_NotifyIcon çağrıları beklemesin. Süreç
//! nasıl ölürse ölsün sistemde iz kalmaz; temiz kapanışta pencere yok edilir ve bir kez daha TaskbarCreated yayınlanır.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::NaiveDateTime;
use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::event::Event;
use koy_core::tray::{
    ITrayService, TaskbarCreatedPolicy, TrayBalloonFilter, TrayBalloonFlags, TrayBalloonNotification, TrayCallback, TrayHostPolicy, TrayIcon,
    TrayIconFields, TrayIconKey, TrayIconStore, TrayMessageParser, TrayMouseAction, TrayNotifyData,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CopyIcon, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DispatchMessageW, FindWindowW, GetMessageW,
    GetWindowRect, GetWindowThreadProcessId, HICON, HWND_TOPMOST, IsWindow, KillTimer, MSG, PostMessageW, PostQuitMessage, RegisterClassExW,
    RegisterWindowMessageW, SMTO_ABORTIFHUNG, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SendMessageTimeoutW, SendNotifyMessageW, SetTimer,
    SetWindowPos, TranslateMessage, UnregisterClassW, WINDOW_EX_STYLE, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{HSTRING, PCWSTR, w};

use super::icon_helper::IconHelper;
use super::shell_tray_locator::{TRAY_CLASS_NAME, find_explorer_tray};
use super::window_styles::{get_process_path, hwnd};

const NOTIFY_CLASS_NAME: PCWSTR = w!("TrayNotifyWnd");
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_ACTIVATEAPP: u32 = 0x001C;
const WM_COPYDATA: u32 = 0x004A;
const WM_COMMAND: u32 = 0x0111;
const WM_TIMER: u32 = 0x0113;
const WM_USER: u32 = 0x0400;
const WM_RESYNC: u32 = 0x8000 + 0x4B; // WM_APP: başka bir iş parçacığından yeniden öne geçme isteği

const RESYNC_TIMER_ID: usize = 1;
const CATCH_UP_TIMER_ID: usize = 2;
const RESYNC_DELAY_MS: u32 = 1500;
const CATCH_UP_DELAY_MS: u32 = 2000;
const FORWARD_TIMEOUT_MS: u32 = 2000;
const STOP_WAIT: Duration = Duration::from_millis(250);
// Yayın bütün uygulamalara simgelerini yeniden ekletir; Gezgin'le sıra kavgasında art arda yayın yapılmasın.
const MIN_RESYNC_INTERVAL: Duration = Duration::from_secs(5);

// Stop beklemeden döner; kapanmakta olan iş parçacığı burada tutulur ki sıradaki Start sınıf adı boşalana dek beklesin.
static CLOSING_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
// Son kendi yayınımız (UTC). Statik: Koy'un öteki pencereleri yankıyı Gezgin yeniden başladı sanmasın.
static LAST_BROADCAST: Mutex<Option<NaiveDateTime>> = Mutex::new(None);
// Yayın anında Gezgin'in Shell_TrayWnd tutamacı: Gezgin gerçekten yeniden başladıysa tutamaç değişir.
static BROADCAST_EXPLORER_TRAY: AtomicIsize = AtomicIsize::new(0);

fn utc_now() -> NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

fn last_broadcast_utc() -> NaiveDateTime {
    LAST_BROADCAST.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or(NaiveDateTime::MIN)
}

fn explorer_tray() -> isize {
    find_explorer_tray(0).map_or(0, |h| h.0 as isize)
}

fn destroy_icons(handles: &[isize]) {
    for &h in handles {
        // SAFETY: kopyaladığımız simgeler bize ait.
        let _ = unsafe { DestroyIcon(HICON(h as *mut _)) };
    }
}

fn copy_icon(h: isize) -> isize {
    // SAFETY: kaynak simge geçersizse çağrı başarısız olur.
    unsafe { CopyIcon(HICON(h as *mut _)) }.map_or(0, |c| c.0 as isize)
}

fn window_pid(h: isize) -> u32 {
    let mut pid = 0u32;
    // SAFETY: çıktı yerel.
    unsafe { GetWindowThreadProcessId(hwnd(h), Some(&mut pid)) };
    pid
}

fn explorer_tray_bounds() -> RECT {
    let mut r = RECT::default();
    if let Some(e) = find_explorer_tray(0) {
        // SAFETY: çıktı yerel.
        if unsafe { GetWindowRect(e, &mut r) }.is_err() {
            r = RECT::default();
        }
    }
    r
}

/// Dosyanın sürüm bilgisindeki FileDescription (C# `FileVersionInfo.FileDescription`).
fn file_description(path: &str) -> Option<String> {
    let path_h = HSTRING::from(path);
    // SAFETY: sürüm bloğu boyutu kadar arabellek; VerQueryValue dönen göstergeler bu arabelleğin içini gösterir.
    unsafe {
        let size = GetFileVersionInfoSizeW(&path_h, None);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        GetFileVersionInfoW(&path_h, None, size, block.as_mut_ptr() as *mut _).ok()?;
        let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let mut candidates: Vec<String> = Vec::new();
        if VerQueryValueW(block.as_ptr() as *const _, w!("\\VarFileInfo\\Translation"), &mut ptr, &mut len).as_bool() && len >= 4 {
            let pairs = std::slice::from_raw_parts(ptr as *const u16, len as usize / 2);
            for p in pairs.chunks_exact(2) {
                candidates.push(format!("{:04x}{:04x}", p[0], p[1]));
            }
        }
        // FileVersionInfo'nun yedekleri: İngilizce (ABD) Unicode ve kod sayfası 1252.
        candidates.extend(["040904b0".to_string(), "040904e4".to_string()]);
        for lang in candidates {
            let key = HSTRING::from(format!("\\StringFileInfo\\{lang}\\FileDescription"));
            let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut len = 0u32;
            if VerQueryValueW(block.as_ptr() as *const _, &key, &mut ptr, &mut len).as_bool() && len > 0 {
                let s = std::slice::from_raw_parts(ptr as *const u16, len as usize);
                let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                return Some(String::from_utf16_lossy(&s[..end]));
            }
        }
        None
    }
}

struct Data {
    store: TrayIconStore,
    balloon_filter: TrayBalloonFilter,
}

struct Inner {
    data: Mutex<Data>,
    app_names: Mutex<(HashMap<u32, String>, HashSet<u32>)>,
    own_process_id: u32,
    // Pencere tanıtıcısını tepsi iş parçacığı yazar, arayüz iş parçacığı okur.
    hwnd: AtomicIsize,
    taskbar_created_message: AtomicU32,
    close_requested: AtomicBool,
    disposed: AtomicBool,
    icons_changed: Event<()>,
    balloon_received: Event<TrayBalloonNotification>,
    exited: Event<()>,
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<Inner>>> = const { RefCell::new(None) };
}

impl Inner {
    fn hwnd(&self) -> isize {
        self.hwnd.load(Ordering::SeqCst)
    }

    fn data(&self) -> std::sync::MutexGuard<'_, Data> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn find(data: &Data, key: &TrayIconKey) -> Option<TrayIcon> {
        data.store.all().iter().find(|i| &i.key == key).cloned()
    }

    fn get_app_name(self: &Arc<Self>, owner_hwnd: isize) -> String {
        let pid = window_pid(owner_hwnd);
        if pid == 0 {
            return String::new();
        }
        {
            let mut names = self.app_names.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(cached) = names.0.get(&pid) {
                return cached.clone();
            }
            // Dosya sürüm bilgisi diskten okunur: arka planda çözülür; çözülünce icons_changed ile pencere yenilenir.
            if !names.1.insert(pid) {
                return String::new();
            }
        }
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            if let Some(inner) = weak.upgrade() {
                inner.resolve_app_name(pid);
            }
        });
        String::new()
    }

    fn resolve_app_name(&self, pid: u32) {
        // Erişilemeyen süreçte ipucu boş kalır.
        let name = get_process_path(pid)
            .filter(|p| !p.is_empty())
            .map(|path| {
                file_description(&path)
                    .filter(|d| !d.trim().is_empty())
                    .map(|d| d.trim().to_string())
                    .unwrap_or_else(|| koy_core::win_path::get_file_name_without_extension(&path).to_string())
            })
            .unwrap_or_default();
        {
            let mut names = self.app_names.lock().unwrap_or_else(|e| e.into_inner());
            names.1.remove(&pid);
            if self.disposed.load(Ordering::SeqCst) {
                return;
            }
            names.0.insert(pid, name.clone());
        }
        if !name.is_empty() {
            self.icons_changed.invoke(&());
        }
    }

    fn handle_tray_message(self: &Arc<Self>, mut data: TrayNotifyData) -> bool {
        // Koy'un kendi simgesi (dock kapalıyken menüye erişim için) dock'un tepsisinde gösterilmez.
        if data.owner_hwnd != 0 && TrayHostPolicy::is_own_icon(window_pid(data.owner_hwnd), self.own_process_id) {
            return true;
        }
        // Uygulama simgesini sonradan yok edebilir; kendi kopyamızı tutarız.
        if data.fields.has(TrayIconFields::ICON) && data.icon_handle != 0 {
            data.icon_handle = copy_icon(data.icon_handle);
        }

        let mut pending: Option<TrayBalloonNotification> = None;
        let mut balloon_icon_copy = 0isize;
        let mut balloon_owner = 0isize;
        let mut balloon_tooltip = String::new();
        let success = {
            let mut d = self.data();
            let result = d.store.apply(&data);
            destroy_icons(&result.released_icons);
            if result.success
                && let Some((title, body, timestamp)) = d.balloon_filter.try_accept(&data)
            {
                let key = TrayIconKey::from_data(&data);
                let icon = Self::find(&d, &key);
                balloon_owner = icon.as_ref().map_or(data.owner_hwnd, |i| i.owner_hwnd);
                balloon_tooltip = match icon.as_ref().filter(|i| !i.tooltip.trim().is_empty()) {
                    Some(i) => i.tooltip.trim().to_string(),
                    None => data.tooltip.trim().to_string(),
                };
                let user_icon = TrayBalloonFlags(data.info_flags.0 & TrayBalloonFlags::ICON_MASK.0) == TrayBalloonFlags::USER && data.balloon_icon_handle != 0;
                let source = if user_icon { data.balloon_icon_handle } else { icon.as_ref().map_or(0, |i| i.icon_handle) };
                if source != 0 {
                    balloon_icon_copy = copy_icon(source);
                }
                pending = Some(TrayBalloonNotification { key, app_name: String::new(), title, body, flags: data.info_flags, time: timestamp, icon: None });
            }
            result.success
        };

        if success {
            self.icons_changed.invoke(&());
        }
        if let Some(mut balloon) = pending {
            if balloon_icon_copy != 0 {
                balloon.icon = IconHelper::hicon_to_png(balloon_icon_copy);
                destroy_icons(&[balloon_icon_copy]);
            }
            balloon.app_name = if balloon_tooltip.is_empty() { self.get_app_name(balloon_owner) } else { balloon_tooltip };
            self.balloon_received.invoke(&balloon);
        }
        success
    }

    /// Öne geçer ve uygulamalardan simgelerini yeniden ister; son yayından kısa süre geçtiyse ertelenir.
    fn resync(&self) {
        self.make_topmost();
        let since_last = (utc_now() - last_broadcast_utc()).to_std().unwrap_or(Duration::MAX);
        if since_last >= MIN_RESYNC_INTERVAL {
            self.broadcast_taskbar_created();
            return;
        }
        let wait = MIN_RESYNC_INTERVAL - since_last;
        // SAFETY: kendi penceremizde zamanlayıcı.
        unsafe { SetTimer(Some(hwnd(self.hwnd())), RESYNC_TIMER_ID, wait.as_micros().div_ceil(1000) as u32 + 50, None) };
    }

    fn make_topmost(&self) {
        let h = self.hwnd();
        if h == 0 {
            return;
        }
        // Görev çubuğu yer değiştirmiş olabilir (ekran değişimi); konum da eşitlenir. Pencere gösterilmez.
        let b = explorer_tray_bounds();
        let mut flags = SWP_NOACTIVATE;
        if b.right <= b.left || b.bottom <= b.top {
            flags |= SWP_NOMOVE | SWP_NOSIZE;
        }
        // SAFETY: kendi penceremiz.
        let _ = unsafe { SetWindowPos(hwnd(h), Some(HWND_TOPMOST), b.left, b.top, b.right - b.left, b.bottom - b.top, flags) };
    }

    /// Uygulamalardan tepsi simgelerini yeniden eklemelerini ister (Gezgin yeniden başlamış gibi).
    fn broadcast_taskbar_created(&self) {
        let msg = self.taskbar_created_message.load(Ordering::SeqCst);
        if msg == 0 {
            return;
        }
        BROADCAST_EXPLORER_TRAY.store(explorer_tray(), Ordering::SeqCst);
        *LAST_BROADCAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(utc_now());
        // SAFETY: HWND_BROADCAST'a bildirim (beklemez).
        let _ = unsafe { SendNotifyMessageW(HWND(0xFFFF as *mut _), msg, WPARAM(0), LPARAM(0)) };
    }
}

fn forward(msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(explorer) = find_explorer_tray(0) else { return LRESULT(0) };
    let mut result = 0usize;
    // SAFETY: zaman aşımlı eşzamanlı ileti; lparam göstergesi (WM_COPYDATA) çağrı süresince geçerli.
    unsafe { SendMessageTimeoutW(explorer, msg, wparam, lparam, SMTO_ABORTIFHUNG, FORWARD_TIMEOUT_MS, Some(&mut result)) };
    LRESULT(result as isize)
}

fn on_copy_data(inner: &Arc<Inner>, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let cds = lparam.0 as *const COPYDATASTRUCT;
    // Pencereye herhangi bir süreç WM_COPYDATA gönderebilir: boyut Core'daki sınırlarla denetlenir, bozuk ya da devasa
    // blok dilim kurulmadan reddedilir.
    // SAFETY: WM_COPYDATA için lparam bir COPYDATASTRUCT gösterir; veri bloğu cbData bayt ve çağrı süresince geçerli.
    let parsed = unsafe {
        if cds.is_null() {
            None
        } else {
            let c = &*cds;
            if c.dwData == TrayMessageParser::COPY_DATA_TRAY_MESSAGE as usize
                && !c.lpData.is_null()
                && TrayMessageParser::is_acceptable_length(c.cbData as i64)
            {
                TrayMessageParser::try_parse(std::slice::from_raw_parts(c.lpData as *const u8, c.cbData as usize))
            } else {
                None
            }
        }
    };
    match parsed {
        Some(data) => {
            let handled = inner.handle_tray_message(data);
            let forwarded = forward(msg, wparam, lparam);
            // Windows'un tepsisi simgeyi zaten biliyorsa hata döner; uygulamaya başarı bildirilir.
            LRESULT(if handled || forwarded.0 != 0 { 1 } else { 0 })
        }
        None => forward(msg, wparam, lparam),
    }
}

unsafe extern "system" fn tray_wnd_proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let inner = CURRENT.with(|c| c.borrow().clone());
    let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Option<LRESULT> {
        let inner = inner.as_ref()?;
        // SAFETY: kendi penceremizde zamanlayıcı ve yok etme çağrıları.
        unsafe {
            match msg {
                WM_COPYDATA => return Some(on_copy_data(inner, msg, wparam, lparam)),
                WM_TIMER if wparam.0 == RESYNC_TIMER_ID || wparam.0 == CATCH_UP_TIMER_ID => {
                    let _ = KillTimer(Some(h), wparam.0);
                    inner.resync();
                    return Some(LRESULT(0));
                }
                WM_RESYNC => {
                    inner.resync();
                    return Some(LRESULT(0));
                }
                WM_CLOSE => {
                    let _ = DestroyWindow(h);
                    return Some(LRESULT(0));
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    return Some(LRESULT(0));
                }
                _ => {}
            }
            let tc = inner.taskbar_created_message.load(Ordering::SeqCst);
            if msg == tc && msg != 0 {
                // Bizim yayınımız değilse Gezgin yeniden başlamıştır: kısa süre sonra öne geçip simgeleri yeniden iste.
                if !TrayHostService::is_own_taskbar_created_broadcast() {
                    SetTimer(Some(h), RESYNC_TIMER_ID, RESYNC_DELAY_MS, None);
                }
                return Some(LRESULT(0));
            }
        }
        if msg == WM_COMMAND || msg == WM_ACTIVATEAPP || msg >= WM_USER {
            return Some(forward(msg, wparam, lparam));
        }
        None
    }));
    match handled {
        Ok(Some(r)) => r,
        Ok(None) => unsafe { DefWindowProcW(h, msg, wparam, lparam) },
        Err(_) => {
            // Yerel geri çağrıdan panik sızarsa süreç çöker; tepsi mesajı kaybolsun, Koy ayakta kalsın.
            CrashLog::write("TrayHostService.TrayWndProc", &CrashError::exception("Panic", "panic", ""));
            unsafe { DefWindowProcW(h, msg, wparam, lparam) }
        }
    }
}

unsafe extern "system" fn notify_wnd_proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: varsayılan işleyiciye iletilir.
    unsafe { DefWindowProcW(h, msg, wparam, lparam) }
}

fn create_windows(inner: &Inner, instance: windows::Win32::Foundation::HINSTANCE) -> bool {
    // SAFETY: sınıf kaydı ve pencere oluşturma bu iş parçacığında; başarısızlıkta sınıflar silinir.
    unsafe {
        inner.taskbar_created_message.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::SeqCst);
        let tray_wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(tray_wnd_proc),
            hInstance: instance,
            lpszClassName: TRAY_CLASS_NAME,
            ..Default::default()
        };
        if RegisterClassExW(&tray_wc) == 0 {
            return false;
        }
        let notify_wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(notify_wnd_proc),
            hInstance: instance,
            lpszClassName: NOTIFY_CLASS_NAME,
            ..Default::default()
        };
        RegisterClassExW(&notify_wc);

        // Pencere hiç gösterilmez ama Gezgin'in görev çubuğuyla aynı dikdörtgende durur: bazı uygulamalar bildirimlerini
        // "Shell_TrayWnd"nin konumuna göre yerleştiriyor.
        let b = explorer_tray_bounds();
        let created = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            TRAY_CLASS_NAME,
            PCWSTR::null(),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            b.left,
            b.top,
            b.right - b.left,
            b.bottom - b.top,
            None,
            None,
            Some(instance),
            None,
        );
        let Ok(h) = created else {
            let _ = UnregisterClassW(TRAY_CLASS_NAME, Some(instance));
            let _ = UnregisterClassW(NOTIFY_CLASS_NAME, Some(instance));
            return false;
        };
        // Bazı uygulamalar simge konumunu bulmak için görev çubuğunun altında bu alt pencereyi arar.
        let _ = CreateWindowExW(WINDOW_EX_STYLE(0), NOTIFY_CLASS_NAME, PCWSTR::null(), WS_CHILD, 0, 0, 0, 0, Some(h), None, Some(instance), None);
        inner.hwnd.store(h.0 as isize, Ordering::SeqCst);
    }
    inner.make_topmost();
    true
}

fn run(inner: Arc<Inner>, ready: mpsc::Sender<()>) {
    CURRENT.with(|c| *c.borrow_mut() = Some(Arc::clone(&inner)));
    // SAFETY: null ad çağıran sürecin modülünü verir.
    let instance: windows::Win32::Foundation::HINSTANCE = match unsafe { GetModuleHandleW(PCWSTR::null()) } {
        Ok(m) => m.into(),
        Err(_) => {
            let _ = ready.send(());
            return;
        }
    };
    let ok = create_windows(&inner, instance);
    let _ = ready.send(());
    if !ok {
        CURRENT.with(|c| *c.borrow_mut() = None);
        return;
    }

    inner.broadcast_taskbar_created();
    // Gezgin durulunca bir kez daha öne geçip yayın yapılır; kaçan kayıtlar böylece gelir.
    // SAFETY: kendi penceremizde zamanlayıcı.
    unsafe { SetTimer(Some(hwnd(inner.hwnd())), CATCH_UP_TIMER_ID, CATCH_UP_DELAY_MS, None) };

    let mut msg = MSG::default();
    let got = loop {
        // SAFETY: mesaj döngüsü.
        let got = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if got <= 0 {
            break got;
        }
        // SAFETY: GetMessage'ın doldurduğu ileti.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    };
    if got < 0 {
        CrashLog::write("TrayHostService.Run (GetMessage hata döndü)", &CrashError::exception("System.InvalidOperationException", &format!("GetMessage={got}"), ""));
    }

    let unexpected = !inner.close_requested.load(Ordering::SeqCst);
    inner.hwnd.store(0, Ordering::SeqCst);
    {
        let mut d = inner.data();
        let released = d.store.clear();
        destroy_icons(&released);
        d.balloon_filter.clear();
    }
    // SAFETY: kendi sınıflarımız.
    unsafe {
        let _ = UnregisterClassW(TRAY_CLASS_NAME, Some(instance));
        let _ = UnregisterClassW(NOTIFY_CLASS_NAME, Some(instance));
    }
    inner.icons_changed.invoke(&());
    // Koy artık Shell_TrayWnd sahibi değil: uygulamalar simgelerini Windows'un tepsisine yeniden eklesin.
    inner.broadcast_taskbar_created();
    if unexpected {
        CrashLog::write(
            "TrayHostService.Run (pencere beklenmedik kapandı)",
            &CrashError::exception("System.InvalidOperationException", "Tepsi penceresi dışarıdan kapatıldı ya da mesaj döngüsü bitti.", ""),
        );
        inner.exited.invoke(&());
    }
    CURRENT.with(|c| *c.borrow_mut() = None);
}

pub struct TrayHostService {
    inner: Arc<Inner>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Default for TrayHostService {
    fn default() -> Self {
        TrayHostService {
            inner: Arc::new(Inner {
                data: Mutex::new(Data { store: TrayIconStore::new(), balloon_filter: TrayBalloonFilter::new(None) }),
                app_names: Mutex::new((HashMap::new(), HashSet::new())),
                own_process_id: std::process::id(),
                hwnd: AtomicIsize::new(0),
                taskbar_created_message: AtomicU32::new(0),
                close_requested: AtomicBool::new(false),
                disposed: AtomicBool::new(false),
                icons_changed: Event::new(),
                balloon_received: Event::new(),
                exited: Event::new(),
            }),
            thread: Mutex::new(None),
        }
    }
}

fn wait_finished(t: &JoinHandle<()>, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !t.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
}

impl TrayHostService {
    /// Az önce gelen "TaskbarCreated" bizim kendi yayınımızın yankısı mı (Gezgin'in yeniden başlaması değil).
    pub fn is_own_taskbar_created_broadcast() -> bool {
        TaskbarCreatedPolicy::is_own_broadcast(last_broadcast_utc(), utc_now(), BROADCAST_EXPLORER_TRAY.load(Ordering::SeqCst), explorer_tray())
    }

    /// Pencere biz istemeden kapandı. Tepsi iş parçacığından gelir; stop / request_close / dispose ile kapanışta gelmez.
    pub fn exited(&self) -> &Event<()> {
        &self.inner.exited
    }

    /// Pencereyi beklemeden kapatma isteği yollar (çökme işleyicileri için: arayüz iş parçacığı bloklanmaz).
    pub fn request_close(&self) {
        self.inner.close_requested.store(true, Ordering::SeqCst);
        let h = self.inner.hwnd();
        if h != 0 {
            // SAFETY: yalnız ileti gönderir.
            let _ = unsafe { PostMessageW(Some(hwnd(h)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
    }

    /// Gezgin kendi görev çubuğunu öne alırsa yeni simge kayıtları Koy'a gelmez. Ön plan değişiminde çağrılır.
    pub fn ensure_first(&self) {
        let h = self.inner.hwnd();
        if h == 0 {
            return;
        }
        // SAFETY: yalnız sorgu ve ileti.
        unsafe {
            if FindWindowW(TRAY_CLASS_NAME, PCWSTR::null()).map_or(0, |f| f.0 as isize) != h {
                let _ = PostMessageW(Some(hwnd(h)), WM_RESYNC, WPARAM(0), LPARAM(0));
            }
        }
    }

    pub fn dispose(&self) {
        if self.inner.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        self.stop();
    }
}

impl Drop for TrayHostService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl ITrayService for TrayHostService {
    fn icons_changed(&self) -> &Event<()> {
        &self.inner.icons_changed
    }

    fn balloon_received(&self) -> &Event<TrayBalloonNotification> {
        &self.inner.balloon_received
    }

    fn get_visible_icons(&self) -> Vec<TrayIcon> {
        let mut icons = self.inner.data().store.visible();
        // Bazı uygulamalar ipucu göndermiyor; üzerine gelince ne olduğu anlaşılsın diye uygulama adı.
        for icon in &mut icons {
            if icon.tooltip.trim().is_empty() {
                icon.tooltip = self.inner.get_app_name(icon.owner_hwnd);
            }
        }
        icons
    }

    fn get_icon_image(&self, key: &TrayIconKey) -> Option<Vec<u8>> {
        // Kilit altında yalnız kopya alınır; GDI okuması ve PNG kodlaması kilit dışında.
        let copy = {
            let d = self.inner.data();
            let icon = Inner::find(&d, key).filter(|i| i.icon_handle != 0)?;
            copy_icon(icon.icon_handle)
        };
        if copy == 0 {
            return None;
        }
        let png = IconHelper::hicon_to_png(copy);
        destroy_icons(&[copy]);
        png
    }

    fn send_mouse(&self, key: &TrayIconKey, action: TrayMouseAction, cursor_x: i32, cursor_y: i32) {
        let Some(icon) = Inner::find(&self.inner.data(), key) else { return };
        let pid = window_pid(icon.owner_hwnd);
        // Son girdiyi Koy aldığı için öne geçme hakkını uygulamaya devredebilir; menüsü ve penceresi öne gelsin.
        if pid != 0 {
            // SAFETY: yalnız izin devri.
            let _ = unsafe { AllowSetForegroundWindow(pid) };
        }
        for m in TrayCallback::build(&icon, action, cursor_x, cursor_y) {
            // SAFETY: bildirim iletisi (beklemez).
            let _ = unsafe { SendNotifyMessageW(hwnd(icon.owner_hwnd), m.message, WPARAM(m.w_param as usize), LPARAM(m.l_param)) };
        }
    }

    fn remove_dead_icons(&self) {
        let changed = {
            let mut d = self.inner.data();
            // SAFETY: yalnız sorgu.
            let released = d.store.remove_dead(|h| unsafe { IsWindow(Some(hwnd(h))) }.as_bool());
            destroy_icons(&released);
            !released.is_empty()
        };
        if changed {
            // Süreç numaraları yeniden kullanılabilir; ölen uygulamanın adı yenisine yapışmasın.
            self.inner.app_names.lock().unwrap_or_else(|e| e.into_inner()).0.clear();
            self.inner.icons_changed.invoke(&());
        }
    }

    fn is_running(&self) -> bool {
        self.inner.hwnd() != 0
    }

    fn start(&self) {
        let mut slot = self.thread.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_some() || self.inner.disposed.load(Ordering::SeqCst) {
            return;
        }
        // Önceki örnek arka planda kapanıyor olabilir; "Shell_TrayWnd" sınıfı boşalmadan yenisi kaydolamaz.
        if let Some(prev) = CLOSING_THREAD.lock().unwrap_or_else(|e| e.into_inner()).take() {
            wait_finished(&prev, Duration::from_secs(2));
        }
        self.inner.close_requested.store(false, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        let inner = Arc::clone(&self.inner);
        *slot = std::thread::Builder::new().name("Koy tepsi".into()).spawn(move || run(inner, tx)).ok();
        let _ = rx.recv_timeout(Duration::from_secs(5));
    }

    fn stop(&self) {
        let Some(thread) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
        self.request_close();
        // Arayüz iş parçacığı burada kısa bekler; tepsi iş parçacığı gecikirse temizliği arka planda bitirir.
        wait_finished(&thread, STOP_WAIT);
        *CLOSING_THREAD.lock().unwrap_or_else(|e| e.into_inner()) = Some(thread);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_own_exe_description_or_none() {
        let exe = std::env::current_exe().unwrap();
        let _ = file_description(&exe.to_string_lossy());
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let d = file_description(&format!(r"{windir}\explorer.exe"));
        assert!(d.is_some_and(|d| !d.is_empty()));
    }
}

//! SetWinEventHook kullanarak ön plandaki pencereyi, konum değişikliklerini ve açık pencereleri izler.
//!
//! İplik: WINEVENT_OUTOFCONTEXT olayları `start`'ı çağıran iş parçacığına gelir; o iş parçacığının mesaj döngüsü olmalı
//! (C#'ta arayüz iş parçacığı). Geri çağrı bağlam taşımadığı için izleyici o iş parçacığının `thread_local` alanında
//! tutulur. Başlık değişimi bildirimi C#'ta `SynchronizationContext` ile Start iş parçacığına taşınır; burada kurucuya
//! verilen `post` ile (verilmezse zamanlayıcı iş parçacığında doğrudan).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::{Duration, Instant};

use koy_core::dock::{PostedWork, RefreshDebouncer};
use koy_core::event::Event;
use koy_core::search::PostFn;
use koy_core::shell::{IWindowTracker, OpenWindowInfo, ProcessTerminationPolicy, TaskbarWindowFacts, TaskbarWindowRule, WindowInfo, WindowRect};
use koy_core::time::TimeSpan;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, DeregisterShellHookWindow, EVENT_OBJECT_CLOAKED, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW, EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EnumChildWindows,
    EnumWindows, GA_ROOT, GW_OWNER, GWL_EXSTYLE, GetAncestor, GetDesktopWindow, GetForegroundWindow, GetShellWindow, GetWindow, GetWindowLongW,
    GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW, RegisterShellHookWindow,
    RegisterWindowMessageW, SW_MINIMIZE, SW_RESTORE, SetForegroundWindow, ShowWindow, WINEVENT_OUTOFCONTEXT, WM_CLOSE, WS_EX_APPWINDOW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, w};

use super::app_user_model_id_reader::AppUserModelIdReader;
use super::fullscreen_detector::class_name;
use super::icon_helper::IconHelper;
use super::small::CursorHelper;
use super::window_styles::{get_process_path, hwnd};

const NAME_CHANGE_DELAY_MS: i64 = 500;
const EMPTY_AUMID_RETRY: Duration = Duration::from_secs(10);

struct Hooks {
    handles: Vec<HWINEVENTHOOK>,
    debouncer: Option<RefreshDebouncer>,
    shell_hook_windows: Vec<isize>,
}

struct Inner {
    started: Mutex<bool>,
    hooks: Mutex<Hooks>,
    current_foreground: Mutex<Option<WindowInfo>>,
    // Son taramada dock listesine giren pencereler (başlık değişimi süzgeci için); her taramada yenisiyle değişir.
    known_windows: RwLock<HashSet<isize>>,
    // AUMID pencere ömrü boyunca değişmez sayılır: HWND başına bir kez okunur. Boş sonuç kısa süre tutulur.
    aumid_cache: Mutex<HashMap<isize, (Option<String>, Instant)>>,
    shell_hook_message: Mutex<u32>,
    post: Option<PostFn>,
    foreground_window_changed: Event<WindowInfo>,
    windows_changed: Event<()>,
    window_flashing: Event<isize>,
}

// SAFETY: HWINEVENTHOOK değerleri yalnız tanıtıcıdır ve kilit altında kullanılır.
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

thread_local! {
    static CURRENT: RefCell<Option<Weak<Inner>>> = const { RefCell::new(None) };
}

pub struct WindowTracker {
    inner: Arc<Inner>,
}

impl Default for WindowTracker {
    fn default() -> Self {
        Self::new(None)
    }
}

fn is_shell_or_desktop(h: HWND) -> bool {
    // SAFETY: yalnız sorgular.
    if h.is_invalid() || h == unsafe { GetDesktopWindow() } || h == unsafe { GetShellWindow() } {
        return true;
    }
    matches!(class_name(h).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
}

fn is_cloaked(h: HWND) -> bool {
    let mut cloaked = 0i32;
    // SAFETY: 4 baytlık çıktı.
    unsafe { DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut i32 as *mut _, 4) }.is_ok() && cloaked != 0
}

/// Pencerenin görev çubuğu olgularını okur (kural Core'daki TaskbarWindowRule'da).
fn read_taskbar_facts(h: HWND) -> TaskbarWindowFacts {
    // SAFETY: yalnız sorgular.
    let (ex, visible, owner) = unsafe {
        (GetWindowLongW(h, GWL_EXSTYLE) as u32, IsWindowVisible(h).as_bool(), GetWindow(h, GW_OWNER).map(|o| !o.is_invalid()).unwrap_or(false))
    };
    TaskbarWindowFacts {
        is_visible: visible,
        is_cloaked: visible && is_cloaked(h),
        is_shell_window: is_shell_or_desktop(h),
        has_owner: owner,
        is_app_window: ex & WS_EX_APPWINDOW.0 != 0,
        is_tool_window: ex & WS_EX_TOOLWINDOW.0 != 0,
        is_no_activate: ex & WS_EX_NOACTIVATE.0 != 0,
    }
}

fn is_root(h: HWND) -> bool {
    // SAFETY: yalnız sorgu.
    (unsafe { GetAncestor(h, GA_ROOT) }) == h
}

/// Üst düzey (kökü kendisi olan) ve görev çubuğuna girecek bir pencere mi.
fn is_top_level_taskbar_window(h: HWND) -> bool {
    is_root(h) && TaskbarWindowRule::should_show(&read_taskbar_facts(h))
}

fn window_pid(h: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: çıktı yerel.
    if (unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) }) == 0 { 0 } else { pid }
}

fn rect_of(h: HWND) -> Option<RECT> {
    let mut r = RECT::default();
    // SAFETY: çıktı yerel.
    unsafe { GetWindowRect(h, &mut r) }.ok().map(|_| r)
}

fn resolve_uwp_process_id(frame: HWND, frame_pid: u32) -> u32 {
    unsafe extern "system" fn visit(child: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam (frame_pid, sonuç) çiftini gösterir; yalnız çağrı süresince.
        let ctx = unsafe { &mut *(lparam.0 as *mut (u32, u32)) };
        let pid = window_pid(child);
        if pid != 0 && pid != ctx.0 {
            ctx.1 = pid;
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut ctx = (frame_pid, 0u32);
    // SAFETY: bağlam çağrı süresince yaşar.
    let _ = unsafe { EnumChildWindows(Some(frame), Some(visit), LPARAM(&mut ctx as *mut _ as isize)) };
    ctx.1
}

impl Inner {
    fn current_foreground(&self) -> Option<WindowInfo> {
        self.current_foreground.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn update_foreground_window(&self) {
        // SAFETY: yalnız sorgu.
        let h = unsafe { GetForegroundWindow() };
        if h.is_invalid() {
            let had = self.current_foreground.lock().unwrap_or_else(|e| e.into_inner()).take().is_some();
            if had {
                self.foreground_window_changed.invoke(&WindowInfo { handle: 0, bounds: WindowRect::EMPTY });
            }
            return;
        }
        let mut bounds = WindowRect::EMPTY;
        if !is_shell_or_desktop(h)
            && let Some(r) = rect_of(h)
        {
            bounds = WindowRect { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
        }
        let info = WindowInfo { handle: h.0 as isize, bounds };
        {
            let mut cur = self.current_foreground.lock().unwrap_or_else(|e| e.into_inner());
            if cur.as_ref() == Some(&info) {
                return;
            }
            *cur = Some(info.clone());
        }
        self.foreground_window_changed.invoke(&info);
    }

    fn get_app_user_model_id(&self, h: HWND, pid: u32) -> Option<String> {
        let handle = h.0 as isize;
        if let Some((value, read)) = self.aumid_cache.lock().unwrap_or_else(|e| e.into_inner()).get(&handle)
            && (value.is_some() || read.elapsed() < EMPTY_AUMID_RETRY)
        {
            return value.clone();
        }
        // Paketli uygulamalar pencereye değil pakete AUMID verir: süreçten oku.
        let value = AppUserModelIdReader::for_window(handle).or_else(|| AppUserModelIdReader::for_process(pid));
        self.aumid_cache.lock().unwrap_or_else(|e| e.into_inner()).insert(handle, (value.clone(), Instant::now()));
        value
    }

    fn handle_win_event(&self, event: u32, h: HWND, id_object: i32, id_child: i32) {
        if h.is_invalid() {
            return;
        }
        // Koy'un kendi pencerelerinin olayları yoksayılır; aksi halde popup açılışında döngüsel tetiklenme oluşur.
        if window_pid(h) == std::process::id() {
            return;
        }
        let handle = h.0 as isize;
        let top = id_object == 0 && id_child == 0;
        if event == EVENT_OBJECT_DESTROY && top {
            self.aumid_cache.lock().unwrap_or_else(|e| e.into_inner()).remove(&handle);
        }
        // Yaşam döngüsü olayları (pencere açılma / kapanma).
        if matches!(event, EVENT_OBJECT_CREATE | EVENT_OBJECT_DESTROY | EVENT_OBJECT_SHOW | EVENT_OBJECT_HIDE) {
            if top {
                self.windows_changed.invoke(&());
            }
            return;
        }
        // Gizlilikten çıkış: yalnız üst düzey ve görev çubuğu penceresi ise. Gizlenme pencereyi listeden düşürür.
        if event == EVENT_OBJECT_UNCLOAKED || event == EVENT_OBJECT_CLOAKED {
            if top && is_root(h) && (event == EVENT_OBJECT_CLOAKED || is_top_level_taskbar_window(h)) {
                self.windows_changed.invoke(&());
            }
            return;
        }
        // Başlık değişimi: yalnız listede olmayan görev çubuğu penceresi yenileme ister.
        if event == EVENT_OBJECT_NAMECHANGE {
            let known = self.known_windows.read().unwrap_or_else(|e| e.into_inner()).contains(&handle);
            if top && !known && is_top_level_taskbar_window(h)
                && let Some(d) = self.hooks.lock().unwrap_or_else(|e| e.into_inner()).debouncer.as_ref()
            {
                d.request();
            }
            return;
        }
        // Konum değişiminde yalnız ana pencere ve yalnız geçerli ön plan penceresi.
        if event == EVENT_OBJECT_LOCATIONCHANGE {
            if id_object != 0 {
                return;
            }
            if self.current_foreground().is_some_and(|f| f.handle != handle) {
                return;
            }
        }
        self.update_foreground_window();
    }
}

unsafe extern "system" fn on_win_event(_hook: HWINEVENTHOOK, event: u32, h: HWND, id_object: i32, id_child: i32, _thread: u32, _time: u32) {
    let inner = CURRENT.with(|c| c.borrow().as_ref().and_then(Weak::upgrade));
    let Some(inner) = inner else { return };
    // Yerel geri çağrıdan sızan panik süreci düşürür; logla ve yut.
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inner.handle_win_event(event, h, id_object, id_child))).is_err() {
        koy_core::diagnostics::CrashLog::write("WindowTracker.OnWinEvent", &koy_core::diagnostics::CrashError::exception("Panic", "panic", ""));
    }
}

impl WindowTracker {
    /// `post`: başlık değişimi bildirimini arayüz iş parçacığına taşır (C# SynchronizationContext).
    pub fn new(post: Option<PostFn>) -> Self {
        WindowTracker {
            inner: Arc::new(Inner {
                started: Mutex::new(false),
                hooks: Mutex::new(Hooks { handles: Vec::new(), debouncer: None, shell_hook_windows: Vec::new() }),
                current_foreground: Mutex::new(None),
                known_windows: RwLock::new(HashSet::new()),
                aumid_cache: Mutex::new(HashMap::new()),
                shell_hook_message: Mutex::new(0),
                post,
                foreground_window_changed: Event::new(),
                windows_changed: Event::new(),
                window_flashing: Event::new(),
            }),
        }
    }

    /// İzleyici çalışıyor mu.
    pub fn is_started(&self) -> bool {
        *self.inner.started.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn shell_hook_message(&self) -> u32 {
        *self.inner.shell_hook_message.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn register_shell_hook(&self, window_handle: isize) {
        if window_handle == 0 {
            return;
        }
        let mut hooks = self.inner.hooks.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: pencere çağıranın.
        if !hooks.shell_hook_windows.contains(&window_handle) && unsafe { RegisterShellHookWindow(hwnd(window_handle)) }.as_bool() {
            hooks.shell_hook_windows.push(window_handle);
            // SAFETY: sabit ad.
            *self.inner.shell_hook_message.lock().unwrap_or_else(|e| e.into_inner()) = unsafe { RegisterWindowMessageW(w!("SHELLHOOK")) };
        }
    }

    pub fn deregister_shell_hook(&self, window_handle: isize) {
        if window_handle == 0 {
            return;
        }
        let mut hooks = self.inner.hooks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = hooks.shell_hook_windows.iter().position(|&h| h == window_handle) {
            hooks.shell_hook_windows.remove(i);
            // SAFETY: kendi kaydımız.
            let _ = unsafe { DeregisterShellHookWindow(hwnd(window_handle)) };
        }
    }

    pub fn handle_shell_hook_message(&self, wparam: isize, lparam: isize) {
        const HSHELL_WINDOWCREATED: i32 = 1;
        const HSHELL_WINDOWDESTROYED: i32 = 2;
        const HSHELL_ACTIVATESHELLWINDOW: i32 = 3;
        const HSHELL_WINDOWACTIVATED: i32 = 4;
        const HSHELL_REDRAW: i32 = 6;
        const HSHELL_FLASH: i32 = 0x8006;
        const HSHELL_RUDEAPPACTIVATED: i32 = 0x8004;
        match wparam as i32 {
            HSHELL_FLASH => {
                self.inner.window_flashing.invoke(&lparam);
                self.inner.windows_changed.invoke(&());
            }
            HSHELL_WINDOWCREATED | HSHELL_WINDOWDESTROYED | HSHELL_REDRAW => self.inner.windows_changed.invoke(&()),
            HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED | HSHELL_ACTIVATESHELLWINDOW => self.inner.update_foreground_window(),
            _ => {}
        }
    }

    pub fn dispose(&self) {
        self.stop();
    }
}

impl Drop for WindowTracker {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IWindowTracker for WindowTracker {
    fn foreground_window_changed(&self) -> &Event<WindowInfo> {
        &self.inner.foreground_window_changed
    }

    fn windows_changed(&self) -> &Event<()> {
        &self.inner.windows_changed
    }

    fn window_flashing(&self) -> &Event<isize> {
        &self.inner.window_flashing
    }

    fn current_foreground(&self) -> Option<WindowInfo> {
        self.inner.current_foreground()
    }

    fn get_open_windows(&self) -> Vec<OpenWindowInfo> {
        struct Ctx<'a> {
            inner: &'a Inner,
            windows: Vec<OpenWindowInfo>,
            seen: HashSet<isize>,
        }

        unsafe extern "system" fn visit(h: HWND, lparam: LPARAM) -> BOOL {
            // SAFETY: lparam yalnız EnumWindows süresince geçerli bağlamı gösterir.
            let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
            if h.is_invalid() {
                return BOOL(1);
            }
            let handle = h.0 as isize;
            ctx.seen.insert(handle);
            // Görev çubuğu kuralı Core'da (TaskbarWindowRule).
            if !TaskbarWindowRule::should_show(&read_taskbar_facts(h)) {
                return BOOL(1);
            }
            // Boş başlık eleme sebebi değil; boşsa dock süreç adını gösterir.
            let mut buf = [0u16; 256];
            // SAFETY: arabellek boyutu verilir.
            let len = unsafe { GetWindowTextW(h, &mut buf) };
            let title = if len > 0 { String::from_utf16_lossy(&buf[..len as usize]) } else { String::new() };
            let Some(wr) = rect_of(h).filter(|r| r.right > r.left && r.bottom > r.top) else { return BOOL(1) };

            let mut pid = window_pid(h);
            // Mağaza penceresinin sahibi ApplicationFrameHost.exe'dir: gerçek süreci çocuk CoreWindow'dan bul; bulunamazsa 0.
            let cls = class_name(h);
            let is_frame = cls == "ApplicationFrameWindow";
            if is_frame {
                pid = resolve_uwp_process_id(h, pid);
            }
            // AUMID: Windows görev çubuğu düğmeleri bununla gruplar.
            let aumid = ctx.inner.get_app_user_model_id(h, pid);
            let mut proc_path = get_process_path(pid);
            // Mağaza uygulamaları tek öğede toplanmasın diye AUMID'den kabuk yolu üretilir.
            if is_frame && let Some(a) = aumid.as_deref().filter(|a| !a.is_empty()) {
                proc_path = Some(format!(r"shell:AppsFolder\{a}"));
            }
            let icon = proc_path.as_deref().filter(|p| !p.is_empty()).and_then(IconHelper::get_icon_bytes);
            let mut info = OpenWindowInfo::new(handle, title, proc_path, pid as i32, WindowRect { left: wr.left, top: wr.top, right: wr.right, bottom: wr.bottom });
            info.icon_data = icon;
            info.app_user_model_id = aumid;
            info.class_name = (!cls.is_empty()).then(|| cls.chars().take(127).collect());
            ctx.windows.push(info);
            BOOL(1)
        }

        let mut ctx = Ctx { inner: &self.inner, windows: Vec::new(), seen: HashSet::new() };
        // SAFETY: bağlam çağrı süresince yaşar.
        let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut ctx as *mut Ctx as isize)) };
        {
            let mut cache = self.inner.aumid_cache.lock().unwrap_or_else(|e| e.into_inner());
            if cache.len() > ctx.seen.len() {
                cache.retain(|k, _| ctx.seen.contains(k));
            }
        }
        *self.inner.known_windows.write().unwrap_or_else(|e| e.into_inner()) = ctx.windows.iter().map(|w| w.handle).collect();
        ctx.windows
    }

    fn activate_window(&self, handle: isize) {
        if handle == 0 {
            return;
        }
        let h = hwnd(handle);
        // SAFETY: pencere çağrıları.
        unsafe {
            if IsIconic(h).as_bool() {
                let _ = ShowWindow(h, SW_RESTORE);
            }
            let _ = SetForegroundWindow(h);
            let _ = BringWindowToTop(h);
        }
    }

    fn minimize_window(&self, handle: isize) {
        if handle != 0 {
            // SAFETY: pencere çağrısı.
            let _ = unsafe { ShowWindow(hwnd(handle), SW_MINIMIZE) };
        }
    }

    fn close_window(&self, handle: isize) {
        if handle != 0 {
            // SAFETY: yalnız ileti gönderir.
            let _ = unsafe { PostMessageW(Some(hwnd(handle)), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
    }

    fn terminate_process(&self, process_id: i32) {
        if process_id <= 0 {
            return;
        }
        // Süreç zaten sonlanmış olabilir: hata yutulur.
        let name = get_process_path(process_id as u32).map(|p| koy_core::win_path::get_file_name_without_extension(&p).to_string());
        let Some(name) = name else { return };
        // Ortak çatı süreçleri (ApplicationFrameHost, explorer…) öldürülürse ilgisiz uygulamalar da kapanır.
        if !ProcessTerminationPolicy::can_terminate(process_id, Some(&name), std::process::id() as i32) {
            return;
        }
        // SAFETY: tutamaç kapatılır.
        unsafe {
            if let Ok(p) = OpenProcess(PROCESS_TERMINATE, false, process_id as u32) {
                let _ = TerminateProcess(p, 1);
                let _ = windows::Win32::Foundation::CloseHandle(p);
            }
        }
    }

    fn start(&self) {
        {
            let mut started = self.inner.started.lock().unwrap_or_else(|e| e.into_inner());
            if *started {
                return;
            }
            *started = true;
        }
        CURRENT.with(|c| *c.borrow_mut() = Some(Arc::downgrade(&self.inner)));
        let hook = |min: u32, max: u32| {
            // SAFETY: süreç dışı WinEvent kancası; geri çağrı bu iş parçacığının mesaj döngüsünde çalışır.
            unsafe { SetWinEventHook(min, max, None, Some(on_win_event), 0, 0, WINEVENT_OUTOFCONTEXT) }
        };
        // Başlık değişimi sık gelebilir: olaylar tek bir bildirimde birleşir (tek atımlık erteleme; boşta zamanlayıcı yok).
        let weak = Arc::downgrade(&self.inner);
        let post = self.inner.post.clone().map(|p| Arc::new(move |work: PostedWork| p(work)) as Arc<dyn Fn(PostedWork) + Send + Sync>);
        let debouncer = RefreshDebouncer::new(
            Arc::new(move || {
                if let Some(i) = weak.upgrade() {
                    i.windows_changed.invoke(&());
                }
            }),
            TimeSpan::from_millis(NAME_CHANGE_DELAY_MS),
            post,
            None,
        );
        {
            let mut hooks = self.inner.hooks.lock().unwrap_or_else(|e| e.into_inner());
            hooks.handles = vec![
                hook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
                hook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
                hook(EVENT_OBJECT_CREATE, EVENT_OBJECT_HIDE),
                // Sanal masaüstü geçişi, Mağaza uygulaması: SHOW/HIDE gelmez, bunlar gelir.
                hook(EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
                hook(EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
            ];
            hooks.debouncer = Some(debouncer);
        }
        self.inner.update_foreground_window();
    }

    fn stop(&self) {
        {
            let mut started = self.inner.started.lock().unwrap_or_else(|e| e.into_inner());
            if !*started {
                return;
            }
            *started = false;
        }
        let mut hooks = self.inner.hooks.lock().unwrap_or_else(|e| e.into_inner());
        for h in hooks.shell_hook_windows.drain(..) {
            // SAFETY: kendi kayıtlarımız.
            let _ = unsafe { DeregisterShellHookWindow(hwnd(h)) };
        }
        for h in hooks.handles.drain(..) {
            if !h.is_invalid() {
                // SAFETY: kendi kancalarımız.
                let _ = unsafe { UnhookWinEvent(h) };
            }
        }
        if let Some(d) = hooks.debouncer.take() {
            d.dispose();
        }
    }

    fn get_cursor_position(&self) -> Option<(i32, i32)> {
        CursorHelper::get_position()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_open_windows() {
        let tracker = WindowTracker::default();
        let windows = tracker.get_open_windows();
        assert!(windows.iter().all(|w| w.handle != 0));
        tracker.start();
        assert!(tracker.is_started());
        tracker.stop();
    }
}

//! Arama panelinin sistem işleri: güç/oturum komutları, sonuç açma, süreç listesi, yer imleri ve bilinen klasörler.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::search::{
    BrowserBookmark, ChromiumBookmarks, FileStampCache, ISearchShell, ISystemCommandService, RunningProcess, SearchCommandSafety,
    SearchPlace, SystemCommand,
};
use koy_core::text;
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::RemoteDesktop::WTSDisconnectSession;
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Music, FOLDERID_Pictures, FOLDERID_Profile, FOLDERID_Videos};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE};
use windows::core::BOOL;

use super::process_start::{ShellExecuteRequest, shell_execute};
use super::win_env::{filetime_to_ticks, known_folder, last_write_utc_ticks};

/// `Environment.SpecialFolder.System`.
fn system_directory() -> String {
    let mut buf = [0u16; 260];
    // SAFETY: arabellek 260 karakter.
    let n = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
    String::from_utf16_lossy(&buf[..n.min(buf.len())])
}

/// Kilitle, uyku, oturum ve güç komutları.
#[derive(Default)]
pub struct SystemCommandService;

impl SystemCommandService {
    const CURRENT_SESSION: u32 = 0xFFFF_FFFF;

    fn run_shutdown(arguments: &str) {
        let exe = Path::new(&system_directory()).join("shutdown.exe");
        let _ = Command::new(exe).args(arguments.split(' ')).creation_flags(CREATE_NO_WINDOW.0).spawn();
    }
}

impl ISystemCommandService for SystemCommandService {
    fn execute(&self, command: SystemCommand) {
        // SAFETY: parametresiz/sabit parametreli sistem çağrıları.
        unsafe {
            match command {
                SystemCommand::Lock => {
                    let _ = LockWorkStation();
                }
                SystemCommand::Sleep => {
                    let _ = SetSuspendState(false, false, false);
                }
                SystemCommand::SwitchUser => {
                    let _ = WTSDisconnectSession(None, Self::CURRENT_SESSION, false);
                }
                SystemCommand::SignOut => Self::run_shutdown("/l"),
                SystemCommand::Restart => Self::run_shutdown("/r /t 0"),
                SystemCommand::Shutdown => Self::run_shutdown("/s /t 0"),
            }
        }
    }
}

/// Arama sonuçlarının dosya sistemi ve süreç işleri. Pano arayüze bağlı olduğu için dışarıdan verilir.
pub struct SearchShell {
    copy_text: Box<dyn Fn(&str) + Send + Sync>,
}

impl SearchShell {
    pub fn new(copy_text: impl Fn(&str) + Send + Sync + 'static) -> Self {
        SearchShell { copy_text: Box::new(copy_text) }
    }

    fn start(request: ShellExecuteRequest<'_>) {
        if let Err(code) = shell_execute(request) {
            // Hedef silinmiş ya da kullanıcı yetki istemini iptal etmiş.
            CrashLog::write("Arama sonucu açma", &CrashError::exception("System.ComponentModel.Win32Exception", &format!("Win32 error {code}"), ""));
        }
    }
}

impl ISearchShell for SearchShell {
    fn file_exists(&self, path: &str) -> bool {
        Path::new(path).is_file()
    }

    fn directory_exists(&self, path: &str) -> bool {
        Path::new(path).is_dir()
    }

    fn get_environment_variable(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn open(&self, target: &str) {
        Self::start(ShellExecuteRequest { file: target, ..Default::default() });
    }

    fn run(&self, executable: &str, arguments: Option<&str>) {
        Self::start(ShellExecuteRequest { file: executable, arguments: Some(arguments.unwrap_or("")), ..Default::default() });
    }

    fn run_with_arguments(&self, executable: &str, arguments: &[String]) {
        // UseShellExecute = false + ArgumentList: Rust `Command` aynı MSVCRT tırnaklama kuralını uygular.
        if let Err(e) = Command::new(executable).args(arguments).spawn() {
            CrashLog::write("Arama sonucu açma", &CrashError::exception("System.ComponentModel.Win32Exception", &e.to_string(), ""));
        }
    }

    fn run_in_terminal(&self, command_line: &str) {
        let args = format!("/k {}", SearchCommandSafety::to_single_line(command_line));
        Self::start(ShellExecuteRequest { file: "cmd.exe", arguments: Some(&args), ..Default::default() });
    }

    fn open_containing_folder(&self, path: &str) {
        let args = format!("/select,\"{path}\"");
        Self::start(ShellExecuteRequest { file: "explorer.exe", arguments: Some(&args), ..Default::default() });
    }

    fn copy_text(&self, text: &str) {
        (self.copy_text)(text);
    }
}

/// Çalışan süreçler ve kapatma isteği.
pub struct SearchProcesses;

struct ProcessEntry {
    id: u32,
    name: String,
}

fn snapshot_processes() -> Vec<ProcessEntry> {
    let mut out = Vec::new();
    // SAFETY: anlık görüntü tutamacı sonunda kapatılır; PROCESSENTRY32W boyutu doldurulur.
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
            // Process.ProcessName: ".exe" uzantısı atılır.
            let name = if text::ends_with_ignore_case(&exe, ".exe") { exe[..exe.len() - 4].to_string() } else { exe };
            out.push(ProcessEntry { id: entry.th32ProcessID, name });
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    out
}

/// `process.StartTime.ToUniversalTime().Ticks`; korumalı süreçte 0 (yalnız ad karşılaştırılır).
fn start_ticks(pid: u32) -> i64 {
    // SAFETY: sınırlı sorgu hakkıyla açılır ve kapatılır.
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return 0 };
        let (mut creation, mut exit, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user).is_ok();
        let _ = CloseHandle(handle);
        if !ok {
            return 0;
        }
        filetime_to_ticks(((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64)
    }
}

fn find_process(id: i32) -> Option<ProcessEntry> {
    snapshot_processes().into_iter().find(|p| p.id as i32 == id)
}

// Liste alındığından beri kimlik başka sürece verilmiş olabilir: ad ve başlangıç zamanı aynı olmalı.
fn is_same(process: &ProcessEntry, target: &RunningProcess) -> bool {
    text::eq_ignore_case(&process.name, &target.name) && start_ticks(process.id) == target.start_ticks
}

/// `Process.MainWindowHandle`: sürecin sahipsiz, görünür ilk üst düzey penceresi.
fn main_window(pid: u32) -> Option<HWND> {
    struct Search {
        pid: u32,
        found: Option<HWND>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam bizim Search değişkenimizi gösterir.
        let search = unsafe { &mut *(lparam.0 as *mut Search) };
        let mut owner_pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner_pid)) };
        let unowned = unsafe { GetWindow(hwnd, GW_OWNER) }.map_or(true, |h| h.is_invalid());
        if owner_pid == search.pid && unowned && unsafe { IsWindowVisible(hwnd) }.as_bool() {
            search.found = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut search = Search { pid, found: None };
    // SAFETY: geri çağrı yalnız EnumWindows süresince `search`'e erişir.
    let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize)) };
    search.found
}

impl SearchProcesses {
    pub fn list() -> Vec<RunningProcess> {
        snapshot_processes()
            .into_iter()
            .filter(|p| p.id > 4)
            .map(|p| RunningProcess::new(p.id as i32, p.name.clone(), start_ticks(p.id)))
            .collect()
    }

    pub fn request_close(target: &RunningProcess) {
        // Süreç bu arada kapanmış olabilir.
        let Some(process) = find_process(target.id) else { return };
        if is_same(&process, target)
            && let Some(hwnd) = main_window(process.id)
        {
            // SAFETY: yalnız mesaj gönderilir.
            let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
    }

    /// Zorla sonlandırır; kimlik başka bir sürece geçmişse dokunmaz.
    pub fn terminate(target: &RunningProcess, kill: impl FnOnce(i32)) {
        let Some(process) = find_process(target.id) else { return };
        if is_same(&process, target) {
            kill(target.id);
        }
    }
}

/// Chrome ailesi tarayıcıların yer imi dosyaları.
pub struct SearchBookmarks;

const BOOKMARK_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const BROWSERS: [&str; 3] = [r"Google\Chrome\User Data", r"Microsoft\Edge\User Data", r"BraveSoftware\Brave-Browser\User Data"];

fn bookmark_cache() -> &'static FileStampCache<Vec<BrowserBookmark>> {
    static CACHE: OnceLock<FileStampCache<Vec<BrowserBookmark>>> = OnceLock::new();
    CACHE.get_or_init(FileStampCache::new)
}

impl SearchBookmarks {
    pub fn load() -> Vec<BrowserBookmark> {
        let root = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default());
        let mut bookmarks = Vec::new();
        for browser in BROWSERS {
            let directory = root.join(browser);
            let Ok(entries) = std::fs::read_dir(&directory) else { continue };
            let mut profiles: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir() && is_profile(p)).collect();
            profiles.sort();
            for profile in profiles {
                bookmarks.extend(read_file(&profile.join("Bookmarks")));
            }
        }
        bookmarks.extend(super::firefox_bookmarks::FirefoxBookmarks::load());

        // DistinctBy(Url, OrdinalIgnoreCase): ilk görülen kalır.
        let mut seen: Vec<String> = Vec::new();
        bookmarks.retain(|b| {
            let key = text::to_upper_invariant(&b.url);
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        });
        bookmarks
    }
}

fn is_profile(directory: &Path) -> bool {
    let name = directory.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    name == "Default" || name.starts_with("Profile ")
}

fn read_file(path: &Path) -> Vec<BrowserBookmark> {
    let key = path.to_string_lossy();
    let Ok(meta) = std::fs::metadata(path) else {
        bookmark_cache().remove(&key);
        return Vec::new();
    };
    if !meta.is_file() || meta.len() > BOOKMARK_MAX_FILE_BYTES {
        bookmark_cache().remove(&key);
        return Vec::new();
    }
    // Dosya değişmediyse (son yazma zamanı + boyut) ayrıştırılmış liste yeniden kullanılır.
    bookmark_cache().get_or_load(&key, last_write_utc_ticks(&meta), meta.len() as i64, || {
        // Tarayıcı dosyayı yazıyor olabilir: std paylaşımlı okuma ile açar (FILE_SHARE_READ | WRITE | DELETE).
        match std::fs::read(path) {
            Ok(bytes) => {
                let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
                ChromiumBookmarks::parse(&String::from_utf8_lossy(bytes))
            }
            Err(_) => Vec::new(),
        }
    })
}

/// Bilinen klasörler ve yerel sürücüler.
pub struct SearchPlaces;

impl SearchPlaces {
    pub fn load(t: impl Fn(&str) -> String) -> Vec<SearchPlace> {
        let home = known_folder(&FOLDERID_Profile);
        let mut places = vec![
            SearchPlace::new(t("SearchPlaceHome"), home.clone(), None),
            SearchPlace::new(t("SearchPlaceDesktop"), known_folder(&FOLDERID_Desktop), None),
            SearchPlace::new(t("SearchPlaceDocuments"), known_folder(&FOLDERID_Documents), None),
            SearchPlace::new(t("SearchPlaceDownloads"), koy_core::win_path::combine(&home, "Downloads"), None),
            SearchPlace::new(t("SearchPlaceMusic"), known_folder(&FOLDERID_Music), None),
            SearchPlace::new(t("SearchPlacePictures"), known_folder(&FOLDERID_Pictures), None),
            SearchPlace::new(t("SearchPlaceVideos"), known_folder(&FOLDERID_Videos), None),
        ];

        let devices = t("SearchPlaceDevices");
        // Ağ ve optik sürücüler yoklanmaz: yanıt vermezlerse panel açılışı bekler.
        // SAFETY: parametresiz çağrı.
        let mask = unsafe { GetLogicalDrives() };
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let root = format!("{}:\\", (b'A' + i as u8) as char);
            let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: sıfırla biten yol.
            let kind = unsafe { GetDriveTypeW(windows::core::PCWSTR(wide.as_ptr())) };
            // DRIVE_REMOVABLE = 2, DRIVE_FIXED = 3.
            if kind == 2 || kind == 3 {
                places.push(SearchPlace::new(root.trim_end_matches('\\'), root.clone(), Some(devices.clone())));
            }
        }

        places.into_iter().filter(|p| !p.path.is_empty()).collect()
    }
}

#[allow(dead_code)]
fn _assert_handle_send(_: HANDLE) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_processes_with_start_times() {
        let own = std::process::id() as i32;
        let list = SearchProcesses::list();
        let me = list.iter().find(|p| p.id == own).expect("own process listed");
        assert!(me.start_ticks > 0);
        assert!(!me.name.to_ascii_lowercase().ends_with(".exe"));
    }

    #[test]
    fn places_include_home_and_a_fixed_drive() {
        let places = SearchPlaces::load(|k| k.to_string());
        assert!(places.iter().any(|p| p.name == "SearchPlaceHome" && !p.path.is_empty()));
        assert!(places.iter().any(|p| p.group.as_deref() == Some("SearchPlaceDevices")));
    }
}

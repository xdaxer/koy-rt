//! Shell klasörünün küçük, durumsuz yardımcıları: `CursorHelper`, `PopupMenuProbe`, `StartupService`,
//! `RecentFilesService`, `TaskbandFavoritesReader`.

use std::path::{Path, PathBuf};

use koy_core::dock::{IRecentFilesService, RecentEntry, RecentFile, RecentFilesPolicy};
use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::shell::{StartupPathPolicy, WindowRect};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetDoubleClickTime};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetCursorPos, GetWindowRect, IsWindowVisible};
use windows::core::{PCWSTR, w};

use super::window_styles::{WindowStyles, hwnd};
use crate::registry::{self, Hive};

/// İmleç (fare) konumu ve tuş durumu.
pub struct CursorHelper;

impl CursorHelper {
    pub fn get_position() -> Option<(i32, i32)> {
        let mut pt = POINT::default();
        // SAFETY: çıktı yerel değişkene yazılır.
        unsafe { GetCursorPos(&mut pt) }.ok().map(|_| (pt.x, pt.y))
    }

    /// Sol, sağ ya da orta fare tuşu şu an basılı mı (pencere sürüklerken kenara inince dock açılmasın). Yoklama tabanlı.
    pub fn is_any_mouse_button_down() -> bool {
        // SAFETY: yalnız tuş durumu sorgusu.
        let down = |vk: i32| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0;
        down(0x01) || down(0x02) || down(0x04)
    }

    /// Kullanıcının Windows'ta ayarladığı çift tıklama süresi (ms).
    pub fn double_click_time_ms() -> u32 {
        // SAFETY: yalnız sorgu.
        unsafe { GetDoubleClickTime() }
    }
}

const MENU_CLASS_NAME: PCWSTR = w!("#32768");

fn rect_of(h: HWND) -> Option<WindowRect> {
    let mut r = RECT::default();
    // SAFETY: çıktı yerel değişkene yazılır.
    unsafe { GetWindowRect(h, &mut r) }.ok().map(|_| WindowRect { left: r.left, top: r.top, right: r.right, bottom: r.bottom })
}

fn menus() -> impl Iterator<Item = HWND> {
    let mut current = HWND::default();
    std::iter::from_fn(move || {
        // SAFETY: üst düzey pencereler arasında sınıf adına göre arama.
        current = unsafe { FindWindowExW(None, Some(current), MENU_CLASS_NAME, PCWSTR::null()) }.ok()?;
        (!current.is_invalid()).then_some(current)
    })
}

/// Ekranda görünen bağlam menülerini ("#32768", alt menüler dahil) ve pencere dikdörtgenlerini sorgular; kanca kurmaz.
pub struct PopupMenuProbe;

impl PopupMenuProbe {
    /// Görünen menü pencerelerinin ekran dikdörtgenleri. `bring_to_front` açıksa her menü odak çalmadan en üstteki
    /// katmanın önüne alınır (dock da topmost olduğundan menü onun arkasında kalmasın).
    pub fn get_visible_menu_rects(bring_to_front: bool) -> Vec<WindowRect> {
        let mut result = Vec::new();
        for h in menus() {
            // SAFETY: yalnız sorgu.
            if !unsafe { IsWindowVisible(h) }.as_bool() {
                continue;
            }
            let Some(rect) = rect_of(h) else { continue };
            result.push(rect);
            if bring_to_front {
                WindowStyles::reassert_topmost(h.0 as isize);
            }
        }
        result
    }

    /// Ekranda görünen bir bağlam menüsü var mı.
    pub fn any_visible_menu() -> bool {
        // SAFETY: yalnız sorgu.
        menus().any(|h| unsafe { IsWindowVisible(h) }.as_bool())
    }

    /// Pencere tanıtıcısının ekran dikdörtgeni; geçersizse `None`.
    pub fn try_get_window_rect(handle: isize) -> Option<WindowRect> {
        if handle == 0 {
            return None;
        }
        rect_of(hwnd(handle))
    }
}

type PathResolver = Box<dyn Fn() -> Option<String> + Send + Sync>;
type FlagResolver = Box<dyn Fn() -> bool + Send + Sync>;

static TARGET_RESOLVER: std::sync::RwLock<Option<PathResolver>> = std::sync::RwLock::new(None);
static INSTALLED_RESOLVER: std::sync::RwLock<Option<FlagResolver>> = std::sync::RwLock::new(None);

fn process_path() -> Option<String> {
    std::env::current_exe().ok().map(|p| p.to_string_lossy().into_owned()).filter(|p| !p.is_empty())
}

fn file_exists(p: &str) -> bool {
    std::path::Path::new(p).is_file()
}

/// Oturum açılışında başlatma (HKCU Run ve Görev Yöneticisi'nin StartupApproved\Run kaydı).
/// Karşılığı: `Platform/Shell/StartupService.cs`; yol, biçim ve göç kararları Core `StartupPathPolicy`'dedir.
pub struct StartupService;

impl StartupService {
    /// Uygulama katmanı kurulum bilgisiyle hedef yol çözücüsünü bağlar (Platform kurulum aracına bağımlı olmaz).
    pub fn configure(target: impl Fn() -> Option<String> + Send + Sync + 'static, is_installed: Option<Box<dyn Fn() -> bool + Send + Sync>>) {
        *TARGET_RESOLVER.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(target));
        *INSTALLED_RESOLVER.write().unwrap_or_else(|e| e.into_inner()) = is_installed;
    }

    fn target() -> Option<String> {
        match TARGET_RESOLVER.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
            Some(f) => f(),
            None => process_path(),
        }
    }

    pub fn is_installed_build() -> bool {
        INSTALLED_RESOLVER.read().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|f| f())
    }

    /// Kayıt var, Görev Yöneticisi'nde kapatılmamış, dosya duruyor ve beklenen hedefe bakıyor mu.
    pub fn is_enabled() -> bool {
        let Some(raw) = registry::get_string(Hive::CurrentUser, StartupPathPolicy::RUN_KEY_PATH, Some(StartupPathPolicy::APP_VALUE_NAME), false)
            .filter(|r| !r.trim().is_empty())
        else {
            return false;
        };
        let approved = read_startup_approved_bytes();
        let expected = Self::target();
        StartupPathPolicy::is_entry_valid(Some(&raw), approved.as_deref(), expected.as_deref(), file_exists)
    }

    /// Başarılıysa true. Açarken Run anahtarı yoksa oluşturulur (temiz Windows kurulumu); Görev Yöneticisi'nin eski
    /// "kapalı" kaydı silinir ki girdi gerçekten çalışsın.
    pub fn set_enabled(enable: bool) -> bool {
        if enable {
            let Some(exe) = Self::target().filter(|p| !p.trim().is_empty()) else { return false };
            let Some(cmd) = StartupPathPolicy::format_command_line(&exe, None) else { return false };
            if !registry::set_string(Hive::CurrentUser, StartupPathPolicy::RUN_KEY_PATH, StartupPathPolicy::APP_VALUE_NAME, &cmd) {
                CrashLog::write("StartupService.SetEnabled", &CrashError::object("Run anahtarına yazılamadı"));
                return false;
            }
            clear_startup_approved();
            return true;
        }
        registry::delete_value(Hive::CurrentUser, StartupPathPolicy::RUN_KEY_PATH, StartupPathPolicy::APP_VALUE_NAME);
        clear_startup_approved();
        true
    }

    /// Kurulu sürüm açılırken Run kaydı eski / geliştirme / taşınabilir ya da `current\Koy.exe` yoluna bakıyorsa
    /// kalıcı kök saplamaya geçirir. Kurulu olmayan çalıştırmada dokunmaz.
    pub fn reconcile_installed() {
        if !Self::is_installed_build() {
            return;
        }
        let Some(expected) = Self::target().filter(|p| !p.trim().is_empty() && file_exists(p)) else { return };
        let Some(raw) = registry::get_string(Hive::CurrentUser, StartupPathPolicy::RUN_KEY_PATH, Some(StartupPathPolicy::APP_VALUE_NAME), false)
            .filter(|r| !r.trim().is_empty())
        else {
            return;
        };
        let approved = read_startup_approved_bytes();
        if StartupPathPolicy::should_migrate_installed_entry(Some(&raw), approved.as_deref(), Some(&expected), file_exists)
            && let Some(cmd) = StartupPathPolicy::format_command_line(&expected, None)
        {
            registry::set_string(Hive::CurrentUser, StartupPathPolicy::RUN_KEY_PATH, StartupPathPolicy::APP_VALUE_NAME, &cmd);
        }
    }
}

fn read_startup_approved_bytes() -> Option<Vec<u8>> {
    registry::get_binary(Hive::CurrentUser, StartupPathPolicy::STARTUP_APPROVED_RUN_KEY_PATH, StartupPathPolicy::APP_VALUE_NAME)
}

fn clear_startup_approved() {
    registry::delete_value(Hive::CurrentUser, StartupPathPolicy::STARTUP_APPROVED_RUN_KEY_PATH, StartupPathPolicy::APP_VALUE_NAME);
}

/// Windows'un Son Kullanılanlar klasörünü (%AppData%\Microsoft\Windows\Recent) okur. Her çağrıda klasör bir kez
/// listelenir; izleyici, önbellek ya da arka plan işi yoktur. Okuma hatası boş liste verir.
#[derive(Default)]
pub struct RecentFilesService;

fn recent_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"Microsoft\Windows\Recent")
}

impl IRecentFilesService for RecentFilesService {
    fn get_recent(&self, max: i32) -> Vec<RecentFile> {
        let dir = recent_dir();
        let Ok(read) = std::fs::read_dir(&dir) else { return Vec::new() };
        let entries: Vec<RecentEntry> = read
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter(|e| Path::new(&e.file_name()).extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")))
            .filter_map(|e| {
                let modified = e.metadata().ok()?.modified().ok()?;
                let utc: chrono::DateTime<chrono::Utc> = modified.into();
                Some(RecentEntry { shortcut_path: e.path().to_string_lossy().into_owned(), last_write_utc: utc.naive_utc() })
            })
            .collect();
        RecentFilesPolicy::select(&entries, max)
    }
}

/// Windows'un görev çubuğu sabitleme sırasını tutan belgesiz ikili değeri okur (Taskband\Favorites). Yalnız okur;
/// çözümleme Core'daki `PinnedOrder::seed_from_taskband`'dadır. Okunamazsa `None`.
pub struct TaskbandFavoritesReader;

impl TaskbandFavoritesReader {
    pub fn read() -> Option<Vec<u8>> {
        registry::get_binary(Hive::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Explorer\Taskband", "Favorites")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_cursor_and_menus() {
        let _ = CursorHelper::get_position();
        assert!(CursorHelper::double_click_time_ms() > 0);
        let _ = PopupMenuProbe::get_visible_menu_rects(false);
        assert!(PopupMenuProbe::try_get_window_rect(0).is_none());
    }

    #[test]
    fn reads_recent_and_startup_without_writing() {
        let recent = RecentFilesService.get_recent(RecentFilesPolicy::MAX_ITEMS);
        assert!(recent.len() <= RecentFilesPolicy::MAX_ITEMS as usize);
        let _ = StartupService::is_enabled();
        let _ = TaskbandFavoritesReader::read();
    }
}

//! Windows görev çubuğunu gizler ve geri getirir.
//! Güvenlik kuralı (AGENTS.md #7): Çökse bile görev çubuğu geri gelmeli.
//! Görev çubuğu gizlendiğinde durum %AppData%\Koy\taskbar-state.json dosyasına yazılır,
//! geri getirildiğinde ise bu dosya silinir. Açılışta bu dosya kontrol edilir.
//! Bu modül gözcüde de derlenir (gözcü, Koy ölünce aynı `restore`'u çalıştırır); bu yüzden yalnız Core ve Win32'ye
//! bağımlı kalır. Koy'a özel kısım (gözcüyü başlatma/durdurma) `watchdog-launcher` feature'ındadır.

use std::path::PathBuf;

use koy_core::shell::{TaskbarSavedState, TaskbarStatePolicy};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Shell::{APPBARDATA, SHAppBarMessage};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, IsWindowVisible, SW_HIDE, SW_SHOW, ShowWindow};
use windows::core::{PCWSTR, w};

use super::shell_tray_locator::find_explorer_tray;

const ABM_GETSTATE: u32 = 0x0000_0004;
const ABM_SETSTATE: u32 = 0x0000_000A;
const ABS_AUTOHIDE: i32 = TaskbarStatePolicy::AUTO_HIDE;
const SECONDARY_TRAY_CLASS: PCWSTR = w!("Shell_SecondaryTrayWnd");

fn state_file_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Koy").join("taskbar-state.json"))
}

fn app_bar_data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, hWnd: hwnd, ..Default::default() }
}

/// Görev çubuğunu gizler. Birden çok kez çağrılabilir (idempotent): zaten gizliyken kayıtlı "önceki durum"
/// korunur, böylece bizim koyduğumuz AUTOHIDE kullanıcının gerçek durumu sanılıp üzerine yazılmaz.
/// Explorer yeniden başlayınca da aynı çağrı gizlemeyi yeniden uygular.
pub fn hide() {
    let saved = read_saved_state();
    let Some(primary) = find_explorer_tray(0) else {
        // Gezgin henüz görev çubuğunu kurmadıysa (oturum açılışı): önceki durumu tahmin edip dosyaya yazma;
        // Shell_TrayWnd oluşunca yeniden gizleme gerçek durumu okuyup gizler.
        return;
    };
    let mut data = app_bar_data(primary);
    // SAFETY: data geçerli boyutlu APPBARDATA.
    let current = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) } as i32;
    let prev_state = TaskbarStatePolicy::choose_previous_state(saved, current);

    // Zaten otomatik gizlemedeyse yeniden SETSTATE gönderilmez: Gezgin her SETSTATE'te çalışma alanını yeniden
    // hesaplar ve dock'un AppBar payını bir an düşürür (büyütülmüş pencereler sıçrar).
    if current & ABS_AUTOHIDE == 0 {
        data.lParam = LPARAM(ABS_AUTOHIDE as isize);
        // SAFETY: yukarıdaki gibi.
        unsafe { SHAppBarMessage(ABM_SETSTATE, &mut data) };
    }

    save_state(TaskbarSavedState { hidden: true, previous_state: prev_state });
    set_visible(false);
    on_hidden();
}

/// Windows Gezgini görev çubuğu penceresi hazır mı (oturum açılışında henüz oluşmamış olabilir).
pub fn is_explorer_taskbar_ready() -> bool {
    find_explorer_tray(0).is_some()
}

/// Önceki oturumdan kalan geri yüklenecek görev çubuğu durumu var mı.
pub fn has_pending_restore() -> bool {
    TaskbarStatePolicy::should_write_state(read_saved_state(), false)
}

/// Görev çubuğunu (birincil ve bütün ikincil monitörlerdekileri) geri getirir ve durumu önceki haline döndürür.
/// Explorer çalışmıyorsa (birincil çubuk yok) durum dosyası silinmez; sonraki açılış yeniden dener.
///
/// `also_skip_process_id`: gözcü için, ölen Koy sürecinin tepsi penceresini atla.
/// `force`: kurtarma amaçlı, kayıtlı durum olmasa bile ABM_SETSTATE gönder.
pub fn restore(also_skip_process_id: u32, force: bool) {
    // Temiz geri yüklemede gözcü gereksiz: önce durdurulur ki aynı işi ikinci kez yapmasın.
    on_restoring();

    set_visible(true);

    let saved = read_saved_state();
    if !TaskbarStatePolicy::should_write_state(saved, force) {
        delete_state();
        return;
    }

    let Some(primary) = find_explorer_tray(also_skip_process_id) else { return };

    let mut data = app_bar_data(primary);
    data.lParam = LPARAM(TaskbarStatePolicy::resolve_restore_state(saved) as isize);
    // SAFETY: data geçerli boyutlu APPBARDATA.
    unsafe { SHAppBarMessage(ABM_SETSTATE, &mut data) };

    delete_state();
}

/// Gezgin, otomatik gizleme durumu değişince ya da kendi kendine görev çubuğunu yeniden gösterir;
/// o zaman alt kenarda bizim dock'la birlikte ikinci bir çubuk çıkar. Birincil veya ikincil
/// monitörlerdeki görev çubuklarından herhangi biri görünür olduysa tekrar gizler.
pub fn ensure_hidden() -> bool {
    if let Some(primary) = find_explorer_tray(0) {
        // SAFETY: yalnız görünürlük sorgusu.
        if unsafe { IsWindowVisible(primary) }.as_bool() {
            set_visible(false);
            return true;
        }
    }

    if secondary_trays().into_iter().any(|h| unsafe { IsWindowVisible(h) }.as_bool()) {
        set_visible(false);
        return true;
    }

    false
}

fn secondary_trays() -> Vec<HWND> {
    let mut found = Vec::new();
    let mut hwnd = HWND::default();
    loop {
        // SAFETY: sınıf adı sabit ve sıfırla biter.
        hwnd = unsafe { FindWindowExW(None, Some(hwnd), SECONDARY_TRAY_CLASS, PCWSTR::null()) }.unwrap_or_default();
        if hwnd.is_invalid() {
            return found;
        }
        found.push(hwnd);
    }
}

#[cfg(feature = "watchdog-launcher")]
fn on_hidden() {
    super::watchdog_launcher::ensure_running();
}

#[cfg(feature = "watchdog-launcher")]
fn on_restoring() {
    super::watchdog_launcher::stop();
}

// Gözcüde gövde yoktur (C#'taki boş kısmi yöntemler).
#[cfg(not(feature = "watchdog-launcher"))]
fn on_hidden() {}

#[cfg(not(feature = "watchdog-launcher"))]
fn on_restoring() {}

fn set_visible(visible: bool) {
    let command = if visible { SW_SHOW } else { SW_HIDE };

    if let Some(primary) = find_explorer_tray(0) {
        // SAFETY: ShowWindow geçersiz tutamaçta yalnız başarısız olur.
        let _ = unsafe { ShowWindow(primary, command) };
    }

    // Diğer monitörlerdeki görev çubukları.
    for secondary in secondary_trays() {
        // SAFETY: yukarıdaki gibi.
        let _ = unsafe { ShowWindow(secondary, command) };
    }
}

fn save_state(state: TaskbarSavedState) {
    // Durum dosyası yazılamasa bile ana iş akışı kesilmez.
    let Some(path) = state_file_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, TaskbarStatePolicy::serialize(state, chrono::Utc::now().naive_utc()));
}

fn read_saved_state() -> Option<TaskbarSavedState> {
    let path = state_file_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    TaskbarStatePolicy::parse(Some(&text))
}

fn delete_state() {
    // Silme hatası yutulur.
    if let Some(path) = state_file_path() {
        let _ = std::fs::remove_file(path);
    }
}

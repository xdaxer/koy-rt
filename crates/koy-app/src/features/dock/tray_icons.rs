//! Tepsi penceresindeki simgeler. Karşılığı: `Tray/TrayIconViewModel.cs` ve `Tray/TrayAppNames.cs`.

use std::collections::HashSet;
use std::sync::Mutex;

use koy_core::tray::{TrayIcon, TrayIconKey, TrayIconVisibilityPolicy};
use koy_platform::shell::WindowStyles;

use crate::infra::image_decode;

/// Tek tepsi simgesi.
pub struct TrayIconEntry {
    pub key: TrayIconKey,
    pub icon_handle: isize,
    decoded: bool,
    pub tooltip: Option<String>,
    pub icon: Option<slint::Image>,
    pub exe_name: String,
}

fn none_if_empty(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_string())
}

impl TrayIconEntry {
    pub fn new(model: &TrayIcon, exe_name: String) -> Self {
        TrayIconEntry { key: model.key.clone(), icon_handle: 0, decoded: false, tooltip: none_if_empty(&model.tooltip), icon: None, exe_name }
    }

    pub fn update(&mut self, model: &TrayIcon, load_image: impl Fn(&TrayIconKey) -> Option<Vec<u8>>) {
        self.tooltip = none_if_empty(&model.tooltip);
        // Çözülemeyen simge de bir kez denenir; aynı tanıtıcıyla her eşitlemede GDI ve PNG işi tekrarlanmaz.
        if self.decoded && model.icon_handle == self.icon_handle {
            return;
        }
        self.decoded = true;
        self.icon_handle = model.icon_handle;
        // Kendi piksel boyutunda çözülür: 16 px simgeyi büyüterek çözmek bulanıklaştırıyordu.
        self.icon = load_image(&self.key).filter(|p| !p.is_empty()).and_then(|p| image_decode::decode(&p, None)).map(|b| b.to_image());
    }
}

static SEEN: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Şimdiye kadar görülen tepsi uygulamalarının anahtarları (Ayarlar'daki uygulama başına görünürlük listesi).
pub fn seen_app_names() -> Vec<String> {
    SEEN.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|s| s.iter().cloned().collect()).unwrap_or_default()
}

/// Sahip pencerenin sürecinin exe adından anahtar ("app.exe"); süreç kapandıysa boş.
pub fn app_name_for_owner(owner_hwnd: isize) -> String {
    let pid = WindowStyles::get_window_process_id(owner_hwnd);
    if pid == 0 {
        return String::new();
    }
    let Some(name) = process_name(pid) else { return String::new() };
    let key = TrayIconVisibilityPolicy::app_key(Some(&format!("{name}.exe")));
    SEEN.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashSet::new).insert(key.clone());
    key
}

/// Sürecin exe adı (uzantısız; C# `Process.ProcessName`). Süreç kapandıysa ya da sorgu reddedildiyse `None`.
fn process_name(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    use windows::core::PWSTR;
    // SAFETY: süreç tutamacı her yolda kapatılır; arabellek boyutu verilir.
    let path = unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size).is_ok();
        let _ = CloseHandle(process);
        ok.then(|| String::from_utf16_lossy(&buf[..size as usize]))?
    };
    std::path::Path::new(&path).file_stem().and_then(|s| s.to_str()).map(str::to_string)
}

//! Bağlı monitörleri EnumDisplayMonitors ve GetDpiForMonitor ile sorgulayan, çözünürlük veya monitör değişikliklerinde
//! (WM_DISPLAYCHANGE; arayüz katmanı `notify_display_changed` çağırır) olay yayınlayan servis.

use std::sync::Mutex;

use koy_core::display::{DisplayMonitor, IMonitorService, MonitorLayoutSignature};
use koy_core::event::Event;
use koy_core::shell::WindowRect;
use koy_core::text;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::core::BOOL;

use super::display_config_reader::DisplayConfigReader;

const MONITORINFOF_PRIMARY: u32 = 1;

struct State {
    started: bool,
    disposed: bool,
    // Son bildirimdeki ekran düzeni; pencerelerden ve sistem olaylarından gelen tekrarlar aynı düzen için bir kez iletilir.
    last_signature: String,
}

pub struct MonitorService {
    state: Mutex<State>,
    monitors_changed: Event<()>,
}

impl Default for MonitorService {
    fn default() -> Self {
        MonitorService { state: Mutex::new(State { started: false, disposed: false, last_signature: String::new() }), monitors_changed: Event::new() }
    }
}

fn rect(r: &RECT) -> WindowRect {
    WindowRect { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
}

fn fallback_name(device_name: &str) -> String {
    let digits: String = device_name.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() { device_name.to_string() } else { format!("Display {digits}") }
}

/// Kimlik, sınırlar, DPI ve ana ekran imzası (çalışma alanı hariç: Core/MonitorLayoutSignature).
fn signature(monitors: &[DisplayMonitor]) -> String {
    MonitorLayoutSignature::compute(monitors)
}

impl MonitorService {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn dispose(&self) {
        let mut st = self.lock();
        if st.disposed {
            return;
        }
        st.disposed = true;
        st.started = false;
    }
}

impl IMonitorService for MonitorService {
    fn monitors_changed(&self) -> &Event<()> {
        &self.monitors_changed
    }

    fn get_monitors(&self) -> Vec<DisplayMonitor> {
        let configs = DisplayConfigReader::read();
        let mut handles: Vec<HMONITOR> = Vec::new();

        unsafe extern "system" fn collect(hmonitor: HMONITOR, _: HDC, _: *mut RECT, lparam: LPARAM) -> BOOL {
            // SAFETY: lparam bizim vektörümüzü gösterir; yalnız EnumDisplayMonitors süresince kullanılır.
            unsafe { (*(lparam.0 as *mut Vec<HMONITOR>)).push(hmonitor) };
            BOOL(1)
        }
        // SAFETY: geri çağrı yalnız çağrı süresince `handles`'a yazar.
        let _ = unsafe { EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut handles as *mut Vec<HMONITOR> as isize)) };

        let mut list = Vec::new();
        for hmonitor in handles {
            let mut mi = MONITORINFOEXW::default();
            mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            // SAFETY: yapı boyutu doğru ayarlandı.
            if !unsafe { GetMonitorInfoW(hmonitor, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO) }.as_bool() {
                continue;
            }
            let is_primary = mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0;
            let len = mi.szDevice.iter().position(|&c| c == 0).unwrap_or(mi.szDevice.len());
            let device_name = String::from_utf16_lossy(&mi.szDevice[..len]);

            // DPI ölçeği (varsayılan 96 DPI = 1.0); API hatasında 1.0.
            let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
            let (scale_x, scale_y) = match unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) } {
                Ok(()) => (dpi_x as f64 / 96.0, dpi_y as f64 / 96.0),
                Err(_) => (1.0, 1.0),
            };

            // Kararlı kimlik: DisplayConfig yolu (çözünürlük, sıra ve aygıt adı değişse de aynı kalır). Okunamazsa Windows
            // aygıt adına düşülür.
            let config = configs.get(&text::to_upper_invariant(&device_name)).filter(|c| !c.device_path.is_empty());
            let id = config.map_or_else(|| device_name.clone(), |c| c.device_path.clone());
            let name = config.filter(|c| !c.friendly_name.is_empty()).map_or_else(|| fallback_name(&device_name), |c| c.friendly_name.clone());

            list.push(
                DisplayMonitor::new(id, name, rect(&mi.monitorInfo.rcMonitor), rect(&mi.monitorInfo.rcWork), is_primary)
                    .with_dpi(scale_x, scale_y)
                    .with_handle(hmonitor.0 as isize)
                    .with_device_name(device_name),
            );
        }
        list
    }

    fn get_primary_monitor(&self) -> Option<DisplayMonitor> {
        let monitors = self.get_monitors();
        monitors.iter().find(|m| m.is_primary).cloned().or_else(|| monitors.first().cloned())
    }

    fn get_monitor(&self, id_or_name: &str) -> Option<DisplayMonitor> {
        if id_or_name.trim().is_empty() {
            return self.get_primary_monitor();
        }
        self.get_monitors()
            .into_iter()
            .find(|m| text::eq_ignore_case(&m.id, id_or_name) || text::eq_ignore_case(&m.name, id_or_name))
            .or_else(|| self.get_primary_monitor())
    }

    fn start(&self) {
        {
            let st = self.lock();
            if st.started || st.disposed {
                return;
            }
        }
        let sig = signature(&self.get_monitors());
        let mut st = self.lock();
        st.started = true;
        st.last_signature = sig;
    }

    fn stop(&self) {
        self.lock().started = false;
    }

    fn notify_display_changed(&self) {
        {
            let st = self.lock();
            if !st.started || st.disposed {
                return;
            }
        }
        let sig = signature(&self.get_monitors());
        {
            let mut st = self.lock();
            if sig == st.last_signature {
                return;
            }
            st.last_signature = sig;
        }
        self.monitors_changed.invoke(&());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_a_primary_monitor() {
        let service = MonitorService::default();
        let monitors = service.get_monitors();
        assert!(!monitors.is_empty());
        let primary = service.get_primary_monitor().unwrap();
        assert!(primary.dpi_scale_x >= 1.0);
        assert_eq!(service.get_monitor("").map(|m| m.id), Some(primary.id.clone()));
        assert_eq!(fallback_name(r"\\.\DISPLAY12"), "Display 12");
    }
}

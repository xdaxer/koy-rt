//! Dock penceresini Windows AppBar olarak kaydeder, ekranın kenarında masaüstü çalışma alanı ayırır ve kapanışta
//! (ABM_REMOVE) eski çalışma alanını geri yükler.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use koy_core::display::WindowPlacementMath;
use koy_core::dock::{AppBarMath, AppBarOwnership, AppBarPlacementCache, DockEdge, IAppBarService};
use koy_core::shell::WindowRect;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows::Win32::UI::Shell::{APPBARDATA, SHAppBarMessage};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

use super::window_styles::hwnd;

const ABM_NEW: u32 = 0;
const ABM_REMOVE: u32 = 1;
const ABM_QUERYPOS: u32 = 2;
const ABM_SETPOS: u32 = 3;
const UNRESOLVED_RETRY_AFTER: Duration = Duration::from_secs(2);
const DEFAULT_HEIGHT_DIP: f64 = 46.0;

fn to_app_bar_edge(edge: DockEdge) -> u32 {
    match edge {
        DockEdge::Left => 0,
        DockEdge::Top => 1,
        DockEdge::Right => 2,
        DockEdge::Bottom => 3,
    }
}

fn to_rect(r: RECT) -> WindowRect {
    WindowRect { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
}

fn monitor_info(mon: HMONITOR) -> Option<MONITORINFO> {
    if mon.is_invalid() {
        return None;
    }
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: yapı boyutu ayarlandı.
    unsafe { GetMonitorInfoW(mon, &mut mi) }.as_bool().then_some(mi)
}

fn message(msg: u32, data: &mut APPBARDATA) -> usize {
    // SAFETY: yapı boyutu ayarlı, çağrı süresince geçerli.
    unsafe { SHAppBarMessage(msg, data) }
}

fn app_bar_data(h: HWND) -> APPBARDATA {
    APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, hWnd: h, ..Default::default() }
}

#[derive(Default)]
struct State {
    registered: HashSet<isize>,
    edges: HashMap<isize, DockEdge>,
    // Her pencere için en son uygulanan alan: aynı alan için SETPOS tekrarlanmaz.
    placements: AppBarPlacementCache,
    // Yeniden iddiadan sonra da uyuşmayan çalışma alanı kenarı (başka çubuk): aynı değer için kısa süre tekrar denenmez.
    unresolved_work_edge: HashMap<isize, (i32, Instant)>,
    owns_bottom_edge: bool,
    disposed: bool,
}

#[derive(Default)]
pub struct AppBarService {
    state: Mutex<State>,
}

impl AppBarService {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Windows görev çubuğu gizliyken true: alt kenar Koy'undur ve ABM_QUERYPOS'un Gezgin yüzünden yukarı itmesi kabul
    /// edilmez (AppBarMath).
    pub fn owns_bottom_edge(&self) -> bool {
        self.lock().owns_bottom_edge
    }

    pub fn set_owns_bottom_edge(&self, value: bool) {
        let mut st = self.lock();
        if st.owns_bottom_edge == value {
            return;
        }
        st.owns_bottom_edge = value;
        st.unresolved_work_edge.clear();
        // Sahiplik değişince eski alan geçersiz: sıradaki güncelleme mutlaka SETPOS gönderir.
        st.placements.clear();
    }

    /// Alan aynı olsa da yeniden iddia eder: Gezgin çalışma alanını yeniden hesapladıysa dock'un payı düşmüş olabilir.
    /// `only_if_needed`: monitörün çalışma alanı payı zaten karşılıyorsa SETPOS gönderilmez. SETPOS gönderildi mi döner.
    pub fn reassert(&self, window_handle: isize, height_dip: f64, dpi_scale: f64, only_if_needed: bool, edge: DockEdge) -> bool {
        if window_handle == 0 || !self.lock().registered.contains(&window_handle) {
            return false;
        }
        let h = hwnd(window_handle);
        if only_if_needed {
            // SAFETY: yalnız sorgu.
            let mon = unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) };
            if let Some(mi) = monitor_info(mon) {
                let monitor_rect = to_rect(mi.rcMonitor);
                let height_px = WindowPlacementMath::to_pixels(if height_dip <= 0.0 { DEFAULT_HEIGHT_DIP } else { height_dip }, dpi_scale);
                let (owns, unresolved) = {
                    let st = self.lock();
                    let unresolved = st.unresolved_work_edge.get(&window_handle).filter(|(_, t)| t.elapsed() < UNRESOLVED_RETRY_AFTER).map(|(c, _)| *c);
                    (st.owns_bottom_edge, unresolved)
                };
                if !AppBarOwnership::needs_reassert_with_edge(edge, monitor_rect, to_rect(mi.rcWork), height_px, owns, unresolved) {
                    return false;
                }
                self.update_position_internal(h, height_dip, dpi_scale, true, edge);

                // Düzelmediyse (başka bir çubuk payı etkiliyor) aynı çalışma alanı için tekrar denenmez.
                if let Some(after) = monitor_info(mon) {
                    let owns = self.owns_bottom_edge();
                    let work = after.rcWork;
                    let still_wrong = AppBarOwnership::needs_reassert_with_edge(edge, monitor_rect, to_rect(work), height_px, owns, None);
                    let mut st = self.lock();
                    if still_wrong {
                        let coord = match edge {
                            DockEdge::Top => work.top,
                            DockEdge::Left => work.left,
                            DockEdge::Right => work.right,
                            DockEdge::Bottom => work.bottom,
                        };
                        st.unresolved_work_edge.insert(window_handle, (coord, Instant::now()));
                    } else {
                        st.unresolved_work_edge.remove(&window_handle);
                    }
                }
                return true;
            }
        }
        self.update_position_internal(h, height_dip, dpi_scale, true, edge);
        true
    }

    fn update_position_internal(&self, h: HWND, height_dip: f64, dpi_scale: f64, force: bool, edge: DockEdge) {
        let handle = h.0 as isize;
        {
            let mut st = self.lock();
            if st.edges.get(&handle).is_some_and(|&prev| prev != edge) {
                drop(st);
                // Kenar değiştiğinde eski kenarın çalışma alanını geri vermek için ABM_REMOVE ve yeni kenarda ABM_NEW
                // zorunludur; aksi halde eski kenar payı Windows'ta asılı kalır.
                self.reregister(handle, height_dip, dpi_scale, edge);
                return;
            }
            st.edges.insert(handle, edge);
        }
        let height_dip = if height_dip <= 0.0 { DEFAULT_HEIGHT_DIP } else { height_dip };

        // SAFETY: yalnız sorgu.
        let mut screen = WindowRect { left: 0, top: 0, right: unsafe { GetSystemMetrics(SM_CXSCREEN) }, bottom: unsafe { GetSystemMetrics(SM_CYSCREEN) } };
        // SAFETY: yalnız sorgu.
        if let Some(mi) = monitor_info(unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST) }) {
            screen = to_rect(mi.rcMonitor);
        }

        // Pencere yüksekliğiyle aynı piksel kuralı: çubuk ile çalışma alanı arasında boşluk kalmaz.
        let px = WindowPlacementMath::to_pixels(height_dip, dpi_scale);
        let rc = match edge {
            DockEdge::Top => RECT { left: screen.left, top: screen.top, right: screen.right, bottom: screen.top + px },
            DockEdge::Left => RECT { left: screen.left, top: screen.top, right: screen.left + px, bottom: screen.bottom },
            DockEdge::Right => RECT { left: screen.right - px, top: screen.top, right: screen.right, bottom: screen.bottom },
            DockEdge::Bottom => RECT { left: screen.left, top: screen.bottom - px, right: screen.right, bottom: screen.bottom },
        };
        let mut data = app_bar_data(h);
        data.uEdge = to_app_bar_edge(edge);
        data.rc = rc;
        let requested = to_rect(rc);

        let owns = {
            let st = self.lock();
            // Alt kenar bizimken sonuç Gezgin'den bağımsızdır: aynı alan için tekrar SETPOS gerekmez.
            if !st.placements.should_send(handle, requested, st.owns_bottom_edge, force) {
                return;
            }
            st.owns_bottom_edge
        };
        message(ABM_QUERYPOS, &mut data);
        let final_rect = AppBarMath::final_rect_with_edge(edge, screen, px, to_rect(data.rc), owns);
        data.rc = RECT { left: final_rect.left, top: final_rect.top, right: final_rect.right, bottom: final_rect.bottom };
        message(ABM_SETPOS, &mut data);
        self.lock().placements.record(handle, requested);
    }

    pub fn dispose(&self) {
        let handles: Vec<isize> = {
            let mut st = self.lock();
            if st.disposed {
                return;
            }
            st.disposed = true;
            st.registered.iter().copied().collect()
        };
        for h in handles {
            self.unregister(h);
        }
    }
}

impl Drop for AppBarService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IAppBarService for AppBarService {
    fn register(&self, window_handle: isize, height_dip: f64, dpi_scale: f64, edge: DockEdge) {
        if window_handle == 0 || self.lock().registered.contains(&window_handle) {
            return;
        }
        let height_dip = if height_dip <= 0.0 { DEFAULT_HEIGHT_DIP } else { height_dip };
        let h = hwnd(window_handle);
        let mut data = app_bar_data(h);
        data.uCallbackMessage = 0x0400 + 100; // WM_USER + 100
        if message(ABM_NEW, &mut data) != 0 {
            {
                let mut st = self.lock();
                st.registered.insert(window_handle);
                st.edges.insert(window_handle, edge);
            }
            self.update_position_internal(h, height_dip, dpi_scale, true, edge);
        }
    }

    fn update_position(&self, window_handle: isize, height_dip: f64, dpi_scale: f64, edge: DockEdge) {
        if window_handle == 0 || !self.lock().registered.contains(&window_handle) {
            return;
        }
        self.update_position_internal(hwnd(window_handle), height_dip, dpi_scale, false, edge);
    }

    fn reregister(&self, window_handle: isize, height_dip: f64, dpi_scale: f64, edge: DockEdge) {
        if window_handle == 0 {
            return;
        }
        // Eski Explorer'ın kaydı yok olmuştur; ABM_REMOVE zararsızdır ve yerel kayıt kümesini de temizler.
        self.unregister(window_handle);
        self.register(window_handle, height_dip, dpi_scale, edge);
    }

    fn unregister(&self, window_handle: isize) {
        {
            let mut st = self.lock();
            if window_handle == 0 || !st.registered.remove(&window_handle) {
                return;
            }
            st.edges.remove(&window_handle);
            st.placements.forget(window_handle);
            st.unresolved_work_edge.remove(&window_handle);
        }
        let mut data = app_bar_data(hwnd(window_handle));
        message(ABM_REMOVE, &mut data);
    }
}

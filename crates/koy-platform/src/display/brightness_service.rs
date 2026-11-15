//! Parlaklık: çentiğin bulunduğu ekran dahiliyse WMI, harici ise DDC/CI (bkz. `BrightnessSourcePicker`). Kaynak kurulumda
//! bir kez değil, `refresh` her çağrıldığında yeniden belirlenir (ekran düzeni değişir). Monitör tanıtıcısı tutulmaz; her
//! çağrıda güncel ekrandan çözülür, kapanmış monitörün ölü tanıtıcısına yazılmaz. Ayar çağrıları arka plandadır ve
//! birleşir (`BrightnessWriteQueue`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use koy_core::display::{BrightnessSourceKind, BrightnessSourcePicker, BrightnessTarget, BrightnessWriteQueue, DisplayMonitor, IBrightnessService};
use koy_core::event::Event;
use koy_core::text;
use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR, PHYSICAL_MONITOR,
    SetMonitorBrightness,
};
use windows::Win32::Foundation::{HANDLE, POINT};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint, MonitorFromWindow};

use super::display_config_reader::DisplayConfigReader;
use crate::bluetooth::support::ComApartment;
use crate::wmi;

pub type MonitorProvider = Box<dyn Fn() -> Option<DisplayMonitor> + Send + Sync>;

struct State {
    source: BrightnessSourceKind,
    brightness: i32,
    ddc_min: i32,
    ddc_max: i32,
}

struct Inner {
    monitor_provider: Option<MonitorProvider>,
    queue: BrightnessWriteQueue,
    sync: Mutex<State>,
    disposed: AtomicBool,
    brightness_changed: Event<i32>,
    availability_changed: Event<()>,
}

pub struct BrightnessService {
    inner: Arc<Inner>,
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.sync.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn monitor(&self) -> Option<DisplayMonitor> {
        self.monitor_provider.as_ref().and_then(|p| p())
    }

    fn refresh(&self) {
        if self.disposed.load(Ordering::SeqCst) {
            return;
        }
        let monitor = self.monitor();
        let target = resolve_target(monitor.as_ref());

        let wmi_value = try_read_wmi();
        // Dahili hedefte WMI yeterliyse DDC/CI sorgusu (yüzlerce ms) atlanır.
        let ddc = if wmi_value.is_some() && target != BrightnessTarget::External { None } else { try_read_ddc(monitor.as_ref()) };
        let kind = BrightnessSourcePicker::pick(target, wmi_value.is_some(), ddc.is_some());

        {
            let mut st = self.lock();
            st.source = kind;
            match kind {
                BrightnessSourceKind::Wmi => st.brightness = wmi_value.unwrap_or(st.brightness),
                BrightnessSourceKind::Ddc => {
                    if let Some((value, min, max)) = ddc {
                        st.ddc_min = min;
                        st.ddc_max = max;
                        st.brightness = BrightnessSourcePicker::to_percent(value, min, max);
                    }
                }
                _ => {}
            }
        }
        self.availability_changed.invoke(&());
    }

    // Tek işçi: kuyrukta son değer kalana kadar döner. Çağrı sürerken gelen değerler birleşir.
    fn apply_loop(&self) {
        let _com = ComApartment::enter_mta();
        let mut failed = false;
        while !self.disposed.load(Ordering::SeqCst) {
            let Some(value) = self.queue.try_take() else { break };
            if !self.apply(value) {
                failed = true;
            }
        }
        // Yazım başarısız olduysa (ekran gitmiş, sürücü reddetti) kaynak yeniden belirlenir; artık yol yoksa kaydırıcı
        // devre dışı kalır.
        if failed && !self.disposed.load(Ordering::SeqCst) {
            self.refresh();
        }
    }

    fn apply(&self, percent: i32) -> bool {
        let (kind, min, max) = {
            let st = self.lock();
            (st.source, st.ddc_min, st.ddc_max)
        };
        let ok = match kind {
            BrightnessSourceKind::Wmi => try_set_wmi(percent),
            BrightnessSourceKind::Ddc => try_set_ddc(self.monitor().as_ref(), BrightnessSourcePicker::from_percent(percent, min, max)),
            _ => false,
        };
        if ok {
            self.lock().brightness = percent;
        }
        ok
    }
}

fn resolve_target(monitor: Option<&DisplayMonitor>) -> BrightnessTarget {
    let Some(monitor) = monitor.filter(|m| !m.device_name.is_empty()) else { return BrightnessTarget::Unknown };
    match DisplayConfigReader::read().get(&text::to_upper_invariant(&monitor.device_name)) {
        None => BrightnessTarget::Unknown,
        Some(e) if e.is_internal => BrightnessTarget::Internal,
        Some(_) => BrightnessTarget::External,
    }
}

fn try_read_wmi() -> Option<i32> {
    // WMI desteklenmiyor ya da masaüstü PC: None.
    let services = wmi::connect(r"root\wmi").ok()?;
    let rows = wmi::query(&services, "SELECT CurrentBrightness FROM WmiMonitorBrightness WHERE Active = TRUE").ok()?;
    rows.iter().find_map(|r| wmi::get_i64(r, "CurrentBrightness")).map(|v| v as i32)
}

fn try_set_wmi(percent: i32) -> bool {
    let Ok(services) = wmi::connect(r"root\wmi") else { return false };
    let Ok(rows) = wmi::query(&services, "SELECT * FROM WmiMonitorBrightnessMethods WHERE Active = TRUE") else { return false };
    let Some(first) = rows.first() else { return false };
    wmi::invoke_method(
        &services,
        first,
        "WmiMonitorBrightnessMethods",
        "WmiSetBrightness",
        &[("Timeout", wmi::MethodArg::U32(1)), ("Brightness", wmi::MethodArg::U8(percent.clamp(0, 100) as u8))],
    )
    .is_ok()
}

/// Verilen ekranın (yoksa birincil ekranın) ilk fiziksel monitörü üzerinde işlem yapar. Tanıtıcı her seferinde güncel
/// ekrandan alınır ve hemen bırakılır.
fn with_physical_monitor<T>(monitor: Option<&DisplayMonitor>, action: impl FnOnce(HANDLE) -> Option<T>) -> Option<T> {
    // SAFETY: fiziksel monitör dizisi doğru boyutta ayrılır ve her yolda yok edilir.
    unsafe {
        let hmon = match monitor {
            Some(m) => {
                let b = &m.bounds;
                MonitorFromPoint(POINT { x: (b.left + b.right) / 2, y: (b.top + b.bottom) / 2 }, MONITOR_DEFAULTTONEAREST)
            }
            None => MonitorFromWindow(Default::default(), MONITOR_DEFAULTTOPRIMARY),
        };
        if hmon.is_invalid() {
            return None;
        }
        let mut count = 0u32;
        GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count).ok()?;
        if count == 0 {
            return None;
        }
        let mut monitors = vec![PHYSICAL_MONITOR::default(); count as usize];
        GetPhysicalMonitorsFromHMONITOR(hmon, &mut monitors).ok()?;
        let result = action(monitors[0].hPhysicalMonitor);
        let _ = DestroyPhysicalMonitors(&monitors);
        result
    }
}

fn try_read_ddc(monitor: Option<&DisplayMonitor>) -> Option<(i32, i32, i32)> {
    with_physical_monitor(monitor, |handle| {
        let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
        // SAFETY: fiziksel monitör tanıtıcısı geçerli.
        (unsafe { GetMonitorBrightness(handle, &mut min, &mut cur, &mut max) } != 0).then_some((cur as i32, min as i32, max as i32))
    })
}

fn try_set_ddc(monitor: Option<&DisplayMonitor>, raw: i32) -> bool {
    // SAFETY: fiziksel monitör tanıtıcısı geçerli.
    with_physical_monitor(monitor, |handle| (unsafe { SetMonitorBrightness(handle, raw as u32) } != 0).then_some(())).is_some()
}

/// Dış değişiklikleri (Fn tuşları, Windows ayarları) WMI olayıyla izler. Yarı eşzamanlı bekleme kendi iş parçacığında
/// engelleyerek yapılır; servis bırakılınca en geç bekleme süresi sonunda biter.
fn start_watching(weak: std::sync::Weak<Inner>) {
    std::thread::Builder::new()
        .name("Koy.BrightnessWatch".into())
        .spawn(move || {
            let _com = ComApartment::enter_mta();
            // WMI olayı kurulamazsa yalnız Koy içi değişiklikler bildirilir.
            let Ok(services) = wmi::connect(r"root\wmi") else { return };
            let Ok(events) = wmi::notification_query(&services, "SELECT * FROM WmiMonitorBrightnessEvent") else { return };
            loop {
                let Some(inner) = weak.upgrade() else { return };
                if inner.disposed.load(Ordering::SeqCst) {
                    return;
                }
                drop(inner);
                let Some(event) = wmi::next_event(&events, 2000) else { continue };
                let Some(inner) = weak.upgrade() else { return };
                // Bozuk olay verisi göstergeyi düşürmesin.
                let Some(value) = wmi::get_i64(&event, "Brightness").map(|v| v as i32) else { continue };
                {
                    let mut st = inner.lock();
                    if value == st.brightness {
                        continue;
                    }
                    st.brightness = value;
                }
                inner.brightness_changed.invoke(&value);
            }
        })
        .ok();
}

impl BrightnessService {
    /// `watch_external_changes`: parlaklık Koy dışından değişince de `brightness_changed` yükselir (yalnız WMI/dahili).
    /// `monitor_provider`: parlaklığı ayarlanacak ekranı o anki haliyle verir (çentiğin bulunduğu ekran). `None` ise ekran
    /// bilinmez: önce WMI, yoksa birincil ekranda DDC/CI. WMI/DDC sorgusu yapar: arayüz iş parçacığında kurulmaz.
    pub fn new(watch_external_changes: bool, monitor_provider: Option<MonitorProvider>) -> Self {
        let inner = Arc::new(Inner {
            monitor_provider,
            queue: BrightnessWriteQueue::new(),
            sync: Mutex::new(State { source: BrightnessSourceKind::None, brightness: 50, ddc_min: 0, ddc_max: 100 }),
            disposed: AtomicBool::new(false),
            brightness_changed: Event::new(),
            availability_changed: Event::new(),
        });
        {
            let _com = ComApartment::enter_mta();
            inner.refresh();
        }
        if watch_external_changes && inner.lock().source == BrightnessSourceKind::Wmi {
            start_watching(Arc::downgrade(&inner));
        }
        BrightnessService { inner }
    }

    pub fn dispose(&self) {
        self.inner.disposed.store(true, Ordering::SeqCst);
    }
}

impl Drop for BrightnessService {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl IBrightnessService for BrightnessService {
    fn is_supported(&self) -> bool {
        self.source() != BrightnessSourceKind::None
    }

    fn brightness(&self) -> i32 {
        self.inner.lock().brightness
    }

    fn source(&self) -> BrightnessSourceKind {
        self.inner.lock().source
    }

    fn brightness_changed(&self) -> &Event<i32> {
        &self.inner.brightness_changed
    }

    fn availability_changed(&self) -> &Event<()> {
        &self.inner.availability_changed
    }

    fn set_brightness(&self, brightness: i32) {
        if self.inner.disposed.load(Ordering::SeqCst) || !self.is_supported() {
            return;
        }
        if self.inner.queue.post(brightness) {
            let inner = Arc::clone(&self.inner);
            std::thread::spawn(move || inner.apply_loop());
        }
    }

    fn refresh(&self) {
        let _com = ComApartment::enter_mta();
        self.inner.refresh();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determines_a_source_without_changing_brightness() {
        // Yalnız okuma: kaynak seçimi ve geçerli değer (sistem parlaklığı değiştirilmez).
        let service = BrightnessService::new(false, None);
        let b = service.brightness();
        assert!((0..=100).contains(&b));
        let _ = service.source();
    }
}

//! Karşılığı: `koy/src/App/Features/Settings/DisplaysViewModel.cs` ve `IdentifyOverlayWindow.cs`.
//!
//! Ekranlar sayfası: gerçek monitör yerleşimini çizer, seçili ekranın çentik/dock ayarını ve "yeni ekran" varsayılanını
//! ayar dosyasına yazar, ekran takılıp çıkınca kendini günceller. Yalnız sayfa görünürken (activate/deactivate) sistem
//! olaylarını dinler.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use koy_core::display::{DisplayMonitor, DisplaySchema, IMonitorService, MonitorPolicy, NewScreenMode};
use koy_core::event::Subscription;
use koy_platform::display::MonitorService;
use koy_platform::shell::WindowStyles;
use slint::ComponentHandle;

use crate::infra::observable::Notifier;
use crate::infra::{dispatch, native_window, ui_ref::UiRef};
use crate::resources::strings::Strings;
use crate::resources::theme_manager::ThemeManager;

use super::settings_view_model::SettingsViewModel;

// Şema alanı: içerik sütunu ~640 DIP, kutu yüksekliği tasarımdaki gibi 132 DIP'i geçmez.
const SCHEMA_MAX_WIDTH: f64 = 560.0;
const SCHEMA_MAX_HEIGHT: f64 = 132.0;
const SCHEMA_GAP: f64 = 8.0;

/// Şemada tek monitör kartı.
#[derive(Clone, Debug)]
pub struct MonitorCard {
    pub monitor: DisplayMonitor,
    pub number: i32,
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub is_selected: bool,
    pub shows_notch: bool,
    pub shows_dock: bool,
}

#[derive(Default)]
struct State {
    monitors: Vec<DisplayMonitor>,
    cards: Vec<MonitorCard>,
    schema_width: f64,
    schema_height: f64,
    selected_id: Option<String>,
    selected_header: String,
    selected_name: String,
    selected_details: String,
    selected_is_primary: bool,
    notch_on: bool,
    dock_on: bool,
    new_screen_mode: NewScreenMode,
}

pub struct DisplaysViewModel {
    vm: Rc<SettingsViewModel>,
    st: RefCell<State>,
    service: RefCell<Option<(Arc<MonitorService>, Subscription)>>,
    active: Cell<bool>,
    pub changed: Notifier,
}

impl DisplaysViewModel {
    pub fn new(vm: Rc<SettingsViewModel>) -> Rc<Self> {
        Rc::new(DisplaysViewModel {
            vm,
            st: RefCell::new(State::default()),
            service: RefCell::new(None),
            active: Cell::new(false),
            changed: Notifier::default(),
        })
    }

    /// Sayfa görünür oldu: ekranları oku ve takma/çıkarmayı dinlemeye başla.
    pub fn activate(self: &Rc<Self>) {
        if !self.active.replace(true) {
            let service = Arc::new(MonitorService::default());
            service.start();
            let ui = UiRef::new(self);
            let sub = service.monitors_changed().subscribe(move |_| {
                // Sistem olayı arayüz dışı iş parçacığından gelir.
                dispatch::post(move || {
                    if let Some(me) = ui.upgrade() {
                        me.refresh();
                    }
                });
            });
            *self.service.borrow_mut() = Some((service, sub));
        }
        self.refresh();
    }

    /// Sayfa kapandı: dinlemeyi bırak.
    pub fn deactivate(&self) {
        if !self.active.replace(false) {
            return;
        }
        if let Some((service, sub)) = self.service.borrow_mut().take() {
            service.monitors_changed().unsubscribe(sub);
            service.dispose();
        }
    }

    pub fn cards(&self) -> Vec<MonitorCard> {
        self.st.borrow().cards.clone()
    }

    pub fn schema_size(&self) -> (f64, f64) {
        let st = self.st.borrow();
        (st.schema_width, st.schema_height)
    }

    pub fn selected(&self) -> Option<(String, String, String, bool, bool, bool)> {
        let st = self.st.borrow();
        st.selected_id.as_ref()?;
        Some((
            st.selected_header.clone(),
            st.selected_name.clone(),
            st.selected_details.clone(),
            st.selected_is_primary,
            st.notch_on,
            st.dock_on,
        ))
    }

    pub fn new_screen_mode(&self) -> NewScreenMode {
        self.st.borrow().new_screen_mode
    }

    fn read_monitors(&self) -> Vec<DisplayMonitor> {
        match self.service.borrow().as_ref() {
            Some((s, _)) => s.get_monitors(),
            None => {
                let s = MonitorService::default();
                let m = s.get_monitors();
                s.dispose();
                m
            }
        }
    }

    /// Bağlı ekranları ve kayıtlı ayarları yeniden okuyup şemayı kurar.
    pub fn refresh(&self) {
        let monitors = self.read_monitors();
        let settings = self.vm.store().load();
        let layout = DisplaySchema::compute(&monitors, SCHEMA_MAX_WIDTH, SCHEMA_MAX_HEIGHT, SCHEMA_GAP);
        let assignments = MonitorPolicy::resolve(Some(&settings.displays.monitors), settings.displays.new_screen_default, &monitors);
        let cards: Vec<MonitorCard> = layout
            .tiles
            .iter()
            .filter_map(|tile| {
                let monitor = monitors.iter().find(|m| m.id.eq_ignore_ascii_case(&tile.id))?.clone();
                let a = assignments.iter().find(|a| a.monitor.id.eq_ignore_ascii_case(&tile.id))?;
                Some(MonitorCard {
                    monitor,
                    number: tile.number,
                    left: tile.left,
                    top: tile.top,
                    width: tile.width,
                    height: tile.height,
                    is_selected: false,
                    shows_notch: a.notch_enabled,
                    shows_dock: a.dock_enabled,
                })
            })
            .collect();
        let selected = {
            let mut st = self.st.borrow_mut();
            st.monitors = monitors;
            st.new_screen_mode = settings.displays.new_screen_default;
            st.schema_width = layout.width;
            st.schema_height = layout.height;
            st.cards = cards;
            // Seçim ekran değişse de korunur; seçili ekran çıktıysa ana ekrana dönülür.
            let by_id = st
                .selected_id
                .as_ref()
                .and_then(|id| st.cards.iter().position(|c| c.monitor.id.eq_ignore_ascii_case(id)));
            by_id.or_else(|| st.cards.iter().position(|c| c.monitor.is_primary)).or(if st.cards.is_empty() { None } else { Some(0) })
        };
        self.select(selected);
    }

    pub fn select(&self, index: Option<usize>) {
        let settings = self.vm.store().load();
        {
            let mut st = self.st.borrow_mut();
            for (i, c) in st.cards.iter_mut().enumerate() {
                c.is_selected = Some(i) == index;
            }
            let card = index.and_then(|i| st.cards.get(i)).cloned();
            st.selected_id = card.as_ref().map(|c| c.monitor.id.clone());
            match card {
                None => {
                    st.selected_header.clear();
                    st.selected_name.clear();
                    st.selected_details.clear();
                    st.selected_is_primary = false;
                }
                Some(card) => {
                    let a = &MonitorPolicy::resolve(
                        Some(&settings.displays.monitors),
                        settings.displays.new_screen_default,
                        std::slice::from_ref(&card.monitor),
                    )[0];
                    st.notch_on = a.notch_enabled;
                    st.dock_on = a.dock_enabled;
                    let m = &card.monitor;
                    st.selected_name = m.name.clone();
                    st.selected_is_primary = m.is_primary;
                    st.selected_header = Strings::format("DisplayHeaderFormat", &[&card.number.to_string(), &m.name.to_uppercase()]);
                    st.selected_details = Strings::format(
                        "DisplayDetailsFormat",
                        &[&m.pixel_width().to_string(), &m.pixel_height().to_string(), &((m.dpi_scale_x * 100.0).round() as i32).to_string()],
                    );
                }
            }
        }
        self.changed.notify();
    }

    fn selected_monitor(&self) -> Option<DisplayMonitor> {
        let st = self.st.borrow();
        let id = st.selected_id.as_ref()?;
        st.cards.iter().find(|c| &c.monitor.id == id).map(|c| c.monitor.clone())
    }

    pub fn set_notch_on(&self, value: bool) {
        let Some(monitor) = self.selected_monitor() else { return };
        let mut current = self.vm.store().load();
        current.displays.monitors = MonitorPolicy::update(
            Some(&current.displays.monitors),
            current.displays.new_screen_default,
            &monitor,
            |mut s| {
                s.notch_enabled = value;
                s
            },
        );
        {
            let mut st = self.st.borrow_mut();
            st.notch_on = value;
            if let Some(c) = st.cards.iter_mut().find(|c| c.monitor.id == monitor.id) {
                c.shows_notch = value;
            }
        }
        self.vm.commit_external("Displays.NotchOn", current);
        self.changed.notify();
    }

    pub fn set_dock_on(&self, value: bool) {
        let Some(monitor) = self.selected_monitor() else { return };
        let mut current = self.vm.store().load();
        current.displays.monitors = MonitorPolicy::update(
            Some(&current.displays.monitors),
            current.displays.new_screen_default,
            &monitor,
            |mut s| {
                s.dock_enabled = value;
                s
            },
        );
        {
            let mut st = self.st.borrow_mut();
            st.dock_on = value;
            if let Some(c) = st.cards.iter_mut().find(|c| c.monitor.id == monitor.id) {
                c.shows_dock = value;
            }
        }
        self.vm.commit_external("Displays.DockOn", current);
        self.changed.notify();
    }

    /// Çalışan özellikler ayarı kurulurken okur; kayıtsız ekran varsa yeni varsayılan hemen uygulanır.
    pub fn set_new_screen_mode(&self, mode: NewScreenMode) {
        if self.st.borrow().new_screen_mode == mode {
            return;
        }
        self.st.borrow_mut().new_screen_mode = mode;
        let mut current = self.vm.store().load();
        current.displays.new_screen_default = mode;
        self.vm.commit_external("Displays.NewScreenDefault", current);
        self.refresh();
    }

    /// "Ekranları tanımla": her monitörde 2 saniye ekran numarası. Güncel liste okunur (ekran yeni takılmış olabilir).
    pub fn identify(&self) {
        let s = MonitorService::default();
        let monitors = s.get_monitors();
        s.dispose();
        IdentifyOverlay::show_on(&monitors);
    }
}

// ---------------------------------------------------------------- tanımlama penceresi

const OVERLAY_SIZE_DIP: f64 = 180.0;
const OVERLAY_LIFETIME: Duration = Duration::from_secs(2);

thread_local! {
    static OPEN: RefCell<Vec<crate::IdentifyOverlayWindow>> = const { RefCell::new(Vec::new()) };
}

struct IdentifyOverlay;

impl IdentifyOverlay {
    /// Bütün monitörlerde numaraları gösterir; önceki tanımlama açıksa kapatır.
    fn show_on(monitors: &[DisplayMonitor]) {
        Self::close_all();
        let ordered = DisplaySchema::order(monitors);
        for (i, m) in ordered.iter().enumerate() {
            let Ok(w) = crate::IdentifyOverlayWindow::new() else { continue };
            ThemeManager::install(&w);
            w.set_number((i + 1) as i32);
            let size = (OVERLAY_SIZE_DIP * m.dpi_scale_x).round() as i32;
            let b = m.bounds;
            w.window().set_size(slint::PhysicalSize::new(size as u32, size as u32));
            w.window().set_position(slint::PhysicalPosition::new(b.left + (b.width() - size) / 2, b.top + (b.height() - size) / 2));
            if w.show().is_err() {
                continue;
            }
            style_when_ready(w.as_weak(), size, (28.0 * m.dpi_scale_x).round() as i32, 0);
            OPEN.with(|o| o.borrow_mut().push(w));
        }
        slint::Timer::single_shot(OVERLAY_LIFETIME, Self::close_all);
    }

    fn close_all() {
        let open: Vec<_> = OPEN.with(|o| o.borrow_mut().drain(..).collect());
        for w in open {
            let _ = w.hide();
        }
    }
}

/// Yerel pencere olay döngüsü dönünce oluşur: araç penceresi, etkinleşmez, tıklama geçirir, köşe bölgesi.
fn style_when_ready(w: slint::Weak<crate::IdentifyOverlayWindow>, size: i32, radius: i32, attempt: u32) {
    let Some(win) = w.upgrade() else { return };
    let hwnd = native_window::hwnd_of(win.window());
    if hwnd != 0 {
        crate::infra::native_window::make_popup(hwnd);
        WindowStyles::apply_tool_window_and_no_activate(hwnd);
        WindowStyles::set_click_through(hwnd, true);
        WindowStyles::reassert_topmost(hwnd);
        crate::features::dock::dock_popups::rounded_region(hwnd, size, size, radius);
        return;
    }
    if attempt < 100 {
        slint::Timer::single_shot(Duration::from_millis(10), move || style_when_ready(w, size, radius, attempt + 1));
    }
}

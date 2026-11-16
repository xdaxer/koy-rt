//! Sağ alt (tepsi) menüsü: çentik / dock anahtarları, Koy penceresi, 1 saat duraklat, güncelleme ve çıkış.
//! Karşılığı: `SagAltMenuWindow.xaml.cs` + `SagAltMenuViewModel.cs`. Tek örnek; kapatmak yerine gizlenir, etkinliğini
//! kaybedince kendiliğinden gizlenir.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use koy_core::appearance::FontScalePolicy;
use koy_core::display::{DisplayMonitor, IMonitorService};
use koy_core::settings::SettingsStore;
use koy_platform::display::MonitorService;
use slint::ComponentHandle;

use crate::infra::native_window;
use crate::resources::strings::Strings;
use crate::resources::theme_manager::ThemeManager;
use crate::resources::window_frame::WindowFrame;
use crate::update_status::UpdateStatus;
use crate::{MenuText, SagAltMenuWindow as View};

/// Menünün uygulamaya ilettiği eylemler (C#: `Application.Current as App`, `KoyWindow` statikleri).
pub struct MenuActions {
    pub notch_toggled: Box<dyn Fn(bool)>,
    pub dock_toggled: Box<dyn Fn(bool)>,
    pub pause_toggled: Box<dyn Fn(bool)>,
    pub is_paused: Box<dyn Fn() -> bool>,
    pub open_koy_window: Box<dyn Fn()>,
    pub open_update: Box<dyn Fn()>,
    pub exit: Box<dyn Fn()>,
}

struct Menu {
    view: View,
    actions: MenuActions,
    store: SettingsStore,
    is_paused: Cell<bool>,
    is_exit_confirming: Cell<bool>,
    hwnd: Cell<isize>,
    alive: Rc<Cell<bool>>,
}

thread_local! {
    static INSTANCE: RefCell<Option<Rc<Menu>>> = const { RefCell::new(None) };
    static ACTIONS: RefCell<Option<MenuActions>> = const { RefCell::new(None) };
}

pub struct SagAltMenuWindow;

impl SagAltMenuWindow {
    /// Uygulama eylemlerini bağlar (ilk gösterimden önce bir kez).
    pub fn set_actions(actions: MenuActions) {
        ACTIONS.with(|a| *a.borrow_mut() = Some(actions));
    }

    /// Örnek bellekte oluşturuldu mu.
    pub fn is_created() -> bool {
        INSTANCE.with(|i| i.borrow().is_some())
    }

    fn instance() -> Option<Rc<Menu>> {
        if let Some(m) = INSTANCE.with(|i| i.borrow().clone()) {
            return Some(m);
        }
        let actions = ACTIONS.with(|a| a.borrow_mut().take())?;
        let menu = Menu::create(actions)?;
        INSTANCE.with(|i| *i.borrow_mut() = Some(Rc::clone(&menu)));
        Some(menu)
    }

    pub fn toggle() {
        let Some(menu) = Self::instance() else { return };
        if native_window::is_visible(menu.hwnd.get()) {
            menu.hide();
        } else {
            menu.show_menu();
        }
    }

    /// Örnek oluşturulmuşsa menüyü gizler; yoksa gereksiz yere pencere kurmaz.
    pub fn hide_if_open() {
        if let Some(m) = INSTANCE.with(|i| i.borrow().clone()) {
            m.hide();
        }
    }

    /// Harici kaynaklardan (kısayollar vb.) değişen durumları menüye iletir.
    pub fn sync_feature_states(notch_enabled: Option<bool>, dock_enabled: Option<bool>) {
        let Some(m) = INSTANCE.with(|i| i.borrow().clone()) else { return };
        if let Some(n) = notch_enabled {
            m.view.set_notch_enabled(n);
        }
        if let Some(d) = dock_enabled {
            m.view.set_dock_enabled(d);
        }
    }
}

fn set_texts(view: &View) {
    let t = view.global::<MenuText>();
    t.set_title(Strings::get("MenuWindowTitle").into());
    t.set_nav_notch(Strings::get("NavNotch").into());
    t.set_nav_dock(Strings::get("NavDock").into());
    t.set_on(Strings::get("On").into());
    t.set_off(Strings::get("Off").into());
    t.set_open_koy_window(Strings::get("OpenKoyWindowAction").into());
}

impl Menu {
    fn create(actions: MenuActions) -> Option<Rc<Menu>> {
        let view = View::new().ok()?;
        ThemeManager::install(&view);
        set_texts(&view);
        let menu = Rc::new(Menu {
            view,
            actions,
            store: SettingsStore::new(SettingsStore::default_path()),
            is_paused: Cell::new(false),
            is_exit_confirming: Cell::new(false),
            hwnd: Cell::new(0),
            alive: Rc::new(Cell::new(true)),
        });
        let weak = Rc::downgrade(&menu);
        let bind = |f: fn(&Rc<Menu>)| {
            let weak = weak.clone();
            move || {
                if let Some(m) = weak.upgrade() {
                    f(&m);
                }
            }
        };
        menu.view.on_toggle_notch(bind(Menu::toggle_notch));
        menu.view.on_toggle_dock(bind(Menu::toggle_dock));
        menu.view.on_open_koy_window(bind(Menu::open_koy_window));
        menu.view.on_pause_one_hour(bind(Menu::pause_one_hour));
        menu.view.on_open_update(bind(Menu::open_update));
        menu.view.on_exit(bind(Menu::exit));
        // Güncelleme durumu değişince menü yenilenir.
        let w = weak.clone();
        UpdateStatus::on_changed(move || {
            if let Some(m) = w.upgrade() {
                m.refresh();
            }
        });
        menu.refresh();
        Some(menu)
    }

    fn refresh(&self) {
        self.is_exit_confirming.set(false);
        let settings = self.store.try_load().unwrap_or_default();
        self.view.set_notch_enabled(settings.notch.enabled);
        self.view.set_dock_enabled(settings.dock.enabled);
        // Duraklatma süresi dolunca uygulama kendiliğinden devam eder; menü gerçek durumu göstermeli.
        self.is_paused.set((self.actions.is_paused)());
        let count = MonitorService::default().get_monitors().len().max(1);
        self.view.set_monitor_summary(Strings::format("MonitorSummary", &[&count.to_string()]).into());
        self.view.set_pause_text(Strings::get(if self.is_paused.get() { "ResumePauseAction" } else { "PauseOneHourAction" }).into());
        self.update_exit_text();
        self.view.set_has_available_update(UpdateStatus::has_available_update());
        self.view.set_available_version_text(
            UpdateStatus::available_version().map(|v| Strings::format("UpdateAvailableBadge", &[&v])).unwrap_or_default().into(),
        );
        set_texts(&self.view);
    }

    fn update_exit_text(&self) {
        self.view.set_exit_text(Strings::get(if self.is_exit_confirming.get() { "ExitKoyConfirm" } else { "ExitKoyAction" }).into());
    }

    fn ensure_native(self: &Rc<Self>) {
        if self.hwnd.get() != 0 {
            return;
        }
        let hwnd = native_window::hwnd_of(self.view.window());
        if hwnd == 0 {
            return;
        }
        self.hwnd.set(hwnd);
        native_window::set_tool_window(hwnd);
        native_window::extend_frame(hwnd);
        WindowFrame::attach(Rc::new(move || hwnd), Rc::clone(&self.alive));
        // Etkinliğini kaybedince gizlenir (C# Window_Deactivated).
        let weak = Rc::downgrade(self);
        native_window::add_hook(
            hwnd,
            Box::new(move |msg, wparam, _| {
                if native_window::is_deactivate(msg, wparam)
                    && let Some(m) = weak.upgrade()
                {
                    crate::infra::dispatch::post_local(move || m.hide());
                }
                None
            }),
        );
    }

    fn show_menu(self: &Rc<Self>) {
        self.refresh();
        let _ = self.view.show();
        self.ensure_native();
        self.position_near_cursor_or_bottom_right();
        native_window::activate(self.hwnd.get());
    }

    fn hide(&self) {
        let _ = self.view.hide();
    }

    fn position_near_cursor_or_bottom_right(&self) {
        // 1. Farenin fiziksel piksel konumu; 2. farenin bulunduğu monitör.
        let cursor = native_window::cursor_pos();
        let monitors = MonitorService::default().get_monitors();
        let target: Option<&DisplayMonitor> = cursor
            .and_then(|(cx, cy)| monitors.iter().find(|m| cx >= m.bounds.left && cx <= m.bounds.right && cy >= m.bounds.top && cy <= m.bounds.bottom))
            .or_else(|| monitors.iter().find(|m| m.is_primary))
            .or_else(|| monitors.first());

        // 3. Monitör DPI ve çalışma alanı (DIP).
        let dpi_x = target.map_or(1.0, |m| if m.dpi_scale_x > 0.0 { m.dpi_scale_x } else { 1.0 });
        let dpi_y = target.map_or(1.0, |m| if m.dpi_scale_y > 0.0 { m.dpi_scale_y } else { 1.0 });
        let (work_left, work_right, work_top, work_bottom) = match target {
            Some(m) => (m.work_area.left as f64 / dpi_x, m.work_area.right as f64 / dpi_x, m.work_area.top as f64 / dpi_y, m.work_area.bottom as f64 / dpi_y),
            None => (0.0, 1280.0, 0.0, 720.0),
        };

        // 4. Pencere boyutu. Koy ölçeği (UiScalePercent): içerik ölçeklenir, pencere genişliği ona uyar.
        let scale = FontScalePolicy::clamp_ui_scale(
            ThemeManager::last_settings().map_or(FontScalePolicy::DEFAULT_UI_SCALE, |s| s.appearance.ui_scale_percent),
        ) as f64
            / 100.0;
        self.view.set_ui_scale(scale as f32);
        let menu_width = 320.0 * scale;
        let menu_height = {
            let h = self.view.window().size().height as f64 / self.view.window().scale_factor() as f64;
            if h > 100.0 { h } else { 350.0 * scale }
        };

        // 5. Hedef konum: imleç varsa sağ kenarı imlece yakın, üstünde (sığmazsa altında); yoksa sağ alt köşe.
        let (mut left, mut top) = match (cursor, target) {
            (Some((cx, cy)), Some(_)) => {
                let (mx, my) = (cx as f64 / dpi_x, cy as f64 / dpi_y);
                let mut t = my - menu_height - 10.0;
                if t < work_top + 8.0 {
                    t = my + 10.0;
                }
                (mx - menu_width + 24.0, t)
            }
            _ => (work_right - menu_width - 16.0, work_bottom - menu_height - 16.0),
        };

        // 6. Monitör sınırları içine sabitle: asla diğer ekrana taşmaz.
        left = left.clamp(work_left + 8.0, (work_right - menu_width - 8.0).max(work_left + 8.0));
        top = top.clamp(work_top + 8.0, (work_bottom - menu_height - 8.0).max(work_top + 8.0));
        self.view.window().set_position(slint::PhysicalPosition::new((left * dpi_x).round() as i32, (top * dpi_y).round() as i32));
    }

    fn toggle_notch(self: &Rc<Self>) {
        let mut settings = self.store.try_load().unwrap_or_default();
        let enabled = !settings.notch.enabled;
        settings.notch.enabled = enabled;
        let _ = self.store.save(&settings);
        self.view.set_notch_enabled(enabled);
        (self.actions.notch_toggled)(enabled);
    }

    fn toggle_dock(self: &Rc<Self>) {
        let mut settings = self.store.try_load().unwrap_or_default();
        let enabled = !settings.dock.enabled;
        settings.dock.enabled = enabled;
        let _ = self.store.save(&settings);
        self.view.set_dock_enabled(enabled);
        (self.actions.dock_toggled)(enabled);
    }

    fn open_koy_window(self: &Rc<Self>) {
        self.hide();
        (self.actions.open_koy_window)();
    }

    fn open_update(self: &Rc<Self>) {
        self.hide();
        (self.actions.open_update)();
    }

    fn pause_one_hour(self: &Rc<Self>) {
        let paused = !self.is_paused.get();
        self.is_paused.set(paused);
        self.view.set_pause_text(Strings::get(if paused { "ResumePauseAction" } else { "PauseOneHourAction" }).into());
        (self.actions.pause_toggled)(paused);
        self.hide();
    }

    /// Çıkış tek tıkla olmaz: ilk tıklama düğmeyi "Çıkışı onayla"ya çevirir, menü kapanınca sıfırlanır.
    fn exit(self: &Rc<Self>) {
        if !self.is_exit_confirming.get() {
            self.is_exit_confirming.set(true);
            self.update_exit_text();
            return;
        }
        (self.actions.exit)();
    }
}

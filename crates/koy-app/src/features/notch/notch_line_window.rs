//! Ekranın üst kenarı boyunca ince çizgi. Karşılığı: `Features/Notch/NotchLineWindow.cs`.
//! Tıklama geçirir, odak çalmaz; çentik penceresinin hemen altında durur.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use slint::ComponentHandle;

use koy_core::display::DisplayMonitor;
use koy_core::notch::{NotchEdgeLine, NotchEdgeLineColor};
use koy_platform::shell::WindowStyles;

use crate::NotchLineWindow as LineView;
use crate::infra::native_window;
use crate::resources::theme_manager::ThemeManager;

pub struct NotchLineWindow {
    view: LineView,
    thickness: i32,
    monitor: RefCell<DisplayMonitor>,
    hwnd: Cell<isize>,
    visible: Cell<bool>,
}

impl NotchLineWindow {
    pub fn new(monitor: DisplayMonitor, color: NotchEdgeLineColor, thickness_dip: i32) -> Option<Rc<Self>> {
        let view = LineView::new().ok()?;
        ThemeManager::install(&view);
        view.set_line(slint::Color::from_argb_encoded(ThemeManager::color(NotchEdgeLine::resource_key(color))));
        Some(Rc::new(Self {
            view,
            thickness: NotchEdgeLine::clamp_thickness(thickness_dip),
            monitor: RefCell::new(monitor),
            hwnd: Cell::new(0),
            visible: Cell::new(false),
        }))
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd.get()
    }

    pub fn show(self: &Rc<Self>) {
        if self.view.show().is_err() {
            return;
        }
        self.visible.set(true);
        self.attach(0);
    }

    fn attach(self: &Rc<Self>, attempt: u32) {
        let hwnd = native_window::hwnd_of(self.view.window());
        if hwnd == 0 {
            if attempt < 200 {
                let w = Rc::downgrade(self);
                slint::Timer::single_shot(Duration::from_millis(15), move || {
                    if let Some(s) = w.upgrade() {
                        s.attach(attempt + 1);
                    }
                });
            }
            return;
        }
        self.hwnd.set(hwnd);
        WindowStyles::apply_tool_window_and_no_activate(hwnd);
        WindowStyles::set_click_through(hwnd, true);
        let m = self.monitor.borrow().clone();
        self.place(m);
    }

    /// Çizgiyi hedef ekranın üst kenarına piksel olarak yerleştirir.
    pub fn place(&self, monitor: DisplayMonitor) {
        let hwnd = self.hwnd.get();
        let same_dpi = (self.view.window().scale_factor() as f64 - monitor.dpi_scale_y).abs() < 0.01;
        let rect = NotchEdgeLine::bounds(&monitor, self.thickness);
        *self.monitor.borrow_mut() = monitor;
        if hwnd != 0 {
            WindowStyles::set_bounds(hwnd, rect, same_dpi);
        }
    }

    pub fn reassert_topmost(&self) {
        let hwnd = self.hwnd.get();
        if hwnd != 0 {
            WindowStyles::reassert_topmost(hwnd);
        }
    }

    pub fn set_line_visible(self: &Rc<Self>, visible: bool) {
        if visible && !self.visible.get() {
            self.show();
        } else if !visible && self.visible.get() {
            let _ = self.view.hide();
            self.visible.set(false);
        }
    }

    pub fn close(&self) {
        let _ = self.view.hide();
        self.visible.set(false);
    }
}

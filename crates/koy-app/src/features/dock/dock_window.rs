//! Dock penceresinin yerel kabuğu. Karşılığı: `DockWindow.xaml.cs` ve `DockItemsStrip.xaml.cs`.
//!
//! Görünüm modelinin durumunu pencereye yazar, yayları kare kare ilerletir, pencereyi hedef ekrana yerleştirir
//! (kapsülde içerik boyutunda, tam genişlikte kenar çubuğu), pencere bölgesini görünen yüzeye kırpar ve şerit
//! etkileşimlerini (tıklama, orta tık, sağ tık menüsü, sürükleyerek sıralama, fare üstü) görünüm modeline iletir.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::ComponentHandle;

use koy_core::display::{DisplayMonitor, WindowPlacementMath};
use koy_core::dock::{DockEdge, DockLeftClickAction, DockPositionResolver, DockProximityMode, MiddleClickAction};
use koy_core::shell::WindowRect;
use koy_core::start_menu::StartMenuActions;
use koy_platform::shell::{CursorHelper, PopupMenuProbe, WindowStyles};

use super::dock_binding;
use super::dock_menus;
use super::dock_popups::DockPopups;
use super::dock_view_model::DockViewModel;
use super::entries::DockEntry;
use super::reorder_helper::DockReorderHelper;
use crate::DockWindow;
use crate::infra::native_window;
use crate::resources::theme_manager::ThemeManager;

const FRAME: Duration = Duration::from_millis(8);
const WM_DISPLAYCHANGE: u32 = 0x007E;
// AppBarService kaydındaki geri çağrı iletisi (WM_USER + 100) ve ABN_POSCHANGED.
const WM_APPBAR_CALLBACK: u32 = 0x0400 + 100;
const ABN_POSCHANGED: usize = 0x0001;

type Handler = RefCell<Option<Box<dyn Fn()>>>;

pub struct DockWindowHost {
    pub window: DockWindow,
    vm: RefCell<Rc<DockViewModel>>,
    reorder: RefCell<Rc<DockReorderHelper>>,
    popups: RefCell<Option<Rc<DockPopups>>>,
    monitor: RefCell<Option<DisplayMonitor>>,
    hwnd: Cell<isize>,
    timer: slint::Timer,
    last_frame: Cell<Option<Instant>>,
    subs: RefCell<Vec<(u8, u64)>>,
    hooks: RefCell<Vec<usize>>,
    me: RefCell<Weak<Self>>,
    /// Başlat düğmesinin sağ tık menüsü (Win+X karşılığı).
    pub start_menu: RefCell<Option<Arc<StartMenuActions>>>,
    pub display_change_handler: Handler,
    pub app_bar_pos_changed_handler: Handler,
    /// Pencere boyutu ya da konumu değişti (görünürlük sınırları ve AppBar yeniden hesaplanır).
    pub bounds_changed_handler: Handler,
    /// Yerel pencere hazır (kabuk kancası, AppBar kaydı, tam ekran algılayıcı).
    pub native_ready: RefCell<Option<Box<dyn FnOnce(isize)>>>,
    shell_hook: RefCell<Option<(u32, Box<dyn Fn(isize, isize)>)>>,
    last_size: Cell<(f32, f32)>,
}

impl DockWindowHost {
    pub fn new(vm: Rc<DockViewModel>) -> Option<Rc<Self>> {
        let window = DockWindow::new().ok()?;
        ThemeManager::install(&window);
        dock_binding::install_text(&window);
        let host = Rc::new(DockWindowHost {
            window,
            reorder: RefCell::new(DockReorderHelper::new(&vm)),
            vm: RefCell::new(vm),
            popups: RefCell::new(None),
            monitor: RefCell::new(None),
            hwnd: Cell::new(0),
            timer: slint::Timer::default(),
            last_frame: Cell::new(None),
            subs: RefCell::new(Vec::new()),
            hooks: RefCell::new(Vec::new()),
            me: RefCell::new(Weak::new()),
            start_menu: RefCell::new(None),
            display_change_handler: RefCell::new(None),
            app_bar_pos_changed_handler: RefCell::new(None),
            bounds_changed_handler: RefCell::new(None),
            native_ready: RefCell::new(None),
            shell_hook: RefCell::new(None),
            last_size: Cell::new((0.0, 0.0)),
        });
        *host.me.borrow_mut() = Rc::downgrade(&host);
        host.wire_window();
        host.attach_view_model();
        Some(host)
    }

    fn weak(&self) -> Weak<Self> {
        self.me.borrow().clone()
    }

    pub fn vm(&self) -> Rc<DockViewModel> {
        Rc::clone(&self.vm.borrow())
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd.get()
    }

    pub fn target_monitor(&self) -> Option<DisplayMonitor> {
        self.monitor.borrow().clone()
    }

    /// Kabuk kancası iletisi ve işleyicisi (pencere izleyicisinin RegisterShellHook'u).
    pub fn set_shell_hook(&self, message: u32, handler: impl Fn(isize, isize) + 'static) {
        *self.shell_hook.borrow_mut() = Some((message, Box::new(handler)));
    }

    // ---- Görünüm modeli ----

    fn attach_view_model(self: &Rc<Self>) {
        let vm = self.vm();
        let dock_rect = {
            let w = self.weak();
            Rc::new(move || w.upgrade().map(|h| h.window_rect()).unwrap_or_default()) as Rc<dyn Fn() -> WindowRect>
        };
        *self.popups.borrow_mut() = DockPopups::new(&vm, dock_rect);
        let w = self.weak();
        let a = vm.changed.subscribe(move || {
            if let Some(h) = w.upgrade() {
                h.refresh();
            }
        });
        let w = self.weak();
        let b = vm.animation_needed.subscribe(move || {
            if let Some(h) = w.upgrade() {
                h.start_rendering();
            }
        });
        let w = self.weak();
        let c = vm.animation_stopped.subscribe(move || {
            if let Some(h) = w.upgrade() {
                h.timer.stop();
            }
        });
        let w = self.weak();
        let d = vm.request_reposition.subscribe(move || {
            if let Some(h) = w.upgrade() {
                h.reposition(None);
            }
        });
        self.subs.borrow_mut().extend([(0, a), (1, b), (2, c), (3, d)]);
        vm.set_monitor(self.monitor.borrow().clone());
        self.refresh();
    }

    fn detach_view_model(&self) {
        let vm = self.vm();
        for (k, id) in self.subs.borrow_mut().drain(..) {
            match k {
                0 => vm.changed.unsubscribe(id),
                1 => vm.animation_needed.unsubscribe(id),
                2 => vm.animation_stopped.unsubscribe(id),
                _ => vm.request_reposition.unsubscribe(id),
            }
        }
        if let Some(p) = self.popups.borrow_mut().take() {
            p.dispose();
        }
        self.timer.stop();
    }

    /// Ayar değişince yeni görünüm modeli takılır (C# `SetViewModel`).
    pub fn set_view_model(self: &Rc<Self>, vm: Rc<DockViewModel>) {
        self.detach_view_model();
        *self.reorder.borrow_mut() = DockReorderHelper::new(&vm);
        *self.vm.borrow_mut() = vm;
        self.attach_view_model();
        let v = self.vm();
        v.evaluate_position(self.content_length());
        self.reposition(None);
    }

    /// Durumu pencereye yazar ve açılır pencereleri eşitler.
    fn refresh(&self) {
        crate::infra::trace::bump(&crate::infra::trace::DOCK_REFRESH);
        let vm = self.vm();
        self.window.set_d(dock_binding::dock_data(&vm));
        self.window.set_p(dock_binding::presentation_data(vm.presentation()));
        self.window.set_entries(dock_binding::entries(&vm));
        if let Some(p) = self.popups.borrow().as_ref() {
            p.sync();
        }
        self.apply_region();
        // Kapsülün içerik boyutu değiştiyse pencere yeniden yerleşir (WPF SizeChanged).
        let s = self.window.window().size();
        let size = (s.width as f32, s.height as f32);
        if size != self.last_size.get() {
            crate::infra::trace::bump(&crate::infra::trace::DOCK_RESIZE);
            crate::infra::trace::line(|| format!("dock boyut {:?} -> {:?}", self.last_size.get(), size));
            self.last_size.set(size);
            vm.evaluate_position(self.content_length());
            self.reposition(None);
            if let Some(f) = self.bounds_changed_handler.borrow().as_ref() {
                f();
            }
        }
    }

    // ---- Animasyon ----

    fn start_rendering(&self) {
        if self.timer.running() {
            return;
        }
        self.last_frame.set(None);
        let w = self.weak();
        self.timer.start(slint::TimerMode::Repeated, FRAME, move || {
            if let Some(h) = w.upgrade() {
                h.on_frame();
            }
        });
    }

    fn on_frame(&self) {
        crate::infra::trace::bump(&crate::infra::trace::DOCK_FRAME);
        let now = Instant::now();
        let dt = match self.last_frame.replace(Some(now)) {
            None => return,
            Some(t) => (now - t).as_secs_f64(),
        };
        let dt = if dt <= 0.0 || dt > 0.1 { 0.016 } else { dt };
        let vm = self.vm();
        // Hareket yoksa (yaylar başka yoldan durduruldu ya da hiç başlamadı) kare zamanlayıcısı boşta dönmesin.
        if !vm.is_animating() {
            self.timer.stop();
            return;
        }
        vm.update_springs(dt);
    }

    // ---- Yerleşim ----

    fn scale(&self) -> f64 {
        self.window.window().scale_factor() as f64
    }

    /// Pencerenin mantıksal boyutu (DIP).
    fn logical_size(&self) -> (f64, f64) {
        let s = self.window.window().size().to_logical(self.window.window().scale_factor());
        (s.width as f64, s.height as f64)
    }

    fn content_length(&self) -> f64 {
        let (w, h) = self.logical_size();
        if self.vm().is_vertical() { h } else { w }
    }

    pub fn actual_thickness(&self) -> f64 {
        let (w, h) = self.logical_size();
        if self.vm().is_vertical() { w } else { h }
    }

    /// AppBar'ın ayıracağı kalınlık (DIP).
    pub fn reserved_height(&self) -> f64 {
        let vm = self.vm();
        if vm.state().is_full_width_mode {
            return DockPositionResolver::FULL_WIDTH_HEIGHT_DIP;
        }
        if vm.effective_proximity() == DockProximityMode::ReserveSpace {
            return DockPositionResolver::SNAPPED_HEIGHT_DIP;
        }
        let t = self.actual_thickness();
        if t > 0.0 { t } else { 70.0 }
    }

    /// Pencerenin ekran dikdörtgeni (piksel).
    pub fn window_rect(&self) -> WindowRect {
        PopupMenuProbe::try_get_window_rect(self.hwnd.get()).unwrap_or_default()
    }

    pub fn set_target_monitor(&self, monitor: Option<DisplayMonitor>) {
        *self.monitor.borrow_mut() = monitor.clone();
        self.vm().set_monitor(monitor);
        self.reposition(None);
    }

    /// Pencereyi hedef ekrana yerleştirir: tam genişlikte kenar çubuğu, kapsülde içerik boyutunda kenar ortası.
    pub fn reposition(&self, target: Option<DisplayMonitor>) {
        if let Some(m) = target {
            *self.monitor.borrow_mut() = Some(m);
        }
        let vm = self.vm();
        let s = vm.settings();
        let edge = s.dock.edge;
        let edge_offset = s.dock.edge_offset_dip;
        let snapped = vm.state().is_snapped;
        let full = vm.state().is_full_width_mode;
        let capsule_margin = if snapped { edge_offset } else { DockPositionResolver::FLOATING_BOTTOM_MARGIN_DIP + edge_offset };
        let Some(monitor) = self.monitor.borrow().clone().filter(|m| m.bounds.width() > 100) else { return };
        let same_dpi = (self.scale() - monitor.dpi_scale_x).abs() < 0.01;
        if full {
            let rect = WindowPlacementMath::edge_bar(&monitor, edge, DockPositionResolver::FULL_WIDTH_HEIGHT_DIP, s.dock.length_percent, edge_offset);
            let sc = monitor.dpi_scale_x.max(0.1);
            self.window.set_bar_width(((rect.right - rect.left) as f64 / sc) as f32);
            self.window.set_bar_height(((rect.bottom - rect.top) as f64 / sc) as f32);
            if self.hwnd.get() != 0 {
                WindowStyles::set_bounds(self.hwnd.get(), rect, same_dpi);
            } else {
                self.window.window().set_position(slint::PhysicalPosition::new(rect.left, rect.top));
            }
        } else {
            let (w, h) = self.logical_size();
            let vertical = matches!(edge, DockEdge::Left | DockEdge::Right);
            let (length, thickness) = if vertical { (h, w) } else { (w, h) };
            let rect = WindowPlacementMath::edge_capsule(&monitor, edge, length, thickness, capsule_margin);
            if self.hwnd.get() != 0 {
                WindowStyles::set_bounds(self.hwnd.get(), rect, false);
            } else {
                self.window.window().set_position(slint::PhysicalPosition::new(rect.left, rect.top));
            }
        }
        self.apply_region();
    }

    /// Pencere bölgesi: görünen yüzey (kayma dahil), çekil tutamağı ve kenar çizgisi. Saydam pencere olmadığından
    /// kapsulün dışı ve gizlenmiş dock bu yolla görünmez ve tıklanmaz olur.
    fn apply_region(&self) {
        crate::infra::trace::bump(&crate::infra::trace::DOCK_REGION);
        let hwnd = self.hwnd.get();
        if hwnd == 0 {
            return;
        }
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, RGN_OR, SetWindowRgn};
        let vm = self.vm();
        let st = vm.state();
        let sc = self.scale();
        let px = |v: f64| (v * sc).round() as i32;
        let (w, h) = self.logical_size();
        let (tx, ty) = (st.translation_x, st.translation_y);
        let visible_surface = st.opacity > 0.02;
        // SAFETY: bölgeler burada oluşturulur; birleştirilenler silinir, son bölge SetWindowRgn ile sisteme geçer.
        unsafe {
            let rgn = CreateRectRgn(0, 0, 0, 0);
            if visible_surface {
                let body = if st.is_full_width_mode {
                    CreateRectRgn(px(tx), px(ty), px(w + tx), px(h + ty))
                } else {
                    let (ml, mt, mr, mb) = st.dock_margin;
                    let r = st.capsule_corner_radius.0.max(st.capsule_corner_radius.2);
                    CreateRoundRectRgn(px(ml + tx), px(mt + ty), px(w - mr + tx) + 1, px(h - mb + ty) + 1, px(r * 2.0), px(r * 2.0))
                };
                CombineRgn(Some(rgn), Some(rgn), Some(body), RGN_OR);
                let _ = DeleteObject(body.into());
            }
            if st.is_handle_visible {
                let (hw, hh) = vm.dodge_handle_size();
                let (x, y) = match vm.edge() {
                    DockEdge::Bottom => ((w - hw) / 2.0, h - hh - 4.0),
                    DockEdge::Top => ((w - hw) / 2.0, 4.0),
                    DockEdge::Left => (4.0, (h - hh) / 2.0),
                    DockEdge::Right => (w - hw - 4.0, (h - hh) / 2.0),
                };
                // Tutamağın üstünde fare algılansın diye biraz geniş tutulur.
                let handle = CreateRectRgn(px(x - 4.0), px(y - 4.0), px(x + hw + 4.0), px(y + hh + 4.0));
                CombineRgn(Some(rgn), Some(rgn), Some(handle), RGN_OR);
                let _ = DeleteObject(handle.into());
            }
            if st.is_edge_line_visible {
                let line = CreateRectRgn(px((w - 160.0) / 2.0), px(h - 2.0), px((w + 160.0) / 2.0), px(h));
                CombineRgn(Some(rgn), Some(rgn), Some(line), RGN_OR);
                let _ = DeleteObject(line.into());
            }
            let _ = SetWindowRgn(HWND(hwnd as *mut _), Some(rgn), true);
        }
    }

    // ---- Gösterim ve yerel kancalar ----

    pub fn show(self: &Rc<Self>) {
        if self.window.show().is_err() {
            return;
        }
        self.attach_native(0);
    }

    fn attach_native(self: &Rc<Self>, attempt: u32) {
        let hwnd = native_window::hwnd_of(self.window.window());
        if hwnd == 0 {
            if attempt < 200 {
                let w = self.weak();
                slint::Timer::single_shot(Duration::from_millis(15), move || {
                    if let Some(h) = w.upgrade() {
                        h.attach_native(attempt + 1);
                    }
                });
            }
            return;
        }
        self.hwnd.set(hwnd);
        WindowStyles::apply_tool_window_and_no_activate(hwnd);
        let w = self.weak();
        let id = native_window::add_hook(
            hwnd,
            Box::new(move |msg, wparam, lparam| {
                let h = w.upgrade()?;
                if msg == WM_DISPLAYCHANGE {
                    if let Some(f) = h.display_change_handler.borrow().as_ref() {
                        f();
                    }
                    h.reposition(None);
                } else if msg == WM_APPBAR_CALLBACK && wparam.0 == ABN_POSCHANGED {
                    if let Some(f) = h.app_bar_pos_changed_handler.borrow().as_ref() {
                        f();
                    }
                } else if let Some((m, f)) = h.shell_hook.borrow().as_ref()
                    && *m != 0
                    && msg == *m
                {
                    f(wparam.0 as isize, lparam.0);
                }
                None
            }),
        );
        self.hooks.borrow_mut().push(id);
        // Önce hedef ekrana yerleş, sonra kayıtlar: AppBar pencerenin bulunduğu monitöre göre yapılır.
        self.reposition(None);
        if let Some(f) = self.native_ready.borrow_mut().take() {
            f(hwnd);
        }
        self.refresh();
    }

    // ---- Etkileşim ----

    fn wire_window(self: &Rc<Self>) {
        let win = &self.window;
        macro_rules! on {
            ($setter:ident, |$h:ident $(, $arg:ident)*| $body:expr) => {{
                let w = self.weak();
                win.$setter(move |$($arg),*| {
                    if let Some($h) = w.upgrade() {
                        $body;
                    }
                });
            }};
        }
        on!(on_start_menu, |h| h.vm().start_menu());
        on!(on_start_right, |h| {
            let actions = h.start_menu.borrow().clone();
            if let Some(a) = actions {
                dock_menus::open_start_menu(&h.vm(), &a, h.hwnd.get(), cursor());
            }
        });
        on!(on_open_downloads, |h| h.vm().open_downloads());
        on!(on_toggle_recents, |h| h.vm().toggle_recents());
        on!(on_toggle_tray, |h| h.vm().toggle_tray());
        on!(on_open_koy_menu, |h| h.vm().open_koy_menu());
        on!(on_now_playing_clicked, |h| h.vm().open_notch_music());
        on!(on_mouse_over, |h, over| h.vm().set_mouse_over_dock(over));
        on!(on_handle_hover, |h, over| h.vm().set_mouse_at_bottom(over));
        on!(on_center_length, |h, len| h.vm().report_center_width(len as f64));
        on!(on_escape, |h| h.vm().close_preview_now());
        on!(on_window_right_click, |h| dock_menus::open_window_menu(&h.vm(), h.hwnd.get(), cursor()));
        on!(on_entry_entered, |h, i| h.on_entry_entered(i as usize));
        on!(on_entry_left, |h, _i| h.on_entry_left());
        on!(on_entry_pointer, |h, i, kind, button, x, y, slot| h.on_entry_pointer(i as usize, kind, button, (x as f64, y as f64), slot as f64));
    }

    fn entry(&self, index: usize) -> Option<DockEntry> {
        self.vm().items().get(index).cloned()
    }

    fn on_entry_entered(&self, index: usize) {
        let vm = self.vm();
        vm.on_item_hover(index);
        if let Some(DockEntry::App(a)) = self.entry(index) {
            vm.on_item_mouse_enter(&a);
        }
    }

    fn on_entry_left(&self) {
        self.reorder.borrow().on_mouse_leave();
        let vm = self.vm();
        vm.on_item_mouse_leave();
        vm.on_hover_ended();
    }

    fn on_entry_pointer(&self, index: usize, kind: i32, button: i32, pos: (f64, f64), slot: f64) {
        let vm = self.vm();
        let reorder = Rc::clone(&self.reorder.borrow());
        match (kind, button) {
            // Sol bas: basma geri bildirimi ve olası sürükleme.
            (0, 0) => reorder.on_mouse_down(index, pos, slot),
            // Orta tık: ayara göre kapat ya da yeni pencere.
            (0, 1) => {
                if let Some(DockEntry::App(a)) = self.entry(index) {
                    vm.close_preview_now();
                    if vm.settings().dock.middle_click_action == MiddleClickAction::Close {
                        a.borrow().close(vm.controller());
                    } else {
                        a.borrow().middle_click(vm.controller());
                    }
                }
            }
            (1, _) => reorder.on_mouse_move(pos, CursorHelper::is_any_mouse_button_down()),
            // Sol bırak: sürükleme bittiyse tıklama sayılmaz.
            (2, 0) => {
                if reorder.try_finish() {
                    return;
                }
                match self.entry(index) {
                    Some(DockEntry::App(a)) => {
                        // "Önizlemeleri göster": çalışan simgede kutucuk açılır; açılamazsa varsayılan eyleme düşer.
                        let running = a.borrow().model.is_running;
                        if vm.controller().left_click_action() == DockLeftClickAction::ShowThumbnails && running && vm.try_show_preview_now(&a) {
                            return;
                        }
                        vm.close_preview_now();
                        a.borrow().click(vm.controller());
                    }
                    Some(DockEntry::Folder(f)) => {
                        vm.close_preview_now();
                        f.borrow_mut().click();
                        vm.changed.notify();
                    }
                    Some(DockEntry::Extra(x)) => {
                        if x.borrow().is_interactive() {
                            vm.activate_extra(&x);
                        }
                    }
                    None => {}
                }
            }
            // Sağ bırak: girdinin menüsü.
            (2, 2) => {
                if let Some(e) = self.entry(index) {
                    let vm2 = Rc::clone(&vm);
                    dock_menus::open_entry_menu(&vm, &e, self.hwnd.get(), cursor(), move || vm2.changed.notify());
                }
            }
            _ => {}
        }
    }

    pub fn dispose(&self) {
        self.detach_view_model();
        let hwnd = self.hwnd.get();
        for id in self.hooks.borrow_mut().drain(..) {
            native_window::remove_hook(hwnd, id);
        }
        let _ = self.window.hide();
    }
}

fn cursor() -> (i32, i32) {
    CursorHelper::get_position().unwrap_or((0, 0))
}

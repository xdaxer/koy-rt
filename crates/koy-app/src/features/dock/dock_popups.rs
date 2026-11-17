//! Dock'un açılır pencerelerinin denetimi: tepsi, son kullanılanlar, klasör paneli ve pencere önizlemesi.
//! Karşılığı: `Tray/TrayButton.xaml.cs`, `Recents/RecentsButton.xaml.cs`, `Folder/FolderPanel.xaml.cs`,
//! `Preview/WindowThumbnailPresenter.cs` ve `DockWindow.xaml.cs`'teki önizleme işleyicileri.
//!
//! WPF Popup'ları hedef öğeye göre yerleşir; burada pencere imleç hizasında, dock'un kenarına göre (alttaki dock'ta
//! üstüne) açılır. Açılış yayı yok (yazılım çizici saydamlık desteklemiyor); kapanma kararları aynıdır.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, VecModel};

use koy_core::dock::{DockEdge, ThumbnailFit};
use koy_core::shell::WindowRect;
use koy_core::time::TimeSpan;
use koy_core::tray::{TrayMouseAction, TrayPopupDismissAction, TrayPopupDismissPolicy};
use koy_platform::shell::{CursorHelper, DwmThumbnailHelper, PopupMenuProbe, WindowStyles};

use super::dock_view_model::DockViewModel;
use super::entries::{DockEntry, DockFolderEntry};
use crate::infra::native_window;
use crate::resources::strings::Strings;
use crate::resources::theme_manager::ThemeManager;
use crate::{FolderAppData, FolderPopupWindow, PreviewPopupWindow, PreviewWindowData, RecentsPopupWindow, TrayIconData, TrayPopupWindow};

const GAP_DIP: f64 = 8.0;
const WATCH: Duration = Duration::from_millis(50);

/// Açılır pencereyi imleç hizasında, dock dikdörtgeninin (piksel) dışına, kenara göre yerleştirir.
fn place(window: &slint::Window, dock: WindowRect, edge: DockEdge) {
    let scale = window.scale_factor();
    let size = window.size();
    let (w, h) = (size.width as i32, size.height as i32);
    let gap = (GAP_DIP * scale as f64).round() as i32;
    let (cx, cy) = CursorHelper::get_position().unwrap_or(((dock.left + dock.right) / 2, (dock.top + dock.bottom) / 2));
    let (x, y) = match edge {
        DockEdge::Bottom => (cx - w / 2, dock.top - h - gap),
        DockEdge::Top => (cx - w / 2, dock.bottom + gap),
        DockEdge::Left => (dock.right + gap, cy - h / 2),
        DockEdge::Right => (dock.left - w - gap, cy - h / 2),
    };
    window.set_position(slint::PhysicalPosition::new(x, y));
}

pub(crate) fn rounded_region(hwnd: isize, w: i32, h: i32, radius: i32) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
    // SAFETY: bölge SetWindowRgn başarılıysa sistemin olur; HWND canlı penceredir.
    unsafe {
        let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, radius * 2, radius * 2);
        let _ = SetWindowRgn(HWND(hwnd as *mut _), Some(rgn), true);
    }
}

/// Pencere biçimi: araç penceresi, etkinleşmez, köşeleri yuvarlatılmış bölge.
fn style_popup(window: &slint::Window, radius_dip: f64) {
    let hwnd = native_window::hwnd_of(window);
    if hwnd == 0 {
        return;
    }
    crate::infra::native_window::make_popup(hwnd);
    WindowStyles::apply_tool_window_and_no_activate(hwnd);
    WindowStyles::reassert_topmost(hwnd);
    let s = window.size();
    rounded_region(hwnd, s.width as i32, s.height as i32, (radius_dip * window.scale_factor() as f64).round() as i32);
}

fn tick_ms() -> TimeSpan {
    thread_local! { static START: Instant = Instant::now(); }
    TimeSpan::from_milliseconds_f64(START.with(|s| s.elapsed().as_secs_f64() * 1000.0))
}

/// Dock'un açılır pencereleri.
pub struct DockPopups {
    vm: Weak<DockViewModel>,
    dock_rect: Rc<dyn Fn() -> WindowRect>,
    tray: TrayPopupWindow,
    recents: RecentsPopupWindow,
    folder: FolderPopupWindow,
    preview: PreviewPopupWindow,
    tray_shown: Cell<bool>,
    recents_shown: Cell<bool>,
    watch: slint::Timer,
    dismiss: RefCell<TrayPopupDismissPolicy>,
    prev_pointer_down: Cell<bool>,
    menu_may_be_open: Cell<bool>,
    last_left_down: Cell<Option<Instant>>,
    double_click: Cell<bool>,
    open_folder: RefCell<Option<Rc<RefCell<DockFolderEntry>>>>,
    folder_drag: RefCell<Option<(usize, (f32, f32))>>,
    preview_id: RefCell<Option<String>>,
    thumbs: RefCell<Vec<isize>>,
    me: RefCell<Weak<Self>>,
}

impl DockPopups {
    pub fn new(vm: &Rc<DockViewModel>, dock_rect: Rc<dyn Fn() -> WindowRect>) -> Option<Rc<Self>> {
        let p = Rc::new(DockPopups {
            vm: Rc::downgrade(vm),
            dock_rect,
            tray: TrayPopupWindow::new().ok()?,
            recents: RecentsPopupWindow::new().ok()?,
            folder: FolderPopupWindow::new().ok()?,
            preview: PreviewPopupWindow::new().ok()?,
            tray_shown: Cell::new(false),
            recents_shown: Cell::new(false),
            watch: slint::Timer::default(),
            dismiss: RefCell::new(TrayPopupDismissPolicy::default()),
            prev_pointer_down: Cell::new(false),
            menu_may_be_open: Cell::new(false),
            last_left_down: Cell::new(None),
            double_click: Cell::new(false),
            open_folder: RefCell::new(None),
            folder_drag: RefCell::new(None),
            preview_id: RefCell::new(None),
            thumbs: RefCell::new(Vec::new()),
            me: RefCell::new(Weak::new()),
        });
        *p.me.borrow_mut() = Rc::downgrade(&p);
        ThemeManager::install(&p.tray);
        ThemeManager::install(&p.recents);
        ThemeManager::install(&p.folder);
        ThemeManager::install(&p.preview);
        p.tray.set_title_text(Strings::get("SystemTray").into());
        p.tray.set_empty_text(Strings::get("TrayEmpty").into());
        p.recents.set_title_text(Strings::get("RecentFiles").into());
        p.recents.set_empty_text(Strings::get("RecentEmpty").into());
        p.folder.set_rename_tip(Strings::get("DockFolderRename").into());
        p.folder.set_dissolve_tip(Strings::get("DockFolderDissolve").into());
        p.preview.set_close_tip(Strings::get("Close").into());
        p.wire();
        Some(p)
    }

    fn weak(&self) -> Weak<Self> {
        self.me.borrow().clone()
    }

    fn wire(self: &Rc<Self>) {
        let w = self.weak();
        self.tray.on_icon_action(move |i, action| {
            if let Some(p) = w.upgrade() {
                p.on_tray_icon(i as usize, action);
            }
        });
        let w = self.weak();
        self.tray.on_escape(move || {
            if let Some(vm) = w.upgrade().and_then(|p| p.vm.upgrade()) {
                vm.set_tray_popup_open(false);
            }
        });
        let w = self.weak();
        self.recents.on_open(move |i| {
            if let Some(vm) = w.upgrade().and_then(|p| p.vm.upgrade()) {
                vm.open_recent(i as usize);
            }
        });
        let w = self.weak();
        self.recents.on_escape(move || {
            if let Some(vm) = w.upgrade().and_then(|p| p.vm.upgrade()) {
                vm.set_recents_open(false);
            }
        });
        // Klasör paneli.
        let w = self.weak();
        self.folder.on_start_edit(move || {
            if let Some(p) = w.upgrade() {
                p.start_folder_edit();
            }
        });
        let w = self.weak();
        self.folder.on_rename(move |text| {
            if let Some(p) = w.upgrade() {
                p.finish_folder_edit(Some(text.to_string()));
            }
        });
        let w = self.weak();
        self.folder.on_cancel_edit(move || {
            if let Some(p) = w.upgrade() {
                p.finish_folder_edit(None);
            }
        });
        let w = self.weak();
        self.folder.on_dissolve(move || {
            if let Some(p) = w.upgrade()
                && let (Some(vm), Some(f)) = (p.vm.upgrade(), p.open_folder.borrow().clone())
            {
                f.borrow_mut().close_folder_panel();
                vm.dissolve_folder(&f);
                p.sync();
            }
        });
        let w = self.weak();
        self.folder.on_escape(move || {
            if let Some(p) = w.upgrade() {
                p.close_folder();
            }
        });
        let w = self.weak();
        self.folder.on_app_pointer(move |i, action, x, y| {
            if let Some(p) = w.upgrade() {
                p.on_folder_app(i as usize, action, (x, y));
            }
        });
        let w = self.weak();
        self.folder.on_app_moved(move |_, x, y| {
            if let Some(p) = w.upgrade() {
                p.on_folder_app_moved((x, y));
            }
        });
        // Önizleme.
        let w = self.weak();
        self.preview.on_hover(move |inside| {
            if let Some(vm) = w.upgrade().and_then(|p| p.vm.upgrade()) {
                if inside {
                    vm.on_popup_mouse_enter();
                } else {
                    vm.on_popup_mouse_leave();
                }
            }
        });
        let w = self.weak();
        self.preview.on_thumb_action(move |i, action| {
            if let Some(p) = w.upgrade() {
                p.on_thumb(i as usize, action);
            }
        });
    }

    /// Görünüm modeli değişince çağrılır: hangi pencerenin açık olacağı durumdan okunur.
    pub fn sync(&self) {
        let Some(vm) = self.vm.upgrade() else { return };
        let (tray_open, recents_open, preview_open, preview_id) = {
            let st = vm.state();
            (st.is_tray_popup_open, st.is_recents_open, st.is_preview_popup_open, st.preview_item_id.clone())
        };
        let edge = vm.edge();
        let dock = (self.dock_rect)();
        // Tepsi.
        if tray_open {
            let icons: Vec<TrayIconData> = vm
                .tray_icons()
                .iter()
                .map(|i| TrayIconData { has_icon: i.icon.is_some(), icon: i.icon.clone().unwrap_or_default(), tooltip: i.tooltip.clone().unwrap_or_default().into() })
                .collect();
            self.tray.set_icons(ModelRc::new(VecModel::from(icons)));
            if !self.tray_shown.replace(true) {
                self.dismiss.borrow_mut().reset();
                self.prev_pointer_down.set(CursorHelper::is_any_mouse_button_down());
                self.show_popup(0, dock, edge);
                self.start_watch();
            }
        } else if self.tray_shown.replace(false) {
            let _ = self.tray.hide();
        }
        // Son kullanılanlar.
        if recents_open {
            let names: Vec<slint::SharedString> = vm.state().recent_files.iter().map(|f| f.name.clone().into()).collect();
            self.recents.set_names(ModelRc::new(VecModel::from(names)));
            if !self.recents_shown.replace(true) {
                self.prev_pointer_down.set(CursorHelper::is_any_mouse_button_down());
                self.show_popup(1, dock, edge);
                self.start_watch();
            }
        } else if self.recents_shown.replace(false) {
            let _ = self.recents.hide();
        }
        // Klasör paneli: açık klasör aranır.
        let open = vm.items().into_iter().find_map(|e| match e {
            DockEntry::Folder(f) if f.borrow().is_folder_panel_open => Some(f),
            _ => None,
        });
        match open {
            Some(f) => {
                let changed = self.open_folder.borrow().as_ref().is_none_or(|o| !Rc::ptr_eq(o, &f));
                self.fill_folder(&f);
                if changed {
                    *self.open_folder.borrow_mut() = Some(Rc::clone(&f));
                    self.show_popup(2, dock, edge);
                    self.start_watch();
                }
                if f.borrow().is_editing_title && !self.folder.get_editing() {
                    self.start_folder_edit();
                }
            }
            None => {
                if self.open_folder.borrow_mut().take().is_some() {
                    let _ = self.folder.hide();
                }
            }
        }
        // Önizleme.
        if preview_open && let Some(id) = preview_id {
            let same = self.preview_id.borrow().as_deref() == Some(id.as_str());
            if !same {
                self.show_preview(&vm, &id, dock, edge);
            }
        } else if self.preview_id.borrow_mut().take().is_some() {
            self.clear_thumbs();
            let _ = self.preview.hide();
        }
    }

    fn popup_window(&self, which: u8) -> &slint::Window {
        match which {
            0 => self.tray.window(),
            1 => self.recents.window(),
            2 => self.folder.window(),
            _ => self.preview.window(),
        }
    }

    /// Pencereyi gösterir ve yerleştirir; yerel pencere hazır olunca (birkaç ms) biçim ve bölge uygulanır.
    fn show_popup(&self, which: u8, dock: WindowRect, edge: DockEdge) {
        let shown = match which {
            0 => self.tray.show(),
            1 => self.recents.show(),
            2 => self.folder.show(),
            _ => self.preview.show(),
        };
        if shown.is_err() {
            return;
        }
        place(self.popup_window(which), dock, edge);
        self.style_when_ready(which, dock, edge, 0);
    }

    fn style_when_ready(&self, which: u8, dock: WindowRect, edge: DockEdge, attempt: u32) {
        let w = self.popup_window(which);
        if native_window::hwnd_of(w) != 0 {
            place(w, dock, edge);
            style_popup(w, 14.0);
            return;
        }
        if attempt < 100 {
            let me = self.weak();
            slint::Timer::single_shot(Duration::from_millis(10), move || {
                if let Some(p) = me.upgrade() {
                    p.style_when_ready(which, dock, edge, attempt + 1);
                }
            });
        }
    }

    fn start_watch(&self) {
        if self.watch.running() {
            return;
        }
        let w = self.weak();
        self.watch.start(slint::TimerMode::Repeated, WATCH, move || {
            if let Some(p) = w.upgrade() {
                p.on_watch();
            }
        });
    }

    /// 50 ms yoklama: dış tıklama ve uygulama menüsü durumu (tepsi), dış tıklama (son kullanılanlar, klasör).
    fn on_watch(&self) {
        let Some(vm) = self.vm.upgrade() else { return };
        let any_open = self.tray_shown.get() || self.recents_shown.get() || self.open_folder.borrow().is_some();
        if !any_open {
            self.watch.stop();
            return;
        }
        let down = CursorHelper::is_any_mouse_button_down();
        let pressed = down && !self.prev_pointer_down.get();
        self.prev_pointer_down.set(down);
        let cursor = CursorHelper::get_position();
        let inside = |w: &slint::Window| {
            let h = native_window::hwnd_of(w);
            matches!((cursor, PopupMenuProbe::try_get_window_rect(h)), (Some((x, y)), Some(r)) if r.contains(x, y))
        };
        let inside_dock = cursor.is_some_and(|(x, y)| (self.dock_rect)().contains(x, y));
        if self.tray_shown.get() {
            let menus = if pressed || self.menu_may_be_open.get() { PopupMenuProbe::get_visible_menu_rects(true) } else { Vec::new() };
            let inside_popup = pressed && inside(self.tray.window());
            let inside_menu = pressed && cursor.is_some_and(|(x, y)| menus.iter().any(|m| m.contains(x, y)));
            let action = self.dismiss.borrow_mut().tick(tick_ms(), pressed, inside_popup, inside_menu, !menus.is_empty());
            self.menu_may_be_open.set(self.dismiss.borrow().is_expecting_menu());
            if action == TrayPopupDismissAction::Close {
                vm.set_tray_popup_open(false);
            }
        }
        // StaysOpen=False karşılığı: dışarı (dock dışı) basış kapatır; düğmeye basış zaten aç/kapa yapar.
        if pressed && !inside_dock {
            if self.recents_shown.get() && !inside(self.recents.window()) {
                vm.set_recents_open(false);
            }
            if self.open_folder.borrow().is_some() && !inside(self.folder.window()) {
                self.close_folder();
            }
        }
    }

    fn on_tray_icon(&self, index: usize, action: i32) {
        let Some(vm) = self.vm.upgrade() else { return };
        let dock = (self.dock_rect)();
        match action {
            // Çift tıklama bırakmada gönderilir (görev çubuğu da DBLCLK ardından LBUTTONUP yollar).
            0 => {
                let now = Instant::now();
                let dbl = Duration::from_millis(CursorHelper::double_click_time_ms() as u64);
                self.double_click.set(self.last_left_down.get().is_some_and(|t| now - t <= dbl));
                self.last_left_down.set(Some(now));
            }
            1 => {
                let a = if self.double_click.replace(false) { TrayMouseAction::LeftDoubleClick } else { TrayMouseAction::LeftClick };
                vm.send_tray_mouse(index, a, dock);
            }
            _ => {
                vm.send_tray_mouse(index, TrayMouseAction::RightClick, dock);
                // Pencere açık kalır; uygulamanın menüsü belirince ve kapanınca karar verilir.
                self.dismiss.borrow_mut().on_icon_right_clicked(tick_ms());
                self.menu_may_be_open.set(true);
            }
        }
    }

    // ---- Klasör ----

    fn fill_folder(&self, f: &Rc<RefCell<DockFolderEntry>>) {
        let f = f.borrow();
        self.folder.set_folder_title(f.title.clone().into());
        let apps: Vec<FolderAppData> = f
            .items
            .iter()
            .map(|a| {
                let a = a.borrow();
                FolderAppData { title: a.title.clone().into(), has_icon: a.icon.is_some(), icon: a.icon.clone().unwrap_or_default(), running: a.is_running }
            })
            .collect();
        self.folder.set_apps(ModelRc::new(VecModel::from(apps)));
    }

    fn folder_hwnd(&self) -> isize {
        native_window::hwnd_of(self.folder.window())
    }

    fn start_folder_edit(&self) {
        let h = self.folder_hwnd();
        if h != 0 {
            WindowStyles::set_no_activate(h, false);
            native_window::activate(h);
        }
        self.folder.set_editing(true);
        self.folder.invoke_focus_title();
    }

    fn finish_folder_edit(&self, new_name: Option<String>) {
        if !self.folder.get_editing() {
            return;
        }
        self.folder.set_editing(false);
        if let (Some(vm), Some(f)) = (self.vm.upgrade(), self.open_folder.borrow().clone()) {
            if let Some(n) = new_name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
                vm.rename_folder(&f, &n);
                self.folder.set_folder_title(n.into());
            }
            f.borrow_mut().is_editing_title = false;
        }
        let h = self.folder_hwnd();
        if h != 0 {
            WindowStyles::set_no_activate(h, true);
        }
    }

    fn close_folder(&self) {
        if self.folder.get_editing() {
            self.finish_folder_edit(None);
        }
        if let Some(f) = self.open_folder.borrow_mut().take() {
            f.borrow_mut().close_folder_panel();
        }
        let _ = self.folder.hide();
        if let Some(vm) = self.vm.upgrade() {
            vm.changed.notify();
        }
    }

    fn on_folder_app(&self, index: usize, action: i32, pos: (f32, f32)) {
        if action == 0 {
            *self.folder_drag.borrow_mut() = Some((index, pos));
            return;
        }
        let Some((i, _)) = self.folder_drag.borrow_mut().take() else { return };
        let (Some(vm), Some(f)) = (self.vm.upgrade(), self.open_folder.borrow().clone()) else { return };
        let app = f.borrow().items.get(i).cloned();
        if let Some(a) = app {
            a.borrow().click(vm.controller());
        }
        self.close_folder();
    }

    /// Uygulama panelin dışına sürüklendi: klasörden çıkarılır (klasörün hemen arkasına).
    fn on_folder_app_moved(&self, pos: (f32, f32)) {
        let Some((i, start)) = *self.folder_drag.borrow() else { return };
        if !CursorHelper::is_any_mouse_button_down() {
            return;
        }
        let (dx, dy) = (pos.0 - start.0, pos.1 - start.1);
        if dx.abs() <= 4.0 && dy.abs() <= 4.0 {
            return;
        }
        let size = self.folder.window().size().to_logical(self.folder.window().scale_factor());
        let outside = pos.0 < 0.0 || pos.1 < 0.0 || pos.0 > size.width || pos.1 > size.height;
        if !outside {
            return;
        }
        *self.folder_drag.borrow_mut() = None;
        let (Some(vm), Some(f)) = (self.vm.upgrade(), self.open_folder.borrow().clone()) else { return };
        let (folder_id, item_id) = {
            let fb = f.borrow();
            (fb.folder.id.clone(), fb.items.get(i).map(|a| a.borrow().id().to_string()))
        };
        if let Some(item) = item_id {
            vm.remove_app_from_folder(&folder_id, &item, -1);
        }
        self.close_folder();
    }

    // ---- Önizleme ----

    fn show_preview(&self, vm: &Rc<DockViewModel>, id: &str, dock: WindowRect, edge: DockEdge) {
        let Some(app) = vm.preview_item() else { return };
        let windows = app.borrow().windows.clone();
        if windows.is_empty() {
            return;
        }
        self.clear_thumbs();
        *self.preview_id.borrow_mut() = Some(id.to_string());
        let multi = windows.len() > 1;
        self.preview.set_multi(multi);
        self.preview.set_windows(ModelRc::new(VecModel::from(
            windows.iter().map(|w| PreviewWindowData { title: w.title.clone().into() }).collect::<Vec<_>>(),
        )));
        self.show_popup(3, dock, edge);
        // Canlı küçük resimler: pencere hazır olunca yerleşim sabitlerinden hesaplanan kutulara kaydedilir.
        let w = self.weak();
        let handles: Vec<isize> = windows.iter().map(|w| w.handle).collect();
        slint::Timer::single_shot(Duration::from_millis(30), move || {
            if let Some(p) = w.upgrade() {
                p.register_thumbs(&handles, multi);
            }
        });
    }

    fn register_thumbs(&self, handles: &[isize], multi: bool) {
        let dest = native_window::hwnd_of(self.preview.window());
        if dest == 0 {
            return;
        }
        let scale = self.preview.window().scale_factor() as f64;
        let px = |v: f64| (v * scale).round() as i32;
        let mut thumbs = self.thumbs.borrow_mut();
        for (i, h) in handles.iter().enumerate() {
            // Kutular .slint'teki sabitlerle aynıdır: kenar 9, tekte başlık 28 + 132 yükseklik, çokluda 148 adım + 18 + 70.
            let (l, t, wd, ht) = if multi { (9.0 + i as f64 * 148.0, 9.0 + 18.0, 140.0, 70.0) } else { (9.0, 9.0 + 28.0, 232.0, 132.0) };
            let thumb = DwmThumbnailHelper::register(dest, *h);
            if thumb == 0 {
                continue;
            }
            let bx = WindowRect { left: px(l), top: px(t), right: px(l + wd), bottom: px(t + ht) };
            let (sw, sh) = DwmThumbnailHelper::query_source_size(thumb);
            DwmThumbnailHelper::update(thumb, ThumbnailFit::fit(sw, sh, bx), 255);
            thumbs.push(thumb);
        }
    }

    fn clear_thumbs(&self) {
        for t in self.thumbs.borrow_mut().drain(..) {
            DwmThumbnailHelper::unregister(t);
        }
    }

    fn on_thumb(&self, index: usize, action: i32) {
        let Some(vm) = self.vm.upgrade() else { return };
        let Some(app) = vm.preview_item() else { return };
        let Some(handle) = app.borrow().windows.get(index).map(|w| w.handle) else { return };
        match action {
            0 => vm.controller().activate_window(handle),
            _ => vm.controller().close_window(handle),
        }
        vm.close_preview_now();
    }

    pub fn dispose(&self) {
        self.watch.stop();
        self.clear_thumbs();
        let _ = self.tray.hide();
        let _ = self.recents.hide();
        let _ = self.folder.hide();
        let _ = self.preview.hide();
    }
}

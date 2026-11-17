//! Dock penceresinin ana görünüm modeli. Karşılığı: `DockViewModel.cs`.
//!
//! Yüzen kapsül ve tam genişlik kiplerini, yakınlık davranışlarını (DockDown, ReserveSpace, Dodge, AlwaysHidden),
//! fare üstü büyütmeyi, sabitli sıralamayı, klasörleri, ek öğeleri, tepsiyi, son kullanılanları ve pencere küçük resim
//! önizlemesini yönetir. Görünüm durumu `DockState`'tedir; şerit girdileri `items()`; değişince `changed` bildirilir.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use koy_core::animation::Spring;
use koy_core::appearance::{AppIconKey, AppIconOverrides, MaterialSetting, MotionKind};
use koy_core::display::DisplayMonitor;
use koy_core::dock::items::{DockClockTick, DockCustomCommand, DockItemKind, DockItemLayout, DockItemRules, DockItemSpec};
use koy_core::dock::{
    DockController, DockEdge, DockFitMath, DockFolder, DockFolders, DockIconAlignment, DockItem, DockLayoutLimits, DockMonitorFilter,
    DockPositionResolver, DockPresentationMath, DockProximityMapping, DockProximityMode, DockReorderMath, DockResolvedState,
    DockRightSection, DockStyleMode, DockWindowMonitorScope, PinnedOrder, RecentFile, RecentFilesPolicy, SnapDebounce,
};
use koy_core::event::Subscription;
use koy_core::jump_list::IJumpListService;
use koy_core::media::{IMediaService, MediaInfo};
use koy_core::settings::{AppSettings, ValueDictionary};
use koy_core::shell::{IShellActionService, WindowRect};
use koy_core::time::TimeSpan;
use koy_core::tray::{ITrayService, TrayIconVisibility, TrayIconVisibilityPolicy, TrayMenuAnchor, TrayMouseAction};
use koy_core::visibility::VisibilityController;
use koy_platform::shell::CursorHelper;

use super::app_icons::{self, DockAppIcons, StartIcon};
use super::context_menu_state;
use super::dock_settings_store;
use super::entries::{AppRef, DockAppEntry, DockEntry, DockExtraEntry, DockFolderEntry};
use super::hooks;
use super::presentation::DockPresentationStyle;
use super::status_view_model::DockStatusViewModel;
use super::tray_icons::{self, TrayIconEntry};
use crate::infra::dispatch;
use crate::infra::observable::{LocalEvent, Notifier, SharedSettings};
use crate::infra::ui_ref::UiRef;
use crate::resources::material_state::MaterialState;
use crate::resources::motion::Motion;
use crate::resources::strings::{self, Strings};
use crate::resources::theme_manager::ThemeManager;

// Kapsülün ölçülmüş sabit payları (DIP): sol/sağ pencere kenar boşluğu, iç dolgu + çerçeve, ayırıcı (1 + 4 + 4).
const CAPSULE_WINDOW_SIDE_MARGINS: f64 = 24.0;
const CAPSULE_PADDING_AND_BORDER: f64 = 18.0;
const CAPSULE_SEPARATOR_WIDTH: f64 = 9.0;
// Fare üstü tepe büyüme (C# `DockFitMath.HoverPeakScale` varsayılanı).
const HOVER_MAX_SCALE: f64 = 1.30;
// Taban renkler (tema `KoyDockSurface` / `KoyDockCapsuleSurface` vermezse).
const DEFAULT_FULL_WIDTH: u32 = 0xFF12_1216;
const DEFAULT_CAPSULE: u32 = 0xFF18_181B;

/// (sol, üst, sağ, alt) DIP.
pub type Thickness = (f64, f64, f64, f64);

/// Görünümün bağlandığı durum (C#'taki `[ObservableProperty]` alanları).
#[derive(Clone)]
pub struct DockState {
    pub translation_x: f64,
    pub translation_y: f64,
    pub opacity: f64,
    pub is_visible: bool,
    pub magnification_enabled: bool,
    pub is_full_width_mode: bool,
    pub is_snapped: bool,
    pub is_handle_visible: bool,
    pub is_edge_line_visible: bool,
    pub dock_height: f64,
    /// Dikeyde kapsül genişliği (yatayda `None` = içeriğe göre).
    pub capsule_width: Option<f64>,
    /// Yatayda kapsül yüksekliği (dikeyde `None`).
    pub capsule_height: Option<f64>,
    pub dock_margin: Thickness,
    /// (sol üst, sağ üst, sağ alt, sol alt).
    pub capsule_corner_radius: (f64, f64, f64, f64),
    pub show_start_button: bool,
    pub show_folders: bool,
    pub show_badges: bool,
    pub is_tray_popup_open: bool,
    pub has_tray_icons: bool,
    pub is_recents_open: bool,
    pub recent_files: Vec<RecentFile>,
    pub now_playing_title: String,
    pub now_playing_artist: String,
    pub now_playing_cover: Option<slint::Image>,
    pub has_now_playing: bool,
    pub icon_size: f64,
    pub item_size: f64,
    pub full_width_surface: slint::Color,
    pub capsule_surface: slint::Color,
    pub is_strip_clipped: bool,
    /// Uygulama şeridinin en çok genişliği (sınırsızsa `f64::INFINITY`).
    pub items_max_width: f64,
    pub is_preview_popup_open: bool,
    /// Önizlenen uygulama girdisinin kimliği (önizleme kutucuğu bu girdinin üstünde açılır).
    pub preview_item_id: Option<String>,
}

impl Default for DockState {
    fn default() -> Self {
        DockState {
            translation_x: 0.0,
            translation_y: 0.0,
            opacity: 1.0,
            is_visible: true,
            magnification_enabled: false,
            is_full_width_mode: false,
            is_snapped: false,
            is_handle_visible: false,
            is_edge_line_visible: false,
            dock_height: 60.0,
            capsule_width: None,
            capsule_height: Some(60.0),
            dock_margin: (12.0, 12.0, 12.0, 10.0),
            capsule_corner_radius: (18.0, 18.0, 18.0, 18.0),
            show_start_button: true,
            show_folders: true,
            show_badges: true,
            is_tray_popup_open: false,
            has_tray_icons: false,
            is_recents_open: false,
            recent_files: Vec::new(),
            now_playing_title: Strings::get("NoMediaPlaying"),
            now_playing_artist: "Koy".into(),
            now_playing_cover: None,
            has_now_playing: false,
            icon_size: 28.0,
            item_size: 38.0,
            full_width_surface: slint::Color::from_argb_encoded(DEFAULT_FULL_WIDTH),
            capsule_surface: slint::Color::from_argb_encoded(DEFAULT_CAPSULE),
            is_strip_clipped: false,
            items_max_width: f64::INFINITY,
            is_preview_popup_open: false,
            preview_item_id: None,
        }
    }
}

/// Konum ve fare durumu (C#'ta özel alanlar).
struct Pointer {
    current_monitor: Option<DisplayMonitor>,
    foreground_bounds: WindowRect,
    foreground_maximized: bool,
    foreground_fullscreen: bool,
    mouse_at_bottom_edge: bool,
    mouse_over_dock: bool,
    mouse_over_popup: bool,
    shell_flyout_open: bool,
    last_content_width: f64,
    center_width: f64,
    preferred_icon_size: f64,
    monitor_bounds: Option<WindowRect>,
    pending_preview_id: Option<String>,
    is_reordering: bool,
    needs_edge_watch: bool,
    is_animating: bool,
    battery_reduce_motion: bool,
    layout: Vec<DockItemSpec>,
    effective_order: Vec<String>,
    has_extras: bool,
    stable_resolved: Option<DockResolvedState>,
}

struct Springs {
    translate_x: Spring,
    translate_y: Spring,
    opacity: Spring,
}

/// Kurucu girdileri.
pub struct DockParts {
    pub settings: SharedSettings,
    pub controller: Arc<DockController>,
    pub shell: Arc<dyn IShellActionService>,
    pub visibility: Option<Arc<VisibilityController>>,
    pub status: Option<Rc<DockStatusViewModel>>,
    pub media: Option<Arc<dyn IMediaService>>,
    pub tray: Option<Arc<dyn ITrayService>>,
    pub recent_files: Option<Arc<dyn koy_core::dock::IRecentFilesService>>,
    pub jump_lists: Option<Arc<dyn IJumpListService + Send + Sync>>,
}

pub struct DockViewModel {
    settings: RefCell<SharedSettings>,
    controller: Arc<DockController>,
    shell: Arc<dyn IShellActionService>,
    visibility: Option<Arc<VisibilityController>>,
    media: Option<Arc<dyn IMediaService>>,
    tray: RefCell<Option<Arc<dyn ITrayService>>>,
    recent_files: Option<Arc<dyn koy_core::dock::IRecentFilesService>>,
    /// Uygulama simgesinin sağ tık menüsündeki atlama listesi; yalnız menü açılınca okunur.
    pub jump_lists: Option<Arc<dyn IJumpListService + Send + Sync>>,
    pub status: Option<Rc<DockStatusViewModel>>,
    /// Başlat düğmesinin simgesi (düğme gizliyse `None`).
    pub start_icon: Option<StartIcon>,
    presentation: Rc<DockPresentationStyle>,
    app_icons: RefCell<DockAppIcons>,
    state: RefCell<DockState>,
    p: RefCell<Pointer>,
    springs: RefCell<Springs>,
    snap_debounce: RefCell<SnapDebounce>,
    items: RefCell<Vec<DockEntry>>,
    tray_icons: RefCell<Vec<TrayIconEntry>>,
    tray_exe_by_owner: RefCell<HashMap<isize, String>>,
    tray_popup_open: Arc<AtomicBool>,
    tray_sync_queued: Arc<AtomicBool>,
    thumbnail_hover_timer: slint::Timer,
    preview_close_timer: slint::Timer,
    dodge_hover_timer: slint::Timer,
    snap_timer: slint::Timer,
    clock_timer: slint::Timer,
    tray_close_timer: slint::Timer,
    tray_clear_timer: slint::Timer,
    thumbnail_delay: Duration,
    dodge_delay: Duration,
    disposed: Cell<bool>,
    me: RefCell<Weak<Self>>,
    subs: RefCell<Vec<(u8, Subscription)>>,
    menu_sub: Cell<u64>,
    started: Instant,
    pub changed: Notifier,
    /// Dock gizli ve kenardan açılabilir: kenar zamanlayıcısı çalışmalı (`needs_edge_watch` değişti).
    pub edge_watch_changed: Notifier,
    pub animation_needed: Notifier,
    pub animation_stopped: Notifier,
    pub request_reposition: Notifier,
    /// Ayarlar ya da şerit yeniden kuruldu (girdi listesi değişti).
    pub items_changed: LocalEvent<()>,
}

fn read_color(key: &str, fallback: u32) -> u32 {
    let c = ThemeManager::color(key);
    if c == 0 { fallback } else { c }
}

impl DockViewModel {
    pub fn new(parts: DockParts) -> Rc<Self> {
        let s = parts.settings.borrow().clone();
        let icon_size = if s.dock.icon_size > 0.0 { s.dock.icon_size } else { 28.0 };
        super::entries::set_icon_pixels(crate::infra::image_decode::pixels_for(icon_size));
        let state = DockState {
            icon_size,
            item_size: DockFitMath::item_size_for(icon_size),
            magnification_enabled: s.dock.hover_magnify,
            is_full_width_mode: s.dock.style == DockStyleMode::FullWidth,
            show_start_button: s.dock.show_start_button,
            show_folders: s.dock.show_folders,
            show_badges: s.dock.show_badges,
            ..Default::default()
        };
        let vm = Rc::new(DockViewModel {
            controller: parts.controller,
            shell: parts.shell,
            visibility: parts.visibility,
            media: parts.media,
            tray: RefCell::new(parts.tray),
            recent_files: parts.recent_files,
            jump_lists: parts.jump_lists,
            status: parts.status,
            // Düğme gizliyken dosya okunmaz.
            start_icon: s.dock.show_start_button.then(|| app_icons::load_start_icon(&s)),
            presentation: Rc::new(DockPresentationStyle::from_settings(&s.dock)),
            app_icons: RefCell::new(DockAppIcons::new(&s)),
            state: RefCell::new(state),
            p: RefCell::new(Pointer {
                current_monitor: None,
                foreground_bounds: WindowRect::default(),
                foreground_maximized: false,
                foreground_fullscreen: false,
                mouse_at_bottom_edge: false,
                mouse_over_dock: false,
                mouse_over_popup: false,
                shell_flyout_open: false,
                last_content_width: 520.0,
                center_width: 0.0,
                preferred_icon_size: icon_size,
                monitor_bounds: None,
                pending_preview_id: None,
                is_reordering: false,
                needs_edge_watch: false,
                is_animating: false,
                battery_reduce_motion: false,
                layout: Vec::new(),
                effective_order: Vec::new(),
                has_extras: false,
                stable_resolved: None,
            }),
            springs: RefCell::new(Springs {
                translate_x: Spring::new(0.0).with_stiffness(280.0).with_damping(25.0),
                translate_y: Spring::new(0.0).with_stiffness(280.0).with_damping(25.0),
                opacity: Spring::new(1.0).with_stiffness(200.0).with_damping(24.0),
            }),
            // Aşağı yapış'ta yüzen ↔ yapışık geçişi 150 ms'den kısa sürerse uygulanmaz (Alt+Tab ara durakları).
            snap_debounce: RefCell::new(SnapDebounce::new(TimeSpan::from_millis(150))),
            items: RefCell::new(Vec::new()),
            tray_icons: RefCell::new(Vec::new()),
            tray_exe_by_owner: RefCell::new(HashMap::new()),
            tray_popup_open: Arc::new(AtomicBool::new(false)),
            tray_sync_queued: Arc::new(AtomicBool::new(false)),
            thumbnail_hover_timer: slint::Timer::default(),
            preview_close_timer: slint::Timer::default(),
            dodge_hover_timer: slint::Timer::default(),
            snap_timer: slint::Timer::default(),
            clock_timer: slint::Timer::default(),
            tray_close_timer: slint::Timer::default(),
            tray_clear_timer: slint::Timer::default(),
            thumbnail_delay: Duration::from_millis(s.dock.thumbnail_delay_ms.max(50) as u64),
            dodge_delay: Duration::from_millis(s.dock.dodge_delay_ms.max(50) as u64),
            settings: RefCell::new(parts.settings),
            disposed: Cell::new(false),
            me: RefCell::new(Weak::new()),
            subs: RefCell::new(Vec::new()),
            menu_sub: Cell::new(0),
            started: Instant::now(),
            changed: Notifier::default(),
            edge_watch_changed: Notifier::default(),
            animation_needed: Notifier::default(),
            animation_stopped: Notifier::default(),
            request_reposition: Notifier::default(),
            items_changed: LocalEvent::default(),
        });
        *vm.me.borrow_mut() = Rc::downgrade(&vm);
        vm.update_material_surfaces();
        let w = Rc::downgrade(&vm);
        MaterialState::on_changed(move || {
            if let Some(vm) = w.upgrade() {
                vm.update_material_surfaces();
            }
        });
        let w = Rc::downgrade(&vm);
        ThemeManager::on_applied(move || {
            if let Some(vm) = w.upgrade() {
                vm.update_material_surfaces();
            }
        });
        vm.wire_events();
        vm.update_items();
        vm
    }

    fn wire_events(self: &Rc<Self>) {
        let ui = UiRef::new(self);
        let s = self.controller.items_changed().subscribe(move |_| {
            let ui = ui.clone();
            dispatch::post(move || {
                if let Some(vm) = ui.upgrade() {
                    vm.update_items();
                }
            });
        });
        self.subs.borrow_mut().push((0, s));
        let ui = UiRef::new(self);
        let s = self.controller.fullscreen_state_changed().subscribe(move |_| {
            let ui = ui.clone();
            dispatch::post(move || {
                if let Some(vm) = ui.upgrade() {
                    vm.reevaluate();
                }
            });
        });
        self.subs.borrow_mut().push((1, s));
        if let Some(v) = &self.visibility {
            // Medya peek zamanlayıcısı arka plandan tikler: arayüze taşınır.
            let ui = UiRef::new(self);
            let s = v.changed().subscribe(move |_| {
                let ui = ui.clone();
                dispatch::post(move || {
                    if let Some(vm) = ui.upgrade()
                        && !vm.disposed.get()
                    {
                        vm.reevaluate();
                    }
                });
            });
            self.subs.borrow_mut().push((2, s));
        }
        if let Some(m) = &self.media {
            let ui = UiRef::new(self);
            let s = m.changed().subscribe(move |info| {
                let (ui, info) = (ui.clone(), info.clone());
                dispatch::post(move || {
                    if let Some(vm) = ui.upgrade() {
                        vm.update_media_info(info.as_ref());
                    }
                });
            });
            self.subs.borrow_mut().push((3, s));
            self.update_media_info(m.current().as_ref());
        }
        let w = Rc::downgrade(self);
        self.menu_sub.set(context_menu_state::subscribe(move || {
            if let Some(vm) = w.upgrade() {
                vm.reevaluate();
            }
        }));
        let tray = self.tray.borrow().clone();
        if let Some(t) = tray {
            self.hook_tray(&t);
        }
    }

    fn hook_tray(self: &Rc<Self>, tray: &Arc<dyn ITrayService>) {
        let ui = UiRef::new(self);
        let open = Arc::clone(&self.tray_popup_open);
        let queued = Arc::clone(&self.tray_sync_queued);
        // Tepsi iş parçacığından gelir. Pencere kapalıyken arayüze dönülmez: açılırken liste baştan okunur. Sık değişen
        // simgelerde iş biriktirilmez: bekleyen tek bir eşitleme yeter.
        let s = tray.icons_changed().subscribe(move |_| {
            if !open.load(Ordering::Acquire) || queued.swap(true, Ordering::AcqRel) {
                return;
            }
            let (ui, queued) = (ui.clone(), Arc::clone(&queued));
            dispatch::post(move || {
                queued.store(false, Ordering::Release);
                if let Some(vm) = ui.upgrade()
                    && vm.state.borrow().is_tray_popup_open
                {
                    vm.sync_tray_icons();
                }
            });
        });
        self.subs.borrow_mut().push((4, s));
    }

    fn weak(&self) -> Weak<Self> {
        self.me.borrow().clone()
    }

    pub fn state(&self) -> std::cell::Ref<'_, DockState> {
        self.state.borrow()
    }

    pub fn items(&self) -> Vec<DockEntry> {
        self.items.borrow().clone()
    }

    pub fn tray_icons(&self) -> std::cell::Ref<'_, Vec<TrayIconEntry>> {
        self.tray_icons.borrow()
    }

    pub fn settings(&self) -> AppSettings {
        self.settings.borrow().borrow().clone()
    }

    pub fn controller(&self) -> &Arc<DockController> {
        &self.controller
    }

    pub fn presentation(&self) -> &Rc<DockPresentationStyle> {
        &self.presentation
    }

    fn dock_settings<R>(&self, f: impl FnOnce(&koy_core::settings::DockSettings) -> R) -> R {
        let s = self.settings.borrow();
        let s = s.borrow();
        f(&s.dock)
    }

    fn notify(&self) {
        self.changed.notify();
    }

    // ---- Hareket ----

    /// true iken kayma, solma ve büyüme yayları anında değer alır (Motion kapalı ya da pil tasarrufu).
    pub fn reduce_motion(&self) -> bool {
        self.p.borrow().battery_reduce_motion || !Motion::enabled()
    }

    pub fn set_reduce_motion(&self, value: bool) {
        self.p.borrow_mut().battery_reduce_motion = value;
    }

    fn time_scale() -> f64 {
        let s = Motion::scale();
        if s > 0.0 { 1.0 / s } else { 1.0 }
    }

    fn magnify_animates(&self) -> bool {
        !self.reduce_motion() && Motion::enabled_for(MotionKind::DockMagnify)
    }

    fn request_animation(&self) {
        self.p.borrow_mut().is_animating = true;
        self.animation_needed.notify();
    }

    // ---- Yerleşim türevleri ----

    pub fn edge(&self) -> DockEdge {
        self.dock_settings(|d| d.edge)
    }

    pub fn is_vertical(&self) -> bool {
        matches!(self.edge(), DockEdge::Left | DockEdge::Right)
    }

    pub fn length_percent(&self) -> i32 {
        self.dock_settings(|d| d.length_percent)
    }

    pub fn edge_offset_dip(&self) -> f64 {
        self.dock_settings(|d| d.edge_offset_dip)
    }

    pub fn inner_padding(&self) -> f64 {
        DockLayoutLimits::clamp_inner_padding_dip(self.dock_settings(|d| d.inner_padding_dip))
    }

    pub fn slot_margin(&self) -> Thickness {
        let spacing = DockLayoutLimits::clamp_icon_spacing_dip(self.dock_settings(|d| d.icon_spacing_dip));
        if self.is_vertical() { (0.0, spacing / 2.0, 0.0, spacing / 2.0) } else { (spacing / 2.0, 0.0, spacing / 2.0, 0.0) }
    }

    /// Önizleme kutucuğunun öğeye göre kayması (x, y).
    pub fn preview_offset(&self) -> (f64, f64) {
        match self.edge() {
            DockEdge::Top => (0.0, 10.0),
            DockEdge::Left => (10.0, 0.0),
            DockEdge::Right => (-10.0, 0.0),
            DockEdge::Bottom => (0.0, -10.0),
        }
    }

    pub fn separator_size(&self) -> (f64, f64) {
        if self.is_vertical() { (24.0, 1.0) } else { (1.0, 24.0) }
    }

    pub fn separator_margin(&self) -> Thickness {
        if self.is_vertical() { (0.0, 4.0, 0.0, 4.0) } else { (4.0, 0.0, 4.0, 0.0) }
    }

    pub fn dodge_handle_size(&self) -> (f64, f64) {
        let (w, h) = (DockPositionResolver::DODGE_HANDLE_WIDTH_DIP, DockPositionResolver::DODGE_HANDLE_HEIGHT_DIP);
        if self.is_vertical() { (h, w) } else { (w, h) }
    }

    pub fn dodge_handle_margin(&self) -> Thickness {
        let m = DockPositionResolver::DODGE_HANDLE_BOTTOM_MARGIN_DIP;
        match self.edge() {
            DockEdge::Top => (0.0, m, 0.0, 0.0),
            DockEdge::Left => (m, 0.0, 0.0, 0.0),
            DockEdge::Right => (0.0, 0.0, m, 0.0),
            DockEdge::Bottom => (0.0, 0.0, 0.0, m),
        }
    }

    pub fn full_width_border_thickness(&self) -> Thickness {
        match self.edge() {
            DockEdge::Top => (0.0, 0.0, 0.0, 1.0),
            DockEdge::Left => (0.0, 0.0, 1.0, 0.0),
            DockEdge::Right => (1.0, 0.0, 0.0, 0.0),
            DockEdge::Bottom => (0.0, 1.0, 0.0, 0.0),
        }
    }

    pub fn full_width_padding(&self) -> Thickness {
        if self.is_vertical() { (0.0, 12.0, 0.0, 12.0) } else { (12.0, 0.0, 12.0, 0.0) }
    }

    /// Stile göre geçerli yakınlık modu: tam genişlikte yalnız Yer ayır ve Hep gizli.
    pub fn effective_proximity(&self) -> DockProximityMode {
        self.dock_settings(|d| DockProximityMapping::effective(d.style, d.proximity))
    }

    pub fn icon_alignment(&self) -> DockIconAlignment {
        self.dock_settings(|d| d.icon_alignment)
    }

    /// Tepsi düğmesi yalnız tepsi penceresi gerçekten çalışıyorsa görünür.
    pub fn has_tray(&self) -> bool {
        self.tray.borrow().as_ref().is_some_and(|t| t.is_running())
    }

    pub fn show_recents(&self) -> bool {
        self.dock_settings(|d| d.show_recents) && self.recent_files.is_some()
    }

    fn right_section(&self) -> DockRightSection {
        let show_folders = self.state.borrow().show_folders;
        let small = self.dock_settings(|d| d.small_tray);
        DockRightSection::resolve(show_folders, self.show_recents(), self.has_tray(), small)
    }

    /// Uygulamalarla Koy menüsü arasındaki ilk ayırıcı.
    pub fn show_right_section(&self) -> bool {
        self.right_section().show_first_separator
    }

    pub fn show_full_tray(&self) -> bool {
        self.right_section().show_full_tray
    }

    pub fn show_small_tray(&self) -> bool {
        self.right_section().show_small_tray
    }

    pub fn small_tray_width(&self) -> f64 {
        DockRightSection::SMALL_TRAY_WIDTH_DIP
    }

    pub fn needs_edge_watch(&self) -> bool {
        self.p.borrow().needs_edge_watch
    }

    pub fn is_reordering(&self) -> bool {
        self.p.borrow().is_reordering
    }

    // ---- Malzeme ----

    /// Tam genişlik ve kapsül yüzeyi: etkin malzemenin opaklığında (kapsül en çok %90).
    fn update_material_surfaces(&self) {
        let level = {
            let s = self.settings.borrow();
            let s = s.borrow();
            MaterialState::for_dock(&s)
        };
        let alpha = MaterialSetting::layered_alpha(level);
        let full = read_color("KoyDockSurface", DEFAULT_FULL_WIDTH);
        let capsule = read_color("KoyDockCapsuleSurface", DEFAULT_CAPSULE);
        let rgb = |c: u32, a: u8| slint::Color::from_argb_u8(a, (c >> 16) as u8, (c >> 8) as u8, c as u8);
        {
            let mut st = self.state.borrow_mut();
            st.full_width_surface = rgb(full, alpha);
            st.capsule_surface = rgb(capsule, alpha.min(0xE6));
        }
        self.notify();
    }

    // ---- Simge değişimi ----

    fn attach_app_icon(&self, app: &mut DockAppEntry) {
        let mut icons = self.app_icons.borrow_mut();
        if !icons.is_active() {
            return;
        }
        let custom = icons.resolve(&app.model);
        let has_override = icons.has_override(&app.model);
        if custom.is_some() || has_override {
            app.apply_app_icon(custom, has_override);
        }
    }

    /// "Simgeyi değiştir...": PNG/ICO seçtirir, doğrulanmış yolu bütün anahtarlarıyla kaydeder, dock yeniden kurulur.
    pub fn change_app_icon(&self, app: &AppRef, owner: Option<isize>) {
        let keys = {
            let a = app.borrow();
            AppIconKey::all(a.model.app_user_model_id.as_deref(), Some(&a.model.id))
        };
        if keys.is_empty() {
            return;
        }
        let Some(file) = crate::infra::file_dialog::pick_file(owner, &Strings::get("DockItemIconPickTitle"), &Strings::get("DockItemIconPickFilter"), true)
        else {
            return;
        };
        // Dosya, ayara yazılmadan önce gerçekten çözülebiliyor olmalı; bozuk dosya sessizce yok sayılır.
        if app_icons::load_file(&file).is_none() {
            return;
        }
        let mut overrides: ValueDictionary<String> = self.settings().appearance.app_icon_overrides.clone();
        let mut accepted = false;
        for key in &keys {
            accepted |= AppIconOverrides::try_set(&mut overrides, key, &file);
        }
        if accepted {
            dock_settings_store::save_app_icon_overrides(overrides);
            hooks::reload_dock();
        }
    }

    /// "Simgeyi sıfırla": öğenin bütün anahtarlarındaki değişimi siler.
    pub fn reset_app_icon(&self, app: &AppRef) {
        let mut overrides: ValueDictionary<String> = self.settings().appearance.app_icon_overrides.clone();
        let a = app.borrow();
        for key in AppIconKey::all(a.model.app_user_model_id.as_deref(), Some(&a.model.id)) {
            AppIconOverrides::reset(&mut overrides, &key);
        }
        drop(a);
        dock_settings_store::save_app_icon_overrides(overrides);
        hooks::reload_dock();
    }

    /// Ayar nesnesini değiştirir (yerinde ayar uygulaması): okuyucular aynı nesneyi görür.
    pub fn adopt_settings(&self, settings: SharedSettings) {
        *self.settings.borrow_mut() = settings;
        let vertical = self.is_vertical();
        for e in self.items.borrow().iter() {
            e.with_motion(|m| m.is_vertical = vertical);
        }
        self.recompute_fit();
        self.reevaluate();
        self.notify();
    }

    /// Dock'un monitörü değişti: "yalnız bu monitörün pencereleri" süzgeci yeni sınırlarla uygulanır.
    pub fn set_monitor_bounds(&self, bounds: WindowRect) {
        if self.p.borrow().monitor_bounds == Some(bounds) {
            return;
        }
        self.p.borrow_mut().monitor_bounds = Some(bounds);
        if self.dock_settings(|d| d.window_monitor_scope) == DockWindowMonitorScope::CurrentMonitor {
            self.update_items();
        }
    }

    fn visible_models(&self) -> Vec<DockItem> {
        let bounds = self.p.borrow().monitor_bounds;
        match bounds {
            Some(b) if self.dock_settings(|d| d.window_monitor_scope) == DockWindowMonitorScope::CurrentMonitor => {
                DockMonitorFilter::apply(&self.controller.items(), b, self.controller.foreground_handle())
            }
            _ => self.controller.items(),
        }
    }

    // ---- Tepsi ----

    /// Tepsi servisini takar ya da çıkarır (görev çubuğu gizlenince / geri gelince).
    pub fn set_tray_service(self: &Rc<Self>, service: Option<Arc<dyn ITrayService>>) {
        let same = match (&*self.tray.borrow(), &service) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.unhook_tray();
        *self.tray.borrow_mut() = service.clone();
        if let Some(t) = &service {
            self.hook_tray(t);
        }
        self.set_tray_popup_open(false);
        self.tray_icons.borrow_mut().clear();
        self.state.borrow_mut().has_tray_icons = false;
        self.recompute_fit();
        self.notify();
    }

    fn unhook_tray(&self) {
        let mut subs = self.subs.borrow_mut();
        if let Some(i) = subs.iter().position(|(k, _)| *k == 4) {
            let (_, s) = subs.remove(i);
            if let Some(t) = self.tray.borrow().as_ref() {
                t.icons_changed().unsubscribe(s);
            }
        }
    }

    pub fn toggle_tray(&self) {
        let Some(tray) = self.tray.borrow().clone() else { return };
        if self.state.borrow().is_tray_popup_open {
            self.set_tray_popup_open(false);
            return;
        }
        self.close_preview_now();
        tray.remove_dead_icons();
        self.sync_tray_icons();
        self.set_tray_popup_open(true);
    }

    /// Tepsi simgesine tıklandı: uygulamaya Windows görev çubuğunun göndereceği iletileri iletir.
    pub fn send_tray_mouse(&self, index: usize, action: TrayMouseAction, dock_rect: WindowRect) {
        let Some(tray) = self.tray.borrow().clone() else { return };
        let Some(key) = self.tray_icons.borrow().get(index).map(|i| i.key.clone()) else { return };
        let (x, y) = CursorHelper::get_position().unwrap_or((0, 0));
        // Çıpa dock'un üzerine düşerse uygulamanın menüsü dock'un içine açılırdı: dock'un üst kenarının üstüne çekilir.
        let (x, y) = TrayMenuAnchor::clamp_above_dock(x, y, dock_rect);
        tray.send_mouse(&key, action, x, y);
        if action == TrayMouseAction::LeftClick {
            // Bazı uygulamalar çift tıklama bekler; pencere çift tıklama süresi kadar açık kalır.
            let w = self.weak();
            self.tray_close_timer.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(CursorHelper::double_click_time_ms() as u64),
                move || {
                    if let Some(vm) = w.upgrade() {
                        vm.set_tray_popup_open(false);
                    }
                },
            );
        } else if action != TrayMouseAction::RightClick {
            // Çift tık uygulamanın penceresini açar; tepsi penceresi önünü kapatmasın. Sağ tıkta açık kalır.
            self.set_tray_popup_open(false);
        }
    }

    pub fn set_tray_popup_open(&self, value: bool) {
        let changed = {
            let mut st = self.state.borrow_mut();
            let c = st.is_tray_popup_open != value;
            st.is_tray_popup_open = value;
            c
        };
        self.tray_popup_open.store(value, Ordering::Release);
        if !changed {
            return;
        }
        if value {
            self.tray_clear_timer.stop();
        } else {
            self.tray_close_timer.stop();
            // Kapanış animasyonu süresince simgeler görünür kalır; sonra bırakılır (kapalıyken bellekte tutulmaz).
            let w = self.weak();
            self.tray_clear_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(500), move || {
                if let Some(vm) = w.upgrade()
                    && !vm.state.borrow().is_tray_popup_open
                {
                    vm.tray_icons.borrow_mut().clear();
                    vm.tray_exe_by_owner.borrow_mut().clear();
                    vm.notify();
                }
            });
        }
        self.reevaluate();
        self.notify();
    }

    fn tray_exe_name(&self, owner: isize) -> String {
        if let Some(n) = self.tray_exe_by_owner.borrow().get(&owner) {
            return n.clone();
        }
        let name = tray_icons::app_name_for_owner(owner);
        self.tray_exe_by_owner.borrow_mut().insert(owner, name.clone());
        name
    }

    /// Uygulama başına görünürlük: Gizli olanlar tepsi penceresine hiç girmez.
    fn sync_tray_icons(&self) {
        let Some(tray) = self.tray.borrow().clone() else { return };
        let (per_app, new_default) = {
            let s = self.settings.borrow();
            let s = s.borrow();
            (s.dock.tray_app_visibility.iter().map(|(k, v)| (k.clone(), *v)).collect::<HashMap<String, TrayIconVisibility>>(), s.dock.new_tray_icon_visibility)
        };
        let models: Vec<_> = tray
            .get_visible_icons()
            .into_iter()
            .filter(|m| TrayIconVisibilityPolicy::resolve(Some(&self.tray_exe_name(m.owner_hwnd)), &per_app, new_default) != TrayIconVisibility::Hidden)
            .collect();
        let mut icons = self.tray_icons.borrow_mut();
        icons.retain(|i| models.iter().any(|m| m.key == i.key));
        for (target, model) in models.iter().enumerate() {
            let existing = icons.iter().position(|i| i.key == model.key);
            let idx = match existing {
                Some(i) => {
                    if i != target {
                        let e = icons.remove(i);
                        let at = target.min(icons.len());
                        icons.insert(at, e);
                    }
                    target.min(icons.len() - 1)
                }
                None => {
                    let e = TrayIconEntry::new(model, self.tray_exe_name(model.owner_hwnd));
                    let at = target.min(icons.len());
                    icons.insert(at, e);
                    at
                }
            };
            let t = Arc::clone(&tray);
            icons[idx].update(model, move |k| t.get_icon_image(k));
        }
        let has = !icons.is_empty();
        drop(icons);
        self.state.borrow_mut().has_tray_icons = has;
        self.notify();
    }

    // ---- Komutlar ----

    pub fn start_menu(&self) {
        self.shell.send_win_key();
    }

    pub fn system_tray(&self) {
        self.shell.toggle_system_tray();
    }

    /// Sağ alt Koy menüsü; işleyici yoksa Ayarlar.
    pub fn open_koy_menu(&self) {
        if !hooks::open_koy_menu() {
            self.open_settings();
        }
    }

    pub fn open_downloads(&self) {
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let path = std::path::Path::new(&home).join("Downloads");
            if path.is_dir() {
                let _ = self.shell.launch(&path.to_string_lossy(), None);
            }
        }
    }

    /// Çentik müzik paneli kısayolu.
    pub fn open_notch_music(&self) {
        if let Some(v) = &self.visibility {
            v.on_media_activity();
        }
    }

    pub fn open_settings(&self) {
        hooks::open_settings(2);
    }

    // ---- Son kullanılanlar ----

    pub fn toggle_recents(&self) {
        let Some(service) = self.recent_files.clone() else { return };
        if self.state.borrow().is_recents_open {
            self.set_recents_open(false);
            return;
        }
        self.close_preview_now();
        let ui = UiRef::new(&self.me.borrow().upgrade().expect("dock görünüm modeli"));
        std::thread::spawn(move || {
            let items = service.get_recent(RecentFilesPolicy::MAX_ITEMS);
            dispatch::post(move || {
                if let Some(vm) = ui.upgrade() {
                    vm.state.borrow_mut().recent_files = items;
                    vm.set_recents_open(true);
                }
            });
        });
    }

    /// Listeden bir dosya seçildi: Windows ilişkili uygulamayla açar, liste kapanır.
    pub fn open_recent(&self, index: usize) {
        let path = self.state.borrow().recent_files.get(index).map(|f| f.open_path.clone());
        self.set_recents_open(false);
        if let Some(p) = path {
            let _ = self.shell.launch(&p, None);
        }
    }

    pub fn set_recents_open(&self, value: bool) {
        {
            let mut st = self.state.borrow_mut();
            st.is_recents_open = value;
            if !value {
                st.recent_files.clear();
            }
        }
        self.reevaluate();
        self.notify();
    }

    // ---- Sığdırma ve konum ----

    pub fn set_monitor(&self, monitor: Option<DisplayMonitor>) {
        self.p.borrow_mut().current_monitor = monitor;
        self.recompute_fit();
        self.reevaluate();
    }

    /// Tam genişlikte orta sütunun genişliği değişti (görünüm bildirir).
    pub fn report_center_width(&self, width: f64) {
        if (width - self.p.borrow().center_width).abs() < 0.5 {
            return;
        }
        self.p.borrow_mut().center_width = width;
        self.recompute_fit();
    }

    /// Çok uygulama açıkken simgeleri ekrana sığdırır (Core: DockFitMath). En küçük simge 24 px; sonra şerit kırpılır.
    fn recompute_fit(&self) {
        let (style, spacing) = self.dock_settings(|d| (d.style, d.icon_spacing_dip));
        let count = self.items.borrow().len() as i32;
        let preferred = self.p.borrow().preferred_icon_size;
        let (icon, max_width) = if style == DockStyleMode::FullWidth {
            let center = self.p.borrow().center_width;
            if center <= 0.0 {
                return;
            }
            (DockFitMath::fit_icon_size(preferred, count, 0.0, center, spacing), center)
        } else {
            let vertical = self.is_vertical();
            let monitor_dim = {
                let p = self.p.borrow();
                match &p.current_monitor {
                    Some(m) if vertical && m.height_dip() > 0.0 => m.height_dip(),
                    Some(m) if !vertical && m.width_dip() > 0.0 => m.width_dip(),
                    _ if vertical => 1080.0,
                    _ => 1920.0,
                }
            };
            let available = monitor_dim - CAPSULE_WINDOW_SIDE_MARGINS;
            let right = self.right_section();
            let st = self.state.borrow();
            let other_slots = st.show_start_button as i32
                + st.show_folders as i32
                + self.show_recents() as i32
                + right.show_full_tray as i32
                + 1; // Koy menüsü
            drop(st);
            let fixed = CAPSULE_PADDING_AND_BORDER
                + CAPSULE_SEPARATOR_WIDTH
                + if right.show_first_separator { CAPSULE_SEPARATOR_WIDTH } else { 0.0 }
                + if right.show_small_tray { DockRightSection::SMALL_TRAY_WIDTH_DIP + 1.0 } else { 0.0 };
            let icon = DockFitMath::fit_icon_size(preferred, count + other_slots, fixed, available, spacing);
            let max = DockFitMath::strip_max_width(available, fixed + other_slots as f64 * DockFitMath::slot_width(icon, spacing));
            (icon, max)
        };
        let item_size = DockFitMath::item_size_for(icon);
        let changed_size = {
            let mut st = self.state.borrow_mut();
            st.items_max_width = max_width;
            st.is_strip_clipped = count as f64 * DockFitMath::slot_width(icon, spacing) > max_width + 0.5;
            let changed = (icon - st.icon_size).abs() >= 0.01;
            if changed {
                st.icon_size = icon;
                st.item_size = item_size;
            }
            changed
        };
        if changed_size {
            // Simgeler yeni boyutta yeniden çözülür (sığdırma küçülttüyse ölçeklenmiş bulanık çizim olmasın).
            super::entries::set_icon_pixels(crate::infra::image_decode::pixels_for(icon));
            for e in self.items.borrow().iter() {
                if let DockEntry::App(a) = e {
                    a.borrow_mut().reload_icon();
                }
                e.with_motion(|m| {
                    m.icon_size = icon;
                    m.item_size = item_size;
                });
                if let DockEntry::Folder(f) = e {
                    for a in f.borrow().items.iter() {
                        let mut a = a.borrow_mut();
                        a.motion.icon_size = icon;
                        a.motion.item_size = item_size;
                    }
                }
            }
        }
        self.notify();
    }

    pub fn update_foreground_window(&self, bounds: WindowRect, is_maximized: bool, is_fullscreen: bool) {
        {
            let mut p = self.p.borrow_mut();
            p.foreground_bounds = bounds;
            p.foreground_maximized = is_maximized;
            p.foreground_fullscreen = is_fullscreen;
        }
        self.reevaluate();
    }

    pub fn set_mouse_at_bottom(&self, at_bottom: bool) {
        if self.p.borrow().mouse_at_bottom_edge == at_bottom {
            return;
        }
        self.p.borrow_mut().mouse_at_bottom_edge = at_bottom;
        if at_bottom && self.effective_proximity() == DockProximityMode::Dodge {
            let w = self.weak();
            self.dodge_hover_timer.start(slint::TimerMode::SingleShot, self.dodge_delay, move || {
                if let Some(vm) = w.upgrade() {
                    vm.reevaluate();
                }
            });
        } else {
            self.dodge_hover_timer.stop();
            self.reevaluate();
        }
    }

    pub fn set_shell_flyout_open(&self, open: bool) {
        if self.p.borrow().shell_flyout_open == open {
            return;
        }
        self.p.borrow_mut().shell_flyout_open = open;
        self.reevaluate();
    }

    pub fn set_mouse_over_dock(&self, over: bool) {
        self.p.borrow_mut().mouse_over_dock = over;
        self.reevaluate();
    }

    fn snap_clock(&self) -> TimeSpan {
        TimeSpan::from_seconds_f64(self.started.elapsed().as_secs_f64())
    }

    fn stabilize_snap(&self, resolved: DockResolvedState) -> DockResolvedState {
        let proximity = self.effective_proximity();
        let stable = self.p.borrow().stable_resolved.clone();
        let Some(stable) = stable.filter(|_| !resolved.is_full_width && !resolved.is_hidden && proximity == DockProximityMode::DockDown) else {
            self.snap_debounce.borrow_mut().reset(resolved.is_snapped);
            self.snap_timer.stop();
            if !resolved.is_hidden {
                self.p.borrow_mut().stable_resolved = Some(resolved.clone());
            }
            return resolved;
        };
        let now = self.snap_clock();
        let effective = self.snap_debounce.borrow_mut().update(resolved.is_snapped, now);
        if effective == resolved.is_snapped {
            self.snap_timer.stop();
            self.p.borrow_mut().stable_resolved = Some(resolved.clone());
            return resolved;
        }
        // Değişim bekliyor: önceki geometri korunur; kalan süre sonra tek atımlık zamanlayıcı yeniden değerlendirir.
        let remaining = self.snap_debounce.borrow().remaining(now).total_milliseconds().max(0.0) as u64 + 10;
        let w = self.weak();
        self.snap_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(remaining), move || {
            if let Some(vm) = w.upgrade()
                && !vm.disposed.get()
            {
                vm.reevaluate();
            }
        });
        DockResolvedState {
            is_snapped: stable.is_snapped,
            height_dip: stable.height_dip,
            top_dip: stable.top_dip,
            corner_radius_top_left: stable.corner_radius_top_left,
            corner_radius_top_right: stable.corner_radius_top_right,
            corner_radius_bottom_right: stable.corner_radius_bottom_right,
            corner_radius_bottom_left: stable.corner_radius_bottom_left,
            ..resolved
        }
    }

    fn reevaluate(&self) {
        self.evaluate_position(0.0);
    }

    fn has_open_folder(&self) -> bool {
        self.items.borrow().iter().any(|e| matches!(e, DockEntry::Folder(f) if f.borrow().is_folder_panel_open))
    }

    /// Konumu, görünürlüğü ve kipi çözer; hedefleri yaylara verir.
    pub fn evaluate_position(&self, content_width: f64) {
        if content_width > 0.0 {
            self.p.borrow_mut().last_content_width = content_width;
        }
        let d = self.settings().dock;
        let proximity = self.effective_proximity();
        let has_open_folder = self.has_open_folder();
        let resolved = {
            let p = self.p.borrow();
            let st = self.state.borrow();
            let effective_mouse_over = p.mouse_over_dock
                || p.mouse_over_popup
                || p.shell_flyout_open
                || st.is_preview_popup_open
                || st.is_tray_popup_open
                || st.is_recents_open
                || context_menu_state::has_active_menu()
                || has_open_folder;
            DockPositionResolver::resolve(
                p.current_monitor.as_ref(),
                d.style,
                proximity,
                p.last_content_width,
                p.foreground_bounds,
                p.foreground_maximized,
                p.mouse_at_bottom_edge,
                effective_mouse_over,
                p.foreground_fullscreen,
                d.hide_in_fullscreen,
                d.edge_reveal,
                d.edge,
                d.length_percent,
                d.edge_offset_dip,
                d.inner_padding_dip,
            )
        };
        let resolved = self.stabilize_snap(resolved);
        let vertical = self.is_vertical();
        let fg_fullscreen = self.p.borrow().foreground_fullscreen;
        // Tam ekran bu monitör için hesaplanır (yan ekrandaki oyun bu ekrandaki dock'u gizlemez).
        let blocked_by_fullscreen = d.hide_in_fullscreen && fg_fullscreen;
        let mut should_show = !resolved.is_hidden && !blocked_by_fullscreen;
        if let Some(v) = &self.visibility
            && proximity != DockProximityMode::ReserveSpace
        {
            should_show = should_show && v.is_visible();
        }
        {
            let mut st = self.state.borrow_mut();
            st.is_full_width_mode = resolved.is_full_width;
            st.is_snapped = resolved.is_snapped;
            st.is_handle_visible = resolved.is_handle_visible;
            st.dock_height = if vertical { resolved.width_dip } else { resolved.height_dip };
            st.capsule_width = vertical.then_some(st.dock_height);
            st.capsule_height = (!vertical).then_some(st.dock_height);
            if resolved.is_full_width {
                st.dock_margin = (0.0, 0.0, 0.0, 0.0);
                st.capsule_corner_radius = (0.0, 0.0, 0.0, 0.0);
            } else if resolved.is_snapped {
                st.dock_margin = match d.edge {
                    DockEdge::Top => (12.0, 0.0, 12.0, 12.0),
                    DockEdge::Left => (0.0, 12.0, 12.0, 12.0),
                    DockEdge::Right => (12.0, 12.0, 0.0, 12.0),
                    DockEdge::Bottom => (12.0, 12.0, 12.0, 0.0),
                };
                st.capsule_corner_radius = (
                    resolved.corner_radius_top_left,
                    resolved.corner_radius_top_right,
                    resolved.corner_radius_bottom_right,
                    resolved.corner_radius_bottom_left,
                );
            } else {
                st.dock_margin = match d.edge {
                    DockEdge::Top => (12.0, 10.0, 12.0, 12.0),
                    DockEdge::Left => (10.0, 12.0, 12.0, 12.0),
                    DockEdge::Right => (12.0, 12.0, 10.0, 12.0),
                    DockEdge::Bottom => (12.0, 12.0, 12.0, 10.0),
                };
                st.capsule_corner_radius = (18.0, 18.0, 18.0, 18.0);
            }
            st.is_visible = should_show;
            // Çekil tutamağı zaten gizli dock'u işaret eder; ikisi üst üste çizilmez.
            st.is_edge_line_visible = !resolved.is_handle_visible
                && DockPresentationMath::shows_hidden_edge_line(d.hidden_edge_line, !should_show, blocked_by_fullscreen || fg_fullscreen);
        }
        if let Some(s) = &self.status {
            s.set_active(should_show);
        }
        self.sync_clock_timer();
        // Fare izleme yalnız dock gizliyken (ya da kenardan açıldıysa) ve tam ekranda değilken gerekir.
        let at_edge = self.p.borrow().mouse_at_bottom_edge;
        let needs_edge_watch = (!should_show || at_edge) && !blocked_by_fullscreen && !fg_fullscreen;
        if needs_edge_watch != self.p.borrow().needs_edge_watch {
            self.p.borrow_mut().needs_edge_watch = needs_edge_watch;
            self.edge_watch_changed.notify();
        }
        let (target_x, target_y) = if should_show || resolved.is_hidden {
            (resolved.translation_x, resolved.translation_y)
        } else {
            let thickness = if resolved.is_full_width {
                DockPositionResolver::FULL_WIDTH_HEIGHT_DIP
            } else if resolved.is_snapped {
                DockPositionResolver::SNAPPED_HEIGHT_DIP
            } else {
                DockPositionResolver::FLOATING_HEIGHT_DIP
            };
            let offset = if resolved.is_full_width || resolved.is_snapped {
                d.edge_offset_dip
            } else {
                DockPositionResolver::FLOATING_BOTTOM_MARGIN_DIP + d.edge_offset_dip
            };
            let shift = thickness + offset + 10.0;
            match d.edge {
                DockEdge::Top => (0.0, -shift),
                DockEdge::Left => (-shift, 0.0),
                DockEdge::Right => (shift, 0.0),
                DockEdge::Bottom => (0.0, shift),
            }
        };
        let target_opacity = if should_show { resolved.opacity } else { 0.0 };
        {
            let mut sp = self.springs.borrow_mut();
            if self.reduce_motion() {
                sp.translate_x.snap_to(target_x);
                sp.translate_y.snap_to(target_y);
                sp.opacity.snap_to(target_opacity);
            } else {
                sp.translate_x.set_target(target_x);
                sp.translate_y.set_target(target_y);
                sp.opacity.set_target(target_opacity);
            }
        }
        self.request_animation();
        self.request_reposition.notify();
        self.notify();
    }

    // ---- Önizleme ----

    fn find_app(&self, id: &str) -> Option<AppRef> {
        for e in self.items.borrow().iter() {
            match e {
                DockEntry::App(a) if a.borrow().id().eq_ignore_ascii_case(id) => return Some(Rc::clone(a)),
                DockEntry::Folder(f) => {
                    if let Some(a) = f.borrow().items.iter().find(|a| a.borrow().id().eq_ignore_ascii_case(id)) {
                        return Some(Rc::clone(a));
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub fn preview_item(&self) -> Option<AppRef> {
        let id = self.state.borrow().preview_item_id.clone()?;
        self.find_app(&id)
    }

    fn set_preview(&self, open: bool, item: Option<String>) {
        let mut st = self.state.borrow_mut();
        st.is_preview_popup_open = open;
        st.preview_item_id = item;
    }

    /// Fare bir uygulama simgesine girdi: bekleme süresi sonra önizleme açılır (açıksa hemen bu öğeye geçer).
    pub fn on_item_mouse_enter(&self, item: &AppRef) {
        self.preview_close_timer.stop();
        let (id, has_windows) = {
            let a = item.borrow();
            (a.id().to_string(), a.has_open_windows())
        };
        if !self.dock_settings(|d| d.thumbnail_preview) || !has_windows {
            self.thumbnail_hover_timer.stop();
            self.set_preview(false, None);
            self.p.borrow_mut().pending_preview_id = None;
            self.notify();
            return;
        }
        self.p.borrow_mut().pending_preview_id = Some(id.clone());
        if self.state.borrow().is_preview_popup_open {
            self.thumbnail_hover_timer.stop();
            self.set_preview(true, Some(id));
            self.notify();
            return;
        }
        let w = self.weak();
        self.thumbnail_hover_timer.start(slint::TimerMode::SingleShot, self.thumbnail_delay, move || {
            if let Some(vm) = w.upgrade() {
                vm.on_thumbnail_timer();
            }
        });
    }

    /// Sol tık "önizlemeleri göster" iken: beklemeden açar. Önizleme kapalıysa ya da pencere yoksa false.
    pub fn try_show_preview_now(&self, item: &AppRef) -> bool {
        let (id, has_windows) = {
            let a = item.borrow();
            (a.id().to_string(), a.has_open_windows())
        };
        if !self.dock_settings(|d| d.thumbnail_preview) || !has_windows {
            return false;
        }
        self.thumbnail_hover_timer.stop();
        self.preview_close_timer.stop();
        self.p.borrow_mut().pending_preview_id = Some(id.clone());
        self.set_preview(true, Some(id));
        self.notify();
        true
    }

    pub fn on_item_mouse_leave(&self) {
        self.thumbnail_hover_timer.stop();
        self.p.borrow_mut().pending_preview_id = None;
        if self.state.borrow().is_preview_popup_open {
            self.start_preview_close();
        } else {
            self.state.borrow_mut().preview_item_id = None;
            self.notify();
        }
    }

    fn start_preview_close(&self) {
        let w = self.weak();
        self.preview_close_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(200), move || {
            if let Some(vm) = w.upgrade() {
                vm.p.borrow_mut().mouse_over_popup = false;
                vm.p.borrow_mut().pending_preview_id = None;
                vm.set_preview(false, None);
                vm.reevaluate();
            }
        });
    }

    pub fn on_popup_mouse_enter(&self) {
        self.p.borrow_mut().mouse_over_popup = true;
        self.preview_close_timer.stop();
        self.reevaluate();
    }

    pub fn on_popup_mouse_leave(&self) {
        self.p.borrow_mut().mouse_over_popup = false;
        self.start_preview_close();
    }

    pub fn close_preview_now(&self) {
        self.thumbnail_hover_timer.stop();
        self.preview_close_timer.stop();
        {
            let mut p = self.p.borrow_mut();
            p.pending_preview_id = None;
            p.mouse_over_popup = false;
        }
        self.set_preview(false, None);
        self.reevaluate();
    }

    fn on_thumbnail_timer(&self) {
        let pending = self.p.borrow().pending_preview_id.clone();
        if let Some(id) = pending
            && self.find_app(&id).is_some_and(|a| a.borrow().has_open_windows())
        {
            self.set_preview(true, Some(id));
            self.notify();
        }
    }

    // ---- Büyütme, sıralama, basma ----

    pub fn on_item_hover(&self, hovered_index: usize) {
        if !self.state.borrow().magnification_enabled || self.reduce_motion() {
            return;
        }
        // Tepe büyüme çubuğun yüksekliğine göre sınırlanır; komşu yarısı kadar büyür.
        let (dock_height, item_size) = {
            let st = self.state.borrow();
            (st.dock_height, st.item_size)
        };
        let peak = DockFitMath::hover_peak_scale(dock_height, item_size, HOVER_MAX_SCALE);
        let neighbor = 1.0 + (peak - 1.0) * 0.5;
        // Büyütme animasyonu anahtarı kapalıysa büyüme yine olur ama yaysız, tek karede.
        let animate = self.magnify_animates();
        for (i, e) in self.items.borrow().iter().enumerate() {
            let target = match i.abs_diff(hovered_index) {
                0 => peak,
                1 => neighbor,
                _ => 1.0,
            };
            e.with_motion(|m| if animate { m.set_target_scale(target) } else { m.snap_scale(target) });
        }
        self.request_animation();
    }

    /// Öğe sürüklenebilir mi: sabitli, sıralama destekleniyor ve en az iki sabitli var.
    pub fn can_start_reorder(&self, entry: &DockEntry) -> bool {
        !self.p.borrow().has_extras && entry.is_pinned() && self.controller.can_reorder_pinned() && self.pinned_count() > 1
    }

    pub fn pinned_count(&self) -> i32 {
        self.items.borrow().iter().filter(|e| e.is_pinned()).count() as i32
    }

    pub fn begin_reorder(&self) {
        self.p.borrow_mut().is_reordering = true;
        self.close_preview_now();
        self.on_hover_ended();
    }

    /// Sürükleme sırasında: öğe imleci izler, aradakiler hedef yuvaya göre bir yuva kayar.
    pub fn update_reorder(&self, dragged: &DockEntry, original_index: i32, drag_offset: f64, slot_width: f64) {
        let count = self.pinned_count();
        let target = DockReorderMath::target_index(count, original_index, drag_offset, slot_width);
        dragged.with_motion(|m| m.snap_offset(drag_offset));
        for (i, e) in self.items.borrow().iter().enumerate().take(count as usize) {
            if i as i32 != original_index {
                e.with_motion(|m| m.set_target_offset(DockReorderMath::shift_for(i as i32, original_index, target, slot_width)));
            }
        }
        self.request_animation();
    }

    /// Bırakıldı. Sıra değiştiyse koleksiyon yeni sıraya taşınır, her öğenin ekrandaki konumu korunacak biçimde kayması
    /// ayarlanır, sonra yay sıfıra çeker (zıplama olmaz). Sıra kalıcı yazılır.
    pub fn commit_reorder(&self, dragged: &DockEntry, original_index: i32, drag_offset: f64, slot_width: f64) {
        self.p.borrow_mut().is_reordering = false;
        let count = self.pinned_count();
        let target = DockReorderMath::target_index(count, original_index, drag_offset, slot_width);
        if target == original_index {
            for e in self.items.borrow().iter() {
                e.with_motion(|m| m.set_target_offset(0.0));
            }
        } else {
            let visual: Vec<f64> = self.items.borrow().iter().enumerate().map(|(i, e)| i as f64 * slot_width + e.with_motion(|m| m.offset())).collect();
            {
                let mut items = self.items.borrow_mut();
                let mut paired: Vec<(DockEntry, f64)> = items.drain(..).zip(visual).collect();
                let moved = paired.remove(original_index as usize);
                paired.insert(target as usize, moved);
                for (i, (e, v)) in paired.iter().enumerate() {
                    e.with_motion(|m| {
                        m.snap_offset(v - i as f64 * slot_width);
                        m.set_target_offset(0.0);
                    });
                }
                *items = paired.into_iter().map(|(e, _)| e).collect();
            }
            match dragged {
                DockEntry::App(a) => {
                    let id = a.borrow().id().to_string();
                    self.controller.move_pinned(&id, target);
                }
                DockEntry::Folder(f) => {
                    let id = f.borrow().id();
                    let (order, folders) = {
                        let s = self.settings();
                        (PinnedOrder::move_item(&s.dock_items.pinned_order, &id, target), s.dock_items.folders.to_vec())
                    };
                    self.save_pinned_order(&order, Some(&folders));
                }
                DockEntry::Extra(_) => {}
            }
        }
        self.request_animation();
        self.notify();
    }

    /// Sürükleme yarıda kesildi: her şey yerine döner.
    pub fn cancel_reorder(&self) {
        if !self.p.borrow().is_reordering {
            return;
        }
        self.p.borrow_mut().is_reordering = false;
        for e in self.items.borrow().iter() {
            e.with_motion(|m| m.set_target_offset(0.0));
        }
        self.request_animation();
    }

    /// Öğeye basıldı / bırakıldı: simge içe çöker ve koyulaşır, bırakınca yayla geri gelir.
    pub fn set_entry_pressed(&self, entry: &DockEntry, pressed: bool) {
        if entry.with_motion(|m| m.is_pressed) == pressed {
            return;
        }
        let animate = !self.reduce_motion();
        entry.with_motion(|m| m.set_pressed(pressed, animate));
        self.request_animation();
    }

    pub fn on_hover_ended(&self) {
        let animate = self.magnify_animates();
        for e in self.items.borrow().iter() {
            e.with_motion(|m| if animate { m.set_target_scale(1.0) } else { m.snap_scale(1.0) });
        }
        self.request_animation();
    }

    /// Kare adımı: yayları ilerletir; hepsi durunca `animation_stopped`.
    pub fn is_animating(&self) -> bool {
        self.p.borrow().is_animating
    }

    pub fn update_springs(&self, delta_seconds: f64) {
        if !self.p.borrow().is_animating {
            return;
        }
        let dt = delta_seconds * Self::time_scale();
        let moving = {
            let mut sp = self.springs.borrow_mut();
            let tx = sp.translate_x.step(dt);
            let ty = sp.translate_y.step(dt);
            let o = sp.opacity.step(dt);
            let mut st = self.state.borrow_mut();
            st.translation_x = sp.translate_x.value();
            st.translation_y = sp.translate_y.value();
            st.opacity = sp.opacity.value().clamp(0.0, 1.0);
            tx || ty || o
        };
        let mut items_moving = false;
        for e in self.items.borrow().iter() {
            e.with_motion(|m| {
                items_moving |= m.step_scale(dt);
                items_moving |= m.step_offset(dt);
                items_moving |= m.step_press(dt);
            });
        }
        self.notify();
        if !moving && !items_moving {
            self.p.borrow_mut().is_animating = false;
            self.animation_stopped.notify();
        }
    }

    // ---- Klasörler ----

    fn save_pinned_order(&self, order: &[String], folders: Option<&[DockFolder]>) {
        let s = self.settings.borrow();
        dock_settings_store::save_pinned_order(order, folders, Some(&mut s.borrow_mut()));
    }

    pub fn dissolve_folder(&self, folder: &Rc<RefCell<DockFolderEntry>>) {
        let id = folder.borrow().folder.id.clone();
        let s = self.settings();
        let (order, folders) = DockFolders::dissolve(&s.dock_items.pinned_order, &s.dock_items.folders, &id);
        self.save_pinned_order(&order, Some(&folders));
        self.update_items();
    }

    pub fn rename_folder(&self, folder: &Rc<RefCell<DockFolderEntry>>, new_name: &str) {
        let id = folder.borrow().folder.id.clone();
        let s = self.settings();
        let folders = DockFolders::rename(&s.dock_items.folders, &id, new_name);
        self.save_pinned_order(&s.dock_items.pinned_order, Some(&folders));
        folder.borrow_mut().title = new_name.to_string();
        self.notify();
    }

    pub fn remove_app_from_folder(&self, folder_id: &str, item_id: &str, insert_index: i32) {
        let s = self.settings();
        let order = &s.dock_items.pinned_order;
        let insert_index = if insert_index < 0 {
            let token = PinnedOrder::to_folder_token(&PinnedOrder::extract_folder_id(folder_id));
            match order.iter().position(|x| x.eq_ignore_ascii_case(&token)) {
                Some(i) => i as i32 + 1,
                None => order.len() as i32,
            }
        } else {
            insert_index
        };
        let (order, folders) = DockFolders::remove_from_folder(order, &s.dock_items.folders, folder_id, item_id, insert_index);
        self.save_pinned_order(&order, Some(&folders));
        self.update_items();
    }

    pub fn create_folder_from_drop(&self, dragged_id: &str, target_id: &str) {
        let s = self.settings();
        let name = DockFolders::next_default_name(&s.dock_items.folders, &Strings::get("DockFolder"));
        let (order, folders) = DockFolders::create_from_drop(&s.dock_items.pinned_order, &s.dock_items.folders, dragged_id, target_id, &name);
        self.save_pinned_order(&order, Some(&folders));
        self.update_items();
    }

    pub fn add_app_to_folder(&self, dragged_id: &str, folder_id: &str) {
        let s = self.settings();
        let (order, folders) = DockFolders::add_to_folder(&s.dock_items.pinned_order, &s.dock_items.folders, dragged_id, folder_id);
        self.save_pinned_order(&order, Some(&folders));
        self.update_items();
    }

    // ---- Şerit kurulumu ----

    /// Şeridi denetleyicinin öğelerinden, sabitli sıradan, klasörlerden ve ek öğelerden yeniden kurar (yerinde günceller).
    /// C#'ta `Dispatcher.InvokeAsync` ile ertelenir; burada da bir sonraki tura bırakılır.
    pub fn update_items(&self) {
        let w = self.weak();
        dispatch::post_local(move || {
            if let Some(vm) = w.upgrade()
                && !vm.disposed.get()
            {
                vm.update_items_now();
            }
        });
    }

    fn new_app(&self, model: DockItem) -> AppRef {
        let (icon, item, vertical) = {
            let st = self.state.borrow();
            (st.icon_size, st.item_size, self.is_vertical())
        };
        let mut a = DockAppEntry::new(model, Rc::clone(&self.presentation));
        a.motion.icon_size = icon;
        a.motion.item_size = item;
        a.motion.is_vertical = vertical;
        self.attach_app_icon(&mut a);
        Rc::new(RefCell::new(a))
    }

    fn update_items_now(&self) {
        crate::infra::trace::bump(&crate::infra::trace::DOCK_ITEMS);
        let models = self.visible_models();
        let s = self.settings();
        let folders: Vec<DockFolder> = s.dock_items.folders.to_vec();
        let pinned_order: Vec<String> = s.dock_items.pinned_order.to_vec();
        let (icon_size, item_size) = {
            let st = self.state.borrow();
            (st.icon_size, st.item_size)
        };
        // 1. Klasörlerdeki uygulamalar (ana sırada ayrıca görünmez).
        let folder_apps: HashSet<String> = folders.iter().flat_map(|f| f.items.iter().map(|a| a.to_lowercase())).collect();
        // 2. Uygulama girdilerini model kimliği ve kısayol adına göre eşle.
        let mut cache: HashMap<String, AppRef> = HashMap::new();
        for m in &models {
            let app = self.new_app(m.clone());
            cache.insert(m.id.to_lowercase(), Rc::clone(&app));
            cache.insert(PinnedOrder::id_of(&m.id).to_lowercase(), app);
        }
        // 3. Hedef sıra.
        let available: Vec<String> = models.iter().filter(|m| m.is_pinned).map(|m| PinnedOrder::id_of(&m.id)).collect();
        let (effective_order, reconciled) = PinnedOrder::reconcile_with_folders(&pinned_order, &available, &folders);
        self.p.borrow_mut().effective_order = effective_order.clone();
        let folder_by_id: HashMap<String, DockFolder> = reconciled.into_iter().map(|f| (f.id.to_lowercase(), f)).collect();
        let existing = self.items.borrow().clone();
        let find_existing_app = |id: &str| {
            existing.iter().find_map(|e| match e {
                DockEntry::App(a) if a.borrow().id().eq_ignore_ascii_case(id) => Some(Rc::clone(a)),
                _ => None,
            })
        };
        let mut desired: Vec<DockEntry> = Vec::new();
        for token in &effective_order {
            if PinnedOrder::is_folder_id(token) {
                let fid = PinnedOrder::extract_folder_id(token);
                let Some(folder) = folder_by_id.get(&fid.to_lowercase()) else { continue };
                let items: Vec<AppRef> = folder
                    .items
                    .iter()
                    .map(|item_id| match cache.get(&item_id.to_lowercase()) {
                        Some(a) => Rc::clone(a),
                        None => {
                            let title = std::path::Path::new(item_id).file_stem().and_then(|s| s.to_str()).unwrap_or(item_id).to_string();
                            self.new_app(DockItem {
                                id: item_id.clone(),
                                title,
                                file_path: Some(item_id.clone()),
                                is_pinned: true,
                                ..Default::default()
                            })
                        }
                    })
                    .collect();
                let found = existing.iter().find_map(|e| match e {
                    DockEntry::Folder(f) if f.borrow().folder.id.eq_ignore_ascii_case(&folder.id) => Some(Rc::clone(f)),
                    _ => None,
                });
                let entry = match found {
                    Some(f) => {
                        {
                            let mut fm = f.borrow_mut();
                            fm.motion.icon_size = icon_size;
                            fm.motion.item_size = item_size;
                            fm.update_folder(folder.clone(), items);
                        }
                        f
                    }
                    None => {
                        let mut f = DockFolderEntry::new(folder.clone(), items);
                        f.motion.icon_size = icon_size;
                        f.motion.item_size = item_size;
                        Rc::new(RefCell::new(f))
                    }
                };
                desired.push(DockEntry::Folder(entry));
            } else if !folder_apps.contains(&token.to_lowercase())
                && let Some(app) = cache.get(&token.to_lowercase())
            {
                let id = app.borrow().id().to_string();
                match find_existing_app(&id) {
                    Some(ex) => {
                        {
                            let mut e = ex.borrow_mut();
                            e.motion.icon_size = icon_size;
                            e.motion.item_size = item_size;
                            e.update(app.borrow().model.clone());
                        }
                        desired.push(DockEntry::App(ex));
                    }
                    None => desired.push(DockEntry::App(Rc::clone(app))),
                }
            }
        }
        // Sabitli olmayan açık pencereler.
        for m in models.iter().filter(|m| !m.is_pinned) {
            if folder_apps.contains(&PinnedOrder::id_of(&m.id).to_lowercase()) || folder_apps.contains(&m.id.to_lowercase()) {
                continue;
            }
            match find_existing_app(&m.id) {
                Some(ex) => {
                    {
                        let mut e = ex.borrow_mut();
                        e.motion.icon_size = icon_size;
                        e.motion.item_size = item_size;
                        e.update(m.clone());
                    }
                    desired.push(DockEntry::App(ex));
                }
                None => {
                    if let Some(a) = cache.get(&m.id.to_lowercase()) {
                        desired.push(DockEntry::App(Rc::clone(a)));
                    }
                }
            }
        }
        // Ek öğeler varsa şerit düzenine katılır.
        let desired = self.compose_extras(desired, &existing);
        // 4. Koleksiyonu yerinde güncelle: var olan girdiler korunur (yay durumları kaybolmaz).
        let vertical = self.is_vertical();
        for e in &desired {
            e.with_motion(|m| m.is_vertical = vertical);
        }
        *self.items.borrow_mut() = desired;
        self.recompute_fit();
        self.reevaluate();
        self.sync_clock_timer();
        self.items_changed.invoke(&());
        self.notify();
    }

    /// Kayıtlı ek öğeleri uygulama/klasör girdileriyle birleştirir. Ek öğe yoksa girdiler değişmeden döner.
    /// Sabitlenmemiş çalışan uygulamalar son sabitli girdinin arkasına girer.
    fn compose_extras(&self, entries: Vec<DockEntry>, existing: &[DockEntry]) -> Vec<DockEntry> {
        let s = self.settings();
        let specs = DockItemRules::resolve(&s.dock_items.items, &s.dock_items.pinned_order, DockItemRules::MAX_ITEM_COUNT);
        let has_extras = DockItemLayout::has_extras(&specs);
        self.p.borrow_mut().has_extras = has_extras;
        if !has_extras {
            self.p.borrow_mut().layout = specs;
            return entries;
        }
        let effective = self.p.borrow().effective_order.clone();
        let layout = DockItemLayout::merge(&specs, &effective);
        self.p.borrow_mut().layout = layout.clone();
        let mut pinned: HashMap<String, DockEntry> = HashMap::new();
        let mut running: Vec<DockEntry> = Vec::new();
        for e in entries {
            if e.is_pinned() {
                let key = match &e {
                    DockEntry::Folder(_) => e.id(),
                    _ => PinnedOrder::id_of(&e.id()),
                };
                pinned.entry(key.to_lowercase()).or_insert(e);
            } else {
                running.push(e);
            }
        }
        let (icon_size, item_size) = {
            let st = self.state.borrow();
            (st.icon_size, st.item_size)
        };
        let mut result: Vec<DockEntry> = Vec::new();
        let (mut separators, mut spacers) = (0, 0);
        for (i, spec) in layout.iter().enumerate() {
            if let Some(token) = DockItemLayout::token_of(spec) {
                if let Some(e) = pinned.get(&token.to_lowercase()) {
                    result.push(e.clone());
                }
                continue;
            }
            let id = match spec.kind {
                DockItemKind::Separator => {
                    separators += 1;
                    format!("item:separator:{}", separators - 1)
                }
                DockItemKind::Spacer => {
                    spacers += 1;
                    format!("item:spacer:{}", spacers - 1)
                }
                k => format!("item:{}:{}", format!("{k:?}").to_lowercase(), spec.id),
            };
            let found = existing.iter().find_map(|e| match e {
                DockEntry::Extra(x) if x.borrow().id().eq_ignore_ascii_case(&id) => Some(Rc::clone(x)),
                _ => None,
            });
            let extra = match found {
                Some(x) => {
                    x.borrow_mut().update(spec.clone(), i as i32);
                    x
                }
                None => Rc::new(RefCell::new(DockExtraEntry::new(spec.clone(), id, i as i32))),
            };
            {
                let mut x = extra.borrow_mut();
                x.motion.icon_size = icon_size;
                x.motion.item_size = item_size;
                x.title = extra_title(spec);
            }
            result.push(DockEntry::Extra(extra));
        }
        let last_pinned = result.iter().rposition(|e| e.is_pinned()).map_or(0, |i| i + 1);
        result.splice(last_pinned..last_pinned, running);
        result
    }

    /// Ek öğeye tıklandı: saat Bildirim Merkezi'ni, çöp kutusu Geri Dönüşüm Kutusu'nu, özel düğme hedefini açar.
    pub fn activate_extra(&self, extra: &Rc<RefCell<DockExtraEntry>>) {
        self.close_preview_now();
        let spec = extra.borrow().spec.clone();
        match spec.kind {
            DockItemKind::Clock => self.shell.open_notification_center(),
            // Boşaltma bu sürümde yok: yalnız kutuyu açar.
            DockItemKind::Trash => {
                let _ = self.shell.launch("shell:RecycleBinFolder", None);
            }
            // Komut güvenilmez: Core ayrıştırır, geçersizse hiçbir şey çalışmaz.
            DockItemKind::Custom => {
                if let Some(l) = DockCustomCommand::try_parse(Some(&spec.command)) {
                    let _ = self.shell.launch(&l.target, l.arguments.as_deref());
                }
            }
            _ => {}
        }
    }

    pub fn add_separator(&self) {
        self.add_extra(DockItemSpec::new(DockItemKind::Separator, "", ""));
    }

    pub fn add_spacer(&self) {
        self.add_extra(DockItemSpec::new(DockItemKind::Spacer, "", ""));
    }

    pub fn add_clock(&self) {
        self.add_extra(DockItemSpec::new(DockItemKind::Clock, "Clock", ""));
    }

    pub fn add_trash(&self) {
        self.add_extra(DockItemSpec::new(DockItemKind::Trash, "Trash", ""));
    }

    /// Özel düğme: komutu çağıran seçer (bir dosya ya da yazılan komut).
    pub fn add_custom(&self, command: Option<String>) {
        if let Some(c) = command {
            self.add_extra(DockItemSpec::new(DockItemKind::Custom, DockItemLayout::new_custom_id(), c));
        }
    }

    fn add_extra(&self, spec: DockItemSpec) {
        let s = self.settings();
        let effective = self.p.borrow().effective_order.clone();
        let current = DockItemLayout::merge(
            &DockItemRules::resolve(&s.dock_items.items, &s.dock_items.pinned_order, DockItemRules::MAX_ITEM_COUNT),
            &effective,
        );
        self.save_layout(DockItemLayout::add(&current, spec));
    }

    pub fn remove_extra(&self, extra: &Rc<RefCell<DockExtraEntry>>) {
        let layout = self.p.borrow().layout.clone();
        self.save_layout(DockItemLayout::remove_at(&layout, extra.borrow().layout_index));
    }

    /// Ek öğeyi bir yuva sola (-1) ya da sağa (+1) kaydırır.
    pub fn move_extra(&self, extra: &Rc<RefCell<DockExtraEntry>>, delta: i32) {
        let layout = self.p.borrow().layout.clone();
        let i = extra.borrow().layout_index;
        self.save_layout(DockItemRules::move_item(&layout, i, i + delta));
    }

    fn save_layout(&self, layout: Vec<DockItemSpec>) {
        {
            let s = self.settings.borrow();
            dock_settings_store::save_items(DockItemLayout::to_stored_entries(&layout), Some(&mut s.borrow_mut()));
        }
        self.update_items();
    }

    /// Saat zamanlayıcısı yalnız dock görünürken ve şeritte saat varken çalışır; dakika başına hizalı tek uyanma.
    fn sync_clock_timer(&self) {
        let clocks: Vec<Rc<RefCell<DockExtraEntry>>> = self
            .items
            .borrow()
            .iter()
            .filter_map(|e| match e {
                DockEntry::Extra(x) if x.borrow().is_clock() => Some(Rc::clone(x)),
                _ => None,
            })
            .collect();
        if self.disposed.get() || !self.state.borrow().is_visible || clocks.is_empty() {
            self.clock_timer.stop();
            return;
        }
        if self.clock_timer.running() {
            let now = chrono::Local::now().naive_local();
            for c in clocks.iter().filter(|c| c.borrow().time_text.is_empty()) {
                apply_clock_text(&mut c.borrow_mut(), now);
            }
            return;
        }
        self.on_clock_tick();
    }

    fn on_clock_tick(&self) {
        let now = chrono::Local::now().naive_local();
        for e in self.items.borrow().iter() {
            if let DockEntry::Extra(x) = e
                && x.borrow().is_clock()
            {
                apply_clock_text(&mut x.borrow_mut(), now);
            }
        }
        let delay = DockClockTick::delay_to_next_minute(now).total_milliseconds().max(1.0) as u64;
        let w = self.weak();
        self.clock_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(delay), move || {
            if let Some(vm) = w.upgrade() {
                vm.on_clock_tick();
            }
        });
        self.notify();
    }

    // ---- Medya ----

    fn update_media_info(&self, info: Option<&MediaInfo>) {
        {
            let mut st = self.state.borrow_mut();
            match info.filter(|i| i.is_playing) {
                None => {
                    st.has_now_playing = false;
                    st.now_playing_title = Strings::get("NoMediaPlaying");
                    st.now_playing_artist = "Koy".into();
                    st.now_playing_cover = None;
                }
                Some(i) => {
                    st.has_now_playing = true;
                    st.now_playing_title = if i.title.trim().is_empty() { Strings::get("UnknownTrack") } else { i.title.clone() };
                    st.now_playing_artist = if i.artist.trim().is_empty() { Strings::get("UnknownArtist") } else { i.artist.clone() };
                    st.now_playing_cover = i
                        .thumbnail
                        .as_deref()
                        .filter(|t| !t.is_empty())
                        .and_then(|t| crate::infra::image_decode::decode(t, Some(crate::infra::image_decode::pixels_for(28.0))))
                        .map(|b| b.to_image());
                }
            }
        }
        self.notify();
    }

    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        for t in [
            &self.thumbnail_hover_timer,
            &self.preview_close_timer,
            &self.dodge_hover_timer,
            &self.snap_timer,
            &self.clock_timer,
            &self.tray_close_timer,
            &self.tray_clear_timer,
        ] {
            t.stop();
        }
        context_menu_state::unsubscribe(self.menu_sub.get());
        for (kind, s) in self.subs.borrow_mut().drain(..) {
            match kind {
                0 => self.controller.items_changed().unsubscribe(s),
                1 => self.controller.fullscreen_state_changed().unsubscribe(s),
                2 => {
                    if let Some(v) = &self.visibility {
                        v.changed().unsubscribe(s);
                    }
                }
                3 => {
                    if let Some(m) = &self.media {
                        m.changed().unsubscribe(s);
                    }
                }
                _ => {
                    if let Some(t) = self.tray.borrow().as_ref() {
                        t.icons_changed().unsubscribe(s);
                    }
                }
            }
        }
        *self.tray.borrow_mut() = None;
    }
}

fn extra_title(spec: &DockItemSpec) -> String {
    match spec.kind {
        DockItemKind::Clock => Strings::get("DockItemClock"),
        DockItemKind::Trash => Strings::get("DockItemTrash"),
        DockItemKind::Custom => {
            DockCustomCommand::try_parse(Some(&spec.command)).map_or_else(|| Strings::get("DockItemCustom"), |l| l.target)
        }
        _ => String::new(),
    }
}

fn apply_clock_text(clock: &mut DockExtraEntry, now: chrono::NaiveDateTime) {
    let culture = strings::culture();
    clock.time_text = DockClockTick::format_time(now, &culture);
    clock.date_text = DockClockTick::format_date(now, &culture);
}

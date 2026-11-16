//! Dock şeridindeki girdiler: uygulama, klasör ve ek öğe (ayırıcı, boşluk, saat, çöp kutusu, özel düğme).
//! Karşılığı: `DockItemViewModel.cs` (`DockEntryViewModel` + `DockItemViewModel`), `DockFolderViewModel.cs`,
//! `DockExtraViewModel.cs`.
//!
//! Ortak hareket (`EntryMotion`): fare üstü büyütme ölçeği, sıralama kayması ve basma geri bildirimi; üçü ayrı yaydır.

use std::cell::RefCell;
use std::rc::Rc;

use koy_core::animation::Spring;
use koy_core::dock::items::{DockItemKind, DockItemSpec};
use koy_core::dock::{DockController, DockFolder, DockItem, DockPresentationMath, DockPress, DockWindowEntry, PinnedOrder};

use super::presentation::DockPresentationStyle;
use crate::infra::image_decode;

/// Yaylarla ölçek, kayma ve basma (C# `DockEntryViewModel`'in hareket kısmı).
pub struct EntryMotion {
    scale: Spring,
    press: Spring,
    offset: Spring,
    press_motion: bool,
    pub is_pressed: bool,
    pub scale_value: f64,
    pub icon_size: f64,
    pub item_size: f64,
    pub is_vertical: bool,
    pub is_target_glow: bool,
}

impl Default for EntryMotion {
    fn default() -> Self {
        EntryMotion {
            scale: Spring::new(1.0).with_stiffness(320.0).with_damping(26.0),
            // Hafif sönümlü: bırakınca küçük bir geri sıçrama.
            press: Spring::new(0.0).with_stiffness(520.0).with_damping(24.0),
            offset: Spring::new(0.0).with_stiffness(380.0).with_damping(30.0),
            press_motion: true,
            is_pressed: false,
            scale_value: 1.0,
            icon_size: 28.0,
            item_size: 38.0,
            is_vertical: false,
            is_target_glow: false,
        }
    }
}

impl EntryMotion {
    /// Basma çarpanı (hover büyütmesinden ayrı). Hareket azaltmada hep 1.
    pub fn press_scale(&self) -> f64 {
        DockPress::scale(self.press.value(), self.press_motion)
    }

    /// Basılıyken koyulaşma (opaklık).
    pub fn press_opacity(&self) -> f64 {
        DockPress::opacity(self.press.value())
    }

    pub fn set_pressed(&mut self, pressed: bool, animate: bool) {
        self.is_pressed = pressed;
        self.press_motion = animate;
        let target = if pressed { 1.0 } else { 0.0 };
        if animate {
            self.press.set_target(target);
        } else {
            self.press.snap_to(target);
        }
    }

    pub fn snap_press(&mut self, pressed: bool) {
        self.is_pressed = pressed;
        self.press_motion = true;
        self.press.snap_to(if pressed { 1.0 } else { 0.0 });
    }

    pub fn step_press(&mut self, dt: f64) -> bool {
        if self.press.is_at_rest() {
            return false;
        }
        self.press.step(dt)
    }

    pub fn offset(&self) -> f64 {
        self.offset.value()
    }

    pub fn translate_x(&self) -> f64 {
        if self.is_vertical { 0.0 } else { self.offset.value() }
    }

    pub fn translate_y(&self) -> f64 {
        if self.is_vertical { self.offset.value() } else { 0.0 }
    }

    pub fn set_target_offset(&mut self, target: f64) {
        self.offset.set_target(target);
    }

    pub fn snap_offset(&mut self, offset: f64) {
        self.offset.snap_to(offset);
    }

    pub fn step_offset(&mut self, dt: f64) -> bool {
        if self.offset.is_at_rest() {
            return false;
        }
        self.offset.step(dt)
    }

    pub fn set_target_scale(&mut self, target: f64) {
        self.scale.set_target(target);
    }

    pub fn snap_scale(&mut self, scale: f64) {
        self.scale.snap_to(scale);
        self.scale_value = scale;
    }

    pub fn step_scale(&mut self, dt: f64) -> bool {
        let moving = self.scale.step(dt);
        self.scale_value = self.scale.value();
        moving
    }
}

/// Uygulama simgesinin kodlanmış verisini 48 px'e çözer.
pub fn load_icon(data: &[u8]) -> Option<slint::Image> {
    image_decode::decode(data, Some(48)).map(|b| b.to_image())
}

/// Tek uygulama (sabitli ya da çalışan). C# `DockItemViewModel`.
pub struct DockAppEntry {
    pub model: DockItem,
    pub title: String,
    pub is_pinned: bool,
    pub is_running: bool,
    pub is_active: bool,
    pub is_flashing: bool,
    pub is_attention: bool,
    pub badge_count: i32,
    pub has_badge: bool,
    pub is_multi_window: bool,
    pub windows: Vec<DockWindowEntry>,
    pub icon: Option<slint::Image>,
    pub presentation: Rc<DockPresentationStyle>,
    /// Simgenin altında çizilecek gösterge sayısı.
    pub indicator_count: i32,
    pub show_badge: bool,
    pub badge_text: String,
    custom_icon: Option<slint::Image>,
    /// Ayarlarda bu uygulamaya ait simge değişimi var ("Simgeyi sıfırla" etkin).
    pub has_icon_override: bool,
    pub motion: EntryMotion,
}

impl DockAppEntry {
    pub fn new(model: DockItem, presentation: Rc<DockPresentationStyle>) -> Self {
        let windows = model.windows.clone().unwrap_or_default();
        let icon = model.icon_data.as_deref().filter(|d| !d.is_empty()).and_then(load_icon);
        let mut e = DockAppEntry {
            title: model.title.clone(),
            is_pinned: model.is_pinned,
            is_running: model.is_running,
            is_active: model.is_active,
            is_flashing: model.is_flashing,
            is_attention: model.is_flashing,
            badge_count: model.badge_count,
            has_badge: model.badge_count > 0,
            is_multi_window: windows.len() > 1,
            windows,
            icon,
            presentation,
            indicator_count: 0,
            show_badge: false,
            badge_text: String::new(),
            custom_icon: None,
            has_icon_override: false,
            motion: EntryMotion::default(),
            model,
        };
        e.refresh_presentation();
        e
    }

    pub fn id(&self) -> &str {
        &self.model.id
    }

    pub fn has_open_windows(&self) -> bool {
        !self.windows.is_empty()
    }

    pub fn single_window(&self) -> Option<&DockWindowEntry> {
        (self.windows.len() == 1).then(|| &self.windows[0])
    }

    /// Gösterge sayısı ve rozet kararı Core'dadır; burada yalnız sonuç yazılır.
    fn refresh_presentation(&mut self) {
        let window_count = if self.is_running { (self.windows.len() as i32).max(1) } else { 0 };
        let p = &self.presentation;
        self.indicator_count = DockPresentationMath::indicator_count(window_count, p.indicator_style, p.one_per_window);
        self.show_badge = p.show_badges && DockPresentationMath::shows_badge(self.badge_count, p.badge_style);
        self.badge_text = DockPresentationMath::badge_text(self.badge_count, p.badge_style);
    }

    pub fn update(&mut self, model: DockItem) {
        self.title = model.title.clone();
        self.is_pinned = model.is_pinned;
        self.is_running = model.is_running;
        self.is_active = model.is_active;
        self.is_flashing = model.is_flashing;
        self.is_attention = model.is_flashing;
        self.badge_count = model.badge_count;
        self.has_badge = model.badge_count > 0;
        let new_windows = model.windows.clone().unwrap_or_default();
        if self.windows != new_windows {
            self.is_multi_window = new_windows.len() > 1;
            self.windows = new_windows;
        }
        if self.custom_icon.is_none()
            && self.icon.is_none()
            && let Some(d) = model.icon_data.as_deref().filter(|d| !d.is_empty())
        {
            self.icon = load_icon(d);
        }
        self.model = model;
        self.refresh_presentation();
    }

    /// Değişim ya da paket simgesini uygular; `None` verilirse uygulamanın kendi simgesine dönülür.
    pub fn apply_app_icon(&mut self, custom: Option<slint::Image>, has_override: bool) {
        self.has_icon_override = has_override;
        let had_custom = self.custom_icon.is_some();
        self.custom_icon = custom.clone();
        if let Some(c) = custom {
            self.icon = Some(c);
        } else if had_custom {
            self.icon = self.model.icon_data.as_deref().filter(|d| !d.is_empty()).and_then(load_icon);
        }
    }

    pub fn click(&self, controller: &DockController) {
        controller.handle_item_click(&self.model);
    }

    pub fn middle_click(&self, controller: &DockController) {
        controller.handle_item_middle_click(&self.model);
    }

    pub fn close(&self, controller: &DockController) {
        controller.handle_item_close(&self.model);
    }

    pub fn terminate(&self, controller: &DockController) {
        controller.handle_item_terminate(&self.model);
    }
}

pub type AppRef = Rc<RefCell<DockAppEntry>>;

/// Sabitli uygulamaları gruplayan klasör. İlk 4 uygulamanın simgesi 2×2 ızgarada; içindekilerden biri çalışıyorsa
/// gösterge yanar. Tıklanınca klasör paneli açılır. C# `DockFolderViewModel`.
pub struct DockFolderEntry {
    pub folder: DockFolder,
    pub title: String,
    pub is_folder_panel_open: bool,
    pub is_editing_title: bool,
    pub items: Vec<AppRef>,
    pub motion: EntryMotion,
}

impl DockFolderEntry {
    pub fn new(folder: DockFolder, items: Vec<AppRef>) -> Self {
        DockFolderEntry {
            title: folder.name.clone(),
            folder,
            is_folder_panel_open: false,
            is_editing_title: false,
            items,
            motion: EntryMotion::default(),
        }
    }

    pub fn id(&self) -> String {
        PinnedOrder::to_folder_token(&self.folder.id)
    }

    pub fn is_running(&self) -> bool {
        self.items.iter().any(|i| i.borrow().is_running)
    }

    /// 2×2 mini ızgara için ilk 4 uygulamanın simgeleri.
    pub fn preview_icons(&self) -> Vec<Option<slint::Image>> {
        self.items.iter().take(4).map(|i| i.borrow().icon.clone()).collect()
    }

    pub fn update_folder(&mut self, folder: DockFolder, items: Vec<AppRef>) {
        self.title = folder.name.clone();
        self.folder = folder;
        self.items = items;
    }

    pub fn close_folder_panel(&mut self) {
        self.is_folder_panel_open = false;
        self.is_editing_title = false;
    }

    pub fn start_rename(&mut self) {
        self.is_folder_panel_open = true;
        self.is_editing_title = true;
    }

    pub fn click(&mut self) {
        self.is_folder_panel_open = !self.is_folder_panel_open;
        if !self.is_folder_panel_open {
            self.is_editing_title = false;
        }
    }
}

/// Ek öğe (ayırıcı, boşluk, saat, çöp kutusu, özel düğme). Yalnız durum taşır; tıklamayı `DockViewModel::activate_extra`
/// yürütür. C# `DockExtraViewModel`.
pub struct DockExtraEntry {
    pub spec: DockItemSpec,
    entry_id: String,
    /// Şeritteki ve kayıtlı düzendeki sırası (taşıma ve kaldırma bununla yapılır).
    pub layout_index: i32,
    pub title: String,
    pub time_text: String,
    pub date_text: String,
    pub motion: EntryMotion,
}

impl DockExtraEntry {
    pub const TRASH_GLYPH: &'static str = "\u{E74D}";
    pub const CUSTOM_GLYPH: &'static str = "\u{E7C9}";

    pub fn new(spec: DockItemSpec, id: String, layout_index: i32) -> Self {
        DockExtraEntry {
            spec,
            entry_id: id,
            layout_index,
            title: String::new(),
            time_text: String::new(),
            date_text: String::new(),
            motion: EntryMotion::default(),
        }
    }

    pub fn id(&self) -> &str {
        &self.entry_id
    }

    pub fn kind(&self) -> DockItemKind {
        self.spec.kind
    }

    pub fn is_separator(&self) -> bool {
        self.kind() == DockItemKind::Separator
    }

    pub fn is_spacer(&self) -> bool {
        self.kind() == DockItemKind::Spacer
    }

    pub fn is_clock(&self) -> bool {
        self.kind() == DockItemKind::Clock
    }

    pub fn is_glyph_button(&self) -> bool {
        matches!(self.kind(), DockItemKind::Trash | DockItemKind::Custom)
    }

    /// Tıklanabilir mi (ayırıcı ve boşluk değil).
    pub fn is_interactive(&self) -> bool {
        !matches!(self.kind(), DockItemKind::Separator | DockItemKind::Spacer)
    }

    pub fn glyph(&self) -> &'static str {
        if self.kind() == DockItemKind::Trash { Self::TRASH_GLYPH } else { Self::CUSTOM_GLYPH }
    }

    pub fn update(&mut self, spec: DockItemSpec, layout_index: i32) {
        self.spec = spec;
        self.layout_index = layout_index;
    }
}

/// Şeritteki bir girdi.
#[derive(Clone)]
pub enum DockEntry {
    App(AppRef),
    Folder(Rc<RefCell<DockFolderEntry>>),
    Extra(Rc<RefCell<DockExtraEntry>>),
}

impl DockEntry {
    pub fn id(&self) -> String {
        match self {
            DockEntry::App(a) => a.borrow().id().to_string(),
            DockEntry::Folder(f) => f.borrow().id(),
            DockEntry::Extra(x) => x.borrow().id().to_string(),
        }
    }

    pub fn is_pinned(&self) -> bool {
        match self {
            DockEntry::App(a) => a.borrow().is_pinned,
            DockEntry::Folder(_) => true,
            DockEntry::Extra(_) => false,
        }
    }

    pub fn is_running(&self) -> bool {
        match self {
            DockEntry::App(a) => a.borrow().is_running,
            DockEntry::Folder(f) => f.borrow().is_running(),
            DockEntry::Extra(_) => false,
        }
    }

    /// Hareket alanına erişim.
    pub fn with_motion<R>(&self, f: impl FnOnce(&mut EntryMotion) -> R) -> R {
        match self {
            DockEntry::App(a) => f(&mut a.borrow_mut().motion),
            DockEntry::Folder(x) => f(&mut x.borrow_mut().motion),
            DockEntry::Extra(x) => f(&mut x.borrow_mut().motion),
        }
    }

    pub fn same(&self, other: &DockEntry) -> bool {
        match (self, other) {
            (DockEntry::App(a), DockEntry::App(b)) => Rc::ptr_eq(a, b),
            (DockEntry::Folder(a), DockEntry::Folder(b)) => Rc::ptr_eq(a, b),
            (DockEntry::Extra(a), DockEntry::Extra(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

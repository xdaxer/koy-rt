//! Dock'un sağ tık menüleri. Karşılığı: `DockContextMenuManager.cs`, `DockJumpListMenu.cs`, `DockStartMenu.cs`,
//! `DockCustomButtonPicker.cs` ve XAML'deki öğe / pencere menüleri. Menüler yerel Win32 menüsüdür; açıkken dock
//! gizlenmez (`context_menu_state`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use koy_core::dock::DockEdge;
use koy_core::dock::items::DockCustomCommand;
use koy_core::jump_list::{IJumpListService, JumpListEntry, JumpListSection, JumpListSectionKind};
use koy_core::start_menu::{StartMenuActions, StartMenuGroup, StartMenuItem};

use super::context_menu_state;
use super::dock_view_model::DockViewModel;
use super::entries::{AppRef, DockEntry, DockExtraEntry, DockFolderEntry};
use crate::infra::dispatch;
use crate::infra::native_menu::{self, MenuAnchor, MenuEntry};
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;

const LOAD_TIMEOUT: Duration = Duration::from_millis(700);
// Atlama listesi girdilerinin kimlikleri bu değerden başlar (öteki menü komutlarıyla çakışmasın).
const JUMP_BASE: u32 = 10_000;

thread_local! {
    static VERSION: Cell<u64> = const { Cell::new(0) };
}

fn anchor_for(edge: DockEdge) -> MenuAnchor {
    match edge {
        DockEdge::Top => MenuAnchor::Below,
        DockEdge::Left => MenuAnchor::Right,
        DockEdge::Right => MenuAnchor::Left,
        DockEdge::Bottom => MenuAnchor::Above,
    }
}

/// Menüyü açar; açık kaldığı sürece dock gizlenmez.
fn open(owner: isize, at: (i32, i32), edge: DockEdge, items: &[MenuEntry]) -> Option<u32> {
    context_menu_state::set_menu_open(true);
    let r = native_menu::show(owner, at.0, at.1, anchor_for(edge), items);
    context_menu_state::set_menu_open(false);
    r
}

fn jump_header(section: &JumpListSection) -> String {
    match section.kind {
        JumpListSectionKind::Custom => section.name.clone().unwrap_or_default(),
        JumpListSectionKind::Frequent => Strings::get("DockJumpListFrequent"),
        JumpListSectionKind::Recent => Strings::get("DockJumpListRecent"),
        _ => Strings::get("DockJumpListTasks"),
    }
}

fn exe_or_none(path: Option<&str>) -> Option<String> {
    path.filter(|p| !p.trim().is_empty() && p.to_lowercase().ends_with(".exe") && std::path::Path::new(p).is_file()).map(str::to_string)
}

/// Uygulama simgesinin menüsü: varsa atlama listesi (en çok 700 ms beklenir), sonra Koy'un komutları.
pub fn open_app_menu(vm: &Rc<DockViewModel>, app: &AppRef, owner: isize, at: (i32, i32)) {
    vm.close_preview_now();
    let version = VERSION.get() + 1;
    VERSION.set(version);
    let (app_id, exe) = {
        let a = app.borrow();
        (a.model.app_user_model_id.clone().filter(|s| !s.is_empty()), exe_or_none(a.model.target_path.as_deref()))
    };
    let Some(service) = vm.jump_lists.clone().filter(|_| app_id.is_some() || exe.is_some()) else {
        show_app_menu(vm, app, owner, at, Vec::new(), None);
        return;
    };
    // Okuma ayrı iş parçacığında; süre dolarsa liste olmadan açılır.
    let shown = Rc::new(Cell::new(false));
    let ui = UiRef::new(vm);
    let app_ui = UiRef::new(app);
    let svc = Arc::clone(&service);
    let (id2, exe2) = (app_id.clone(), exe.clone());
    let shown_ui = UiRef::new(&shown);
    std::thread::spawn(move || {
        let id = id2.or_else(|| exe2.as_deref().and_then(|e| svc.resolve_app_id(e)));
        let sections = id.map(|id| svc.get(&id)).unwrap_or_default();
        dispatch::post(move || {
            let (Some(vm), Some(app), Some(shown)) = (ui.upgrade(), app_ui.upgrade(), shown_ui.upgrade()) else { return };
            if shown.replace(true) || VERSION.get() != version {
                return;
            }
            show_app_menu(&vm, &app, owner, at, sections, Some((service, exe)));
        });
    });
    let w = Rc::downgrade(vm);
    let a = Rc::downgrade(app);
    let s = Rc::clone(&shown);
    slint::Timer::single_shot(LOAD_TIMEOUT, move || {
        if let (Some(vm), Some(app)) = (w.upgrade(), a.upgrade())
            && !s.replace(true)
            && VERSION.get() == version
        {
            show_app_menu(&vm, &app, owner, at, Vec::new(), None);
        }
    });
}

fn show_app_menu(
    vm: &Rc<DockViewModel>,
    app: &AppRef,
    owner: isize,
    at: (i32, i32),
    sections: Vec<JumpListSection>,
    service: Option<(Arc<dyn IJumpListService + Send + Sync>, Option<String>)>,
) {
    const NEW_INSTANCE: u32 = 1;
    const CHANGE_ICON: u32 = 2;
    const RESET_ICON: u32 = 3;
    const CLOSE: u32 = 4;
    const END_TASK: u32 = 5;
    let mut items = Vec::new();
    let mut jump: Vec<JumpListEntry> = Vec::new();
    for s in sections.iter().filter(|s| !s.entries.is_empty()) {
        items.push(MenuEntry::Header(jump_header(s)));
        for e in &s.entries {
            if e.is_separator() {
                items.push(MenuEntry::Separator);
            } else {
                items.push(MenuEntry::item(JUMP_BASE + jump.len() as u32, e.title.clone()));
                jump.push(e.clone());
            }
        }
    }
    if !items.is_empty() {
        items.push(MenuEntry::Separator);
    }
    let has_override = app.borrow().has_icon_override;
    items.extend([
        MenuEntry::item(NEW_INSTANCE, Strings::get("LaunchNewInstance")),
        MenuEntry::Separator,
        MenuEntry::item(CHANGE_ICON, Strings::get("DockItemChangeIcon")),
        MenuEntry::Item { id: RESET_ICON, label: Strings::get("DockItemResetIcon"), enabled: has_override },
        MenuEntry::Separator,
        MenuEntry::item(CLOSE, Strings::get("Close")),
        MenuEntry::item(END_TASK, Strings::get("EndTask")),
    ]);
    let Some(cmd) = open(owner, at, vm.edge(), &items) else { return };
    let controller = Arc::clone(vm.controller());
    match cmd {
        NEW_INSTANCE => app.borrow().middle_click(&controller),
        CHANGE_ICON => vm.change_app_icon(app, Some(owner)),
        RESET_ICON => vm.reset_app_icon(app),
        CLOSE => app.borrow().close(&controller),
        END_TASK => app.borrow().terminate(&controller),
        c if c >= JUMP_BASE => {
            if let (Some(entry), Some((svc, exe))) = (jump.get((c - JUMP_BASE) as usize), service) {
                svc.launch(entry, exe.as_deref());
            }
        }
        _ => {}
    }
}

/// Klasörün menüsü: Adını değiştir, Klasörü dağıt.
pub fn open_folder_menu(vm: &Rc<DockViewModel>, folder: &Rc<RefCell<DockFolderEntry>>, owner: isize, at: (i32, i32), on_rename: impl Fn()) {
    vm.close_preview_now();
    let items = [MenuEntry::item(1, Strings::get("DockFolderRename")), MenuEntry::Separator, MenuEntry::item(2, Strings::get("DockFolderDissolve"))];
    match open(owner, at, vm.edge(), &items) {
        Some(1) => {
            folder.borrow_mut().start_rename();
            on_rename();
        }
        Some(2) => {
            folder.borrow_mut().close_folder_panel();
            vm.dissolve_folder(folder);
        }
        _ => {}
    }
}

/// Ek öğenin menüsü: sola / sağa taşı, kaldır.
pub fn open_extra_menu(vm: &Rc<DockViewModel>, extra: &Rc<RefCell<DockExtraEntry>>, owner: isize, at: (i32, i32)) {
    vm.close_preview_now();
    let items = [
        MenuEntry::item(1, Strings::get("DockItemMoveLeft")),
        MenuEntry::item(2, Strings::get("DockItemMoveRight")),
        MenuEntry::Separator,
        MenuEntry::item(3, Strings::get("DockItemRemove")),
    ];
    match open(owner, at, vm.edge(), &items) {
        Some(1) => vm.move_extra(extra, -1),
        Some(2) => vm.move_extra(extra, 1),
        Some(3) => vm.remove_extra(extra),
        _ => {}
    }
}

/// Dock'un boş yerine sağ tık: Öğe ekle (ayırıcı, boşluk, saat, çöp kutusu, özel düğme) ve Ayarlar.
pub fn open_window_menu(vm: &Rc<DockViewModel>, owner: isize, at: (i32, i32)) {
    let items = [
        MenuEntry::Submenu {
            label: Strings::get("DockAddItem"),
            items: vec![
                MenuEntry::item(1, Strings::get("DockAddSeparator")),
                MenuEntry::item(2, Strings::get("DockAddSpacer")),
                MenuEntry::item(3, Strings::get("DockAddClock")),
                MenuEntry::item(4, Strings::get("DockAddTrash")),
                MenuEntry::item(5, Strings::get("DockAddCustom")),
            ],
        },
        MenuEntry::item(6, Strings::get("Settings")),
    ];
    match open(owner, at, vm.edge(), &items) {
        Some(1) => vm.add_separator(),
        Some(2) => vm.add_spacer(),
        Some(3) => vm.add_clock(),
        Some(4) => vm.add_trash(),
        Some(5) => vm.add_custom(pick_custom_command(owner)),
        Some(6) => vm.open_settings(),
        _ => {}
    }
}

/// Özel düğme: dosya seçtirir; komut tırnaklı yol olur ve Core doğrular.
fn pick_custom_command(owner: isize) -> Option<String> {
    let file = crate::infra::file_dialog::pick_file(Some(owner), &Strings::get("DockItemCustomPick"), "", false)?;
    let command = format!("\"{file}\"");
    DockCustomCommand::try_parse(Some(&command)).map(|_| command)
}

/// Başlat düğmesinin menüsü (Win+X karşılığı).
pub fn open_start_menu(vm: &Rc<DockViewModel>, actions: &Arc<StartMenuActions>, owner: isize, at: (i32, i32)) {
    vm.close_preview_now();
    let groups: Vec<StartMenuGroup> = actions.build_menu();
    let mut flat: Vec<StartMenuItem> = Vec::new();
    fn row(item: &StartMenuItem, flat: &mut Vec<StartMenuItem>) -> MenuEntry {
        let label = Strings::get(&format!("StartItem{}", item.id.name()));
        match item.children.as_ref().filter(|c| !c.is_empty()) {
            Some(children) => MenuEntry::Submenu { label, items: children.iter().map(|c| row(c, flat)).collect() },
            None => {
                flat.push(item.clone());
                MenuEntry::item(flat.len() as u32, label)
            }
        }
    }
    let mut items = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            items.push(MenuEntry::Separator);
        }
        for it in &g.items {
            items.push(row(it, &mut flat));
        }
    }
    if let Some(cmd) = open(owner, at, vm.edge(), &items)
        && let Some(item) = flat.get(cmd as usize - 1).cloned()
    {
        // UAC istemi başlatma çağrısını kilitleyebilir: dock donmasın diye ayrı iş parçacığında.
        let a = Arc::clone(actions);
        std::thread::spawn(move || a.execute(&item));
    }
}

/// Girdiye göre uygun menü.
pub fn open_entry_menu(vm: &Rc<DockViewModel>, entry: &DockEntry, owner: isize, at: (i32, i32), on_folder_rename: impl Fn()) {
    match entry {
        DockEntry::App(a) => open_app_menu(vm, a, owner, at),
        DockEntry::Folder(f) => open_folder_menu(vm, f, owner, at, on_folder_rename),
        DockEntry::Extra(x) => open_extra_menu(vm, x, owner, at),
    }
}

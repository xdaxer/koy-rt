//! Dock'tan uygulama düzeyine çıkan çağrılar (C#'ta statik özellikler ve `App`/`KoyWindow` erişimleri). Uygulama
//! (`app.rs`) açılışta doldurur; boşken çağrı bir şey yapmaz.

use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static OPEN_KOY_MENU: RefCell<Option<Rc<dyn Fn()>>> = RefCell::new(None);
    static OPEN_SETTINGS: RefCell<Option<Rc<dyn Fn(i32)>>> = RefCell::new(None);
    static RELOAD_DOCK: RefCell<Option<Rc<dyn Fn()>>> = RefCell::new(None);
}

/// Koy menüsü (sağ alt menü). C# `DockViewModel.OpenKoyMenuHandler`.
pub fn set_open_koy_menu(f: impl Fn() + 'static) {
    OPEN_KOY_MENU.with(|s| *s.borrow_mut() = Some(Rc::new(f)));
}

/// Ayarlar penceresini verilen sayfada açar. C# `KoyWindow.ShowInstance(page)`.
pub fn set_open_settings(f: impl Fn(i32) + 'static) {
    OPEN_SETTINGS.with(|s| *s.borrow_mut() = Some(Rc::new(f)));
}

/// Dock'u ayar dosyasından yeniden kurar. C# `App.ReloadDock`.
pub fn set_reload_dock(f: impl Fn() + 'static) {
    RELOAD_DOCK.with(|s| *s.borrow_mut() = Some(Rc::new(f)));
}

pub(super) fn open_koy_menu() -> bool {
    let f = OPEN_KOY_MENU.with(|s| s.borrow().clone());
    f.map(|f| f()).is_some()
}

pub(super) fn open_settings(page: i32) {
    let f = OPEN_SETTINGS.with(|s| s.borrow().clone());
    if let Some(f) = f {
        f(page);
    }
}

pub(super) fn reload_dock() {
    let f = RELOAD_DOCK.with(|s| s.borrow().clone());
    if let Some(f) = f {
        f();
    }
}

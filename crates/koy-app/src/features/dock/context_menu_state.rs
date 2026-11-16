//! Dock'ta açık bir bağlam menüsü var mı (C# `DockContextMenuManager.HasActiveMenu` ve `ActiveMenuChanged`).
//! Menü açıkken dock gizlenmez; açılıp kapanınca dock konumunu yeniden değerlendirir.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! {
    static ACTIVE: Cell<u32> = const { Cell::new(0) };
    static HANDLERS: RefCell<Vec<(u64, Rc<dyn Fn()>)>> = RefCell::new(Vec::new());
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

pub fn has_active_menu() -> bool {
    ACTIVE.get() > 0
}

/// Bir menü açıldı (`true`) ya da kapandı (`false`).
pub fn set_menu_open(open: bool) {
    let before = has_active_menu();
    ACTIVE.set(if open { ACTIVE.get() + 1 } else { ACTIVE.get().saturating_sub(1) });
    if before != has_active_menu() {
        let list: Vec<Rc<dyn Fn()>> = HANDLERS.with(|h| h.borrow().iter().map(|(_, f)| Rc::clone(f)).collect());
        for f in list {
            f();
        }
    }
}

pub fn subscribe(f: impl Fn() + 'static) -> u64 {
    let id = NEXT.get();
    NEXT.set(id + 1);
    HANDLERS.with(|h| h.borrow_mut().push((id, Rc::new(f))));
    id
}

pub fn unsubscribe(id: u64) {
    HANDLERS.with(|h| h.borrow_mut().retain(|(i, _)| *i != id));
}

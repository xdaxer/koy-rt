//! Bildirim erişim durumu değişti (C# `NotchFeature.NotificationAccessChanged` statik olayı): Ayarlar sayfası dinler.

use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static HANDLERS: RefCell<Vec<Rc<dyn Fn()>>> = RefCell::new(Vec::new());
}

/// Arayüz iş parçacığından abone olunur.
pub fn on_changed(f: impl Fn() + 'static) {
    HANDLERS.with(|h| h.borrow_mut().push(Rc::new(f)));
}

pub(super) fn raise_changed() {
    let list = HANDLERS.with(|h| h.borrow().clone());
    for f in list {
        f();
    }
}

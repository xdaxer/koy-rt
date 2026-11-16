//! Sistem tepsisi simgesini ve gelen tıklama olaylarını yönetir. Sol tek tık veya sağ tık: Sağ Alt Menüyü gecikmesiz
//! açar/kapatır. Çift tık: açık menüyü gizler ve doğrudan Koy ana penceresini açar.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use koy_platform::shell::{TrayHostService, TrayIconService};

use crate::infra::message_window::MessageWindow;

const WM_TRAY_CALLBACK: u32 = 0x8001;
const WM_LBUTTONUP: i32 = 0x0202;
const WM_LBUTTONDBLCLK: i32 = 0x0203;
const WM_RBUTTONUP: i32 = 0x0205;

/// Menü ve ana pencere eylemleri (C#'ta doğrudan `SagAltMenuWindow` / `KoyWindow` statik çağrıları).
pub struct TrayActions {
    pub toggle_menu: Box<dyn Fn()>,
    pub hide_menu: Box<dyn Fn()>,
    pub show_koy_window: Box<dyn Fn()>,
}

struct Inner {
    window: RefCell<Option<MessageWindow>>,
    tray: RefCell<Option<TrayIconService>>,
    ignore_next_left_up: Cell<bool>,
    running: Cell<bool>,
    // Dock açıkken Koy simgesi dock'ta durur; bildirim alanı simgesi gizlenir ama dinleyici pencere kalır
    // (Explorer yeniden başlama mesajı dock'un görev çubuğu gizlemesi için gerekli).
    icon_visible: Cell<bool>,
    actions: TrayActions,
    /// Explorer yeniden başladı; simge geri eklendi. Dock görev çubuğu gizlemesini yeniden uygulamalı.
    explorer_restarted: RefCell<Vec<Rc<dyn Fn()>>>,
}

#[derive(Clone)]
pub struct TrayIconManager {
    inner: Rc<Inner>,
}

impl TrayIconManager {
    pub fn new(actions: TrayActions) -> Self {
        TrayIconManager {
            inner: Rc::new(Inner {
                window: RefCell::new(None),
                tray: RefCell::new(None),
                ignore_next_left_up: Cell::new(false),
                running: Cell::new(false),
                icon_visible: Cell::new(true),
                actions,
                explorer_restarted: RefCell::new(Vec::new()),
            }),
        }
    }

    pub fn on_explorer_restarted(&self, f: impl Fn() + 'static) {
        self.inner.explorer_restarted.borrow_mut().push(Rc::new(f));
    }

    /// Bildirim alanı simgesini gösterir ya da kaldırır. Dinleyici pencere ve Explorer izlemesi etkilenmez.
    pub fn set_icon_visible(&self, visible: bool) {
        self.inner.icon_visible.set(visible);
        if !self.inner.running.get() {
            return;
        }
        if let Some(t) = self.inner.tray.borrow().as_ref() {
            if visible {
                t.add("Koy");
            } else {
                t.remove();
            }
        }
    }

    /// `show_icon`: başlangıçta simge eklensin mi (dock açılacaksa eklenip hemen kaldırılmasın diye).
    pub fn start(&self, show_icon: bool) {
        if self.inner.running.get() {
            return;
        }
        self.inner.icon_visible.set(show_icon);
        let weak = Rc::downgrade(&self.inner);
        let taskbar_created = TrayIconService::taskbar_created_message();
        let window = MessageWindow::new(
            "KoyTrayListener",
            "TrayIconManager.WndProc",
            Box::new(move |msg, _wparam, lparam| {
                let inner = weak.upgrade()?;
                handle_message(&inner, taskbar_created, msg, lparam.0)
            }),
        );
        let Some(window) = window else { return };
        let tray = TrayIconService::new(window.hwnd(), WM_TRAY_CALLBACK);
        if show_icon {
            tray.add("Koy");
        }
        *self.inner.window.borrow_mut() = Some(window);
        *self.inner.tray.borrow_mut() = Some(tray);
        self.inner.running.set(true);
    }

    pub fn stop(&self) {
        if !self.inner.running.get() {
            return;
        }
        self.inner.running.set(false);
        self.inner.ignore_next_left_up.set(false);
        if let Some(t) = self.inner.tray.borrow_mut().take() {
            t.remove();
        }
        drop(self.inner.window.borrow_mut().take());
    }
}

fn handle_message(inner: &Rc<Inner>, taskbar_created: u32, msg: u32, lparam: isize) -> Option<isize> {
    if taskbar_created != 0 && msg == taskbar_created {
        // Koy'un tepsi penceresi simgeleri yeniden eklettirmek için kendisi yayınlar; yankısı Gezgin'in yeniden
        // başlaması değildir.
        if TrayHostService::is_own_taskbar_created_broadcast() {
            return Some(0);
        }
        // Simge gizliyse (dock açık) yeniden eklenmez; yalnız dock görev çubuğu gizlemesini yeniden uygular.
        if inner.icon_visible.get()
            && let Some(t) = inner.tray.borrow().as_ref()
        {
            t.recreate();
        }
        let listeners: Vec<Rc<dyn Fn()>> = inner.explorer_restarted.borrow().clone();
        for f in listeners {
            f();
        }
        return Some(0);
    }
    if msg == WM_TRAY_CALLBACK {
        match lparam as i32 {
            WM_LBUTTONDBLCLK => {
                // Win32 çift tıkta DBLCLK ardından bir LBUTTONUP daha gönderir; menüyü yeniden açmasın diye yutulur.
                inner.ignore_next_left_up.set(true);
                (inner.actions.hide_menu)();
                (inner.actions.show_koy_window)();
            }
            WM_LBUTTONUP => {
                if inner.ignore_next_left_up.get() {
                    inner.ignore_next_left_up.set(false);
                    return Some(0);
                }
                // Sol tek tık: gecikmesiz doğrudan açılır menüyü aç/kapat.
                (inner.actions.toggle_menu)();
            }
            WM_RBUTTONUP => {
                inner.ignore_next_left_up.set(false);
                (inner.actions.toggle_menu)();
            }
            _ => {}
        }
        return Some(0);
    }
    None
}

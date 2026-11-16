//! Sistem olayları (C# `Microsoft.Win32.SystemEvents` karşılığı): ekran ayarları değişti (WM_DISPLAYCHANGE) ve
//! kullanıcı tercihleri değişti (WM_SETTINGCHANGE, tema ve duvar kağıdı). Tek gizli pencere, arayüz iş parçacığında;
//! olaylar orada yükselir.

use std::cell::RefCell;
use std::rc::Rc;

use super::message_window::MessageWindow;

const WM_SETTINGCHANGE: u32 = 0x001A;
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_THEMECHANGED: u32 = 0x031A;
const WM_DWMCOLORIZATIONCOLORCHANGED: u32 = 0x0320;

type Listener = Rc<dyn Fn()>;

#[derive(Default)]
struct Listeners {
    next: u64,
    display: Vec<(u64, Listener)>,
    preferences: Vec<(u64, Listener)>,
}

thread_local! {
    static LISTENERS: RefCell<Listeners> = RefCell::new(Listeners::default());
    static WINDOW: RefCell<Option<MessageWindow>> = const { RefCell::new(None) };
}

/// Abonelik; düşünce dinleyici kalkar.
pub struct Subscription(u64);

impl Drop for Subscription {
    fn drop(&mut self) {
        let id = self.0;
        LISTENERS.with(|l| {
            let mut l = l.borrow_mut();
            l.display.retain(|(i, _)| *i != id);
            l.preferences.retain(|(i, _)| *i != id);
        });
    }
}

fn raise(pick: fn(&Listeners) -> &Vec<(u64, Listener)>) {
    // Dinleyici çağrısı sırasında abone ekleme/çıkarma olabilir: önce kopya alınır.
    let list: Vec<Listener> = LISTENERS.with(|l| pick(&l.borrow()).iter().map(|(_, f)| Rc::clone(f)).collect());
    for f in list {
        f();
    }
}

fn ensure_window() {
    WINDOW.with(|w| {
        if w.borrow().is_some() {
            return;
        }
        let window = MessageWindow::new(
            ".NET-BroadcastEventWindow.Koy",
            "SystemEvents.WndProc",
            Box::new(|msg, _, _| {
                match msg {
                    WM_DISPLAYCHANGE => raise(|l| &l.display),
                    WM_SETTINGCHANGE | WM_THEMECHANGED | WM_DWMCOLORIZATIONCOLORCHANGED => raise(|l| &l.preferences),
                    _ => return None,
                }
                Some(0)
            }),
        );
        *w.borrow_mut() = window;
    });
}

fn subscribe(push: fn(&mut Listeners) -> &mut Vec<(u64, Listener)>, f: impl Fn() + 'static) -> Subscription {
    ensure_window();
    LISTENERS.with(|l| {
        let mut l = l.borrow_mut();
        l.next += 1;
        let id = l.next;
        push(&mut l).push((id, Rc::new(f)));
        Subscription(id)
    })
}

pub struct SystemEvents;

impl SystemEvents {
    /// `SystemEvents.DisplaySettingsChanged`. Yalnız arayüz iş parçacığından çağrılır.
    pub fn on_display_settings_changed(f: impl Fn() + 'static) -> Subscription {
        subscribe(|l| &mut l.display, f)
    }

    /// `SystemEvents.UserPreferenceChanged` (tema, renk, duvar kağıdı).
    pub fn on_user_preference_changed(f: impl Fn() + 'static) -> Subscription {
        subscribe(|l| &mut l.preferences, f)
    }
}

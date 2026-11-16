//! Genel kısayolların yaşam döngüsü: gizli bir pencere WM_HOTKEY mesajlarını alır, `ShortcutManager` kayıt ve çakışma
//! kararlarını verir. Çentik ve dock kapalıyken de çalışır (kısayollar özellikten bağımsızdır).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use koy_core::settings::AppSettings;
use koy_core::shortcuts::{Hotkey, IKeyboardHookService, ShortcutAction, ShortcutBindings, ShortcutManager, ShortcutResult};
use koy_platform::shell::{HotkeyService, KeyboardHookService};

use crate::infra::dispatch;
use crate::infra::message_window::MessageWindow;
use crate::infra::ui_ref::UiRef;

type Listener = Rc<dyn Fn()>;
type TriggerListener = Rc<dyn Fn(ShortcutAction)>;

#[derive(Default)]
struct Inner {
    window: Option<MessageWindow>,
    hotkeys: Option<Arc<HotkeyService>>,
    hook: Option<Arc<KeyboardHookService>>,
    manager: Option<ShortcutManager>,
    results: HashMap<ShortcutAction, ShortcutResult>,
    results_changed: Vec<Listener>,
    triggered: Vec<TriggerListener>,
}

#[derive(Clone, Default)]
pub struct ShortcutController {
    inner: Rc<RefCell<Inner>>,
}

impl ShortcutController {
    /// Son kayıt denemesinin eylem başına sonucu. Sayfa hata satırlarını bundan çizer.
    pub fn results(&self) -> HashMap<ShortcutAction, ShortcutResult> {
        self.inner.borrow().results.clone()
    }

    pub fn on_results_changed(&self, f: impl Fn() + 'static) {
        self.inner.borrow_mut().results_changed.push(Rc::new(f));
    }

    pub fn on_triggered(&self, f: impl Fn(ShortcutAction) + 'static) {
        self.inner.borrow_mut().triggered.push(Rc::new(f));
    }

    pub fn start(&self, settings: &AppSettings) {
        if self.inner.borrow().manager.is_some() {
            return;
        }
        // Pencere yordamı hotkey servisine bu tanıtıcı üzerinden ulaşır.
        let slot: Rc<RefCell<Option<Arc<HotkeyService>>>> = Rc::new(RefCell::new(None));
        let slot_for_proc = Rc::clone(&slot);
        let window = MessageWindow::new(
            "KoyHotkeyListener",
            "ShortcutController.WndProc",
            Box::new(move |msg, wparam, _| {
                let hotkeys = slot_for_proc.borrow().clone()?;
                hotkeys.handle_message(msg as i32, wparam.0 as isize).then_some(0)
            }),
        );
        let Some(window) = window else { return };
        let hotkeys = Arc::new(HotkeyService::new(window.hwnd()));
        *slot.borrow_mut() = Some(Arc::clone(&hotkeys));
        let hook = Arc::new(KeyboardHookService::new(dispatch::post_fn()));
        let manager = ShortcutManager::new(hotkeys.clone(), Some(hook.clone() as Arc<dyn IKeyboardHookService>));

        // Tetikleme arayüz iş parçacığında gelir (hotkey: pencere yordamı; kanca: post ile); jeton orada çözülür.
        let me = UiRef::new(&self.inner);
        manager.triggered().subscribe(move |action: &ShortcutAction| {
            let Some(inner) = me.upgrade() else { return };
            let action = *action;
            let listeners: Vec<TriggerListener> = inner.borrow().triggered.clone();
            dispatch::guard(&format!("Kısayol {action:?}"), || {
                for f in &listeners {
                    f(action);
                }
            });
        });

        {
            let mut inner = self.inner.borrow_mut();
            inner.window = Some(window);
            inner.hotkeys = Some(hotkeys);
            inner.hook = Some(hook);
            inner.manager = Some(manager);
        }
        self.apply(settings);
    }

    /// Ayarlardaki kısayolları yeniden kaydeder (açılışta ve her değişiklikte).
    pub fn apply(&self, settings: &AppSettings) {
        let results = {
            let inner = self.inner.borrow();
            let Some(manager) = inner.manager.as_ref() else { return };
            manager.apply(&ShortcutBindings::read_all(settings))
        };
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            inner.results = results;
            inner.results_changed.clone()
        };
        for f in listeners {
            f();
        }
    }

    /// Ayarlardaki kutu için bir sonraki kombinasyonu yakalar; kanca kurulamazsa false (kutu tuş olaylarıyla çalışır).
    pub fn begin_capture(&self, on_captured: impl Fn(Hotkey) + Send + Sync + 'static) -> bool {
        let hook = self.inner.borrow().hook.clone();
        hook.is_some_and(|h| h.begin_capture(Box::new(on_captured)))
    }

    pub fn end_capture(&self) {
        let hook = self.inner.borrow().hook.clone();
        if let Some(h) = hook {
            h.end_capture();
        }
    }

    pub fn dispose(&self) {
        let mut inner = self.inner.borrow_mut();
        if let Some(m) = inner.manager.take() {
            m.dispose();
        }
        inner.hotkeys = None;
        inner.hook = None;
        inner.window = None;
    }
}

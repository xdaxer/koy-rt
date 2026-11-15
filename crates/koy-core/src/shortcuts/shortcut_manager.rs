//! Karşılığı: `Shortcuts/ShortcutManager.cs`.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::hotkey::{Hotkey, HotkeyModifiers};
use super::i_hotkey_service::{HotkeyRegistrationResult, IHotkeyService};
use super::i_keyboard_hook_service::IKeyboardHookService;
use super::shortcut_bindings::{ShortcutAction, ShortcutBindings};
use crate::event::{Event, Subscription};
use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ShortcutStatus {
    /// Atanmamış: kayıt yapılmadı, hata değil.
    #[default]
    Unassigned,
    Registered,

    /// Metin çözülemedi ya da tuş kısayol olamıyor.
    Invalid,

    /// Windows'un hiçbir uygulamaya bırakmadığı kısayol (Win+L, Ctrl+Alt+Del).
    ReservedBySystem,

    /// Aynı kombinasyon başka bir Koy eylemine atanmış; bu eylem kaydedilmedi.
    DuplicateOfOtherAction,

    /// Başka bir uygulama kısayolu tutuyor (RegisterHotKey reddetti).
    InUseByOtherApp,

    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutResult {
    pub status: ShortcutStatus,
    pub conflicts_with: Option<ShortcutAction>,
}

impl ShortcutResult {
    /// C#: `new ShortcutResult(status, conflictsWith = null)`.
    pub fn new(status: ShortcutStatus, conflicts_with: Option<ShortcutAction>) -> Self {
        ShortcutResult { status, conflicts_with }
    }

    /// Çakışmasız sonuç (`conflictsWith` boş).
    pub fn of(status: ShortcutStatus) -> Self {
        Self::new(status, None)
    }

    pub fn is_error(&self) -> bool {
        !matches!(self.status, ShortcutStatus::Registered | ShortcutStatus::Unassigned)
    }
}

/// Kısayol kombinasyonu için ön denetimler (kayıt denemeden önce).
pub struct ShortcutConflicts;

impl ShortcutConflicts {
    /// Kancayla bile ezilemeyen kombinasyonlar: bunları Windows, uygulamalardan önce işler.
    pub fn is_reserved_by_system(hotkey: Hotkey) -> bool {
        let m = hotkey.modifiers;
        let ctrl_alt = (m & (HotkeyModifiers::Control | HotkeyModifiers::Alt)) == (HotkeyModifiers::Control | HotkeyModifiers::Alt);
        (m == HotkeyModifiers::Win && hotkey.virtual_key == 0x4C) || (ctrl_alt && hotkey.virtual_key == 0x2E)
    }

    /// Windows'un kullandığı ama kancayla ezilebilen kombinasyonlar; RegisterHotKey bunları alamaz.
    pub fn is_windows_owned(hotkey: Hotkey) -> bool {
        let m = hotkey.modifiers;
        let vk = hotkey.virtual_key;
        let win = m.intersects(HotkeyModifiers::Win);
        let alt = m.intersects(HotkeyModifiers::Alt);
        let ctrl = m.intersects(HotkeyModifiers::Control);

        // Alt+Tab, Alt+Esc, Alt+F4, Alt+Space (pencere menüsü)
        if alt && !ctrl && !win && matches!(vk, 0x09 | 0x1B | 0x73 | 0x20) {
            return true;
        }

        // Ctrl+Alt+Tab, Win+Tab
        if vk == 0x09 && (win || (ctrl && alt)) {
            return true;
        }

        // Ctrl+Esc (Başlat), Ctrl+Shift+Esc (Görev Yöneticisi)
        if ctrl && !alt && !win && vk == 0x1B {
            return true;
        }

        // Yalnız Win + harf/rakam/boşluk: Windows neredeyse hepsini kullanır (D, E, L, R, K, A, S, V...).
        if m == HotkeyModifiers::Win && matches!(vk, 0x41..=0x5A | 0x30..=0x39 | 0x20) {
            return true;
        }

        // Win+Shift+S (ekran alıntısı), Win+Shift+M
        if m == (HotkeyModifiers::Win | HotkeyModifiers::Shift) && matches!(vk, 0x53 | 0x4D) {
            return true;
        }

        false
    }

    /// Kullanıcının seçtiği kombinasyon kaydedilmeden önce denetlenir: sistem kısayolu ya da başka eylemle çakışma.
    /// Sorun yoksa `None` döner. Eylemler C# sözlüğünün dolaşım sırasıyla (`ShortcutBindings::all()` sırası) denetlenir.
    pub fn check(hotkey: Hotkey, for_action: ShortcutAction, current: &HashMap<ShortcutAction, String>) -> Option<ShortcutResult> {
        if !hotkey.is_valid() {
            return Some(ShortcutResult::of(ShortcutStatus::Invalid));
        }
        if Self::is_reserved_by_system(hotkey) {
            return Some(ShortcutResult::of(ShortcutStatus::ReservedBySystem));
        }

        for &action in ShortcutBindings::all() {
            let Some(text) = current.get(&action) else { continue };
            if action != for_action && Hotkey::try_parse(text.as_str()) == Some(hotkey) {
                return Some(ShortcutResult::new(ShortcutStatus::DuplicateOfOtherAction, Some(action)));
            }
        }

        None
    }
}

#[derive(Default)]
struct ManagerState {
    registered: HashSet<ShortcutAction>,
    hooked: HashSet<ShortcutAction>,
}

fn id_of(action: ShortcutAction) -> i32 {
    action as i32 + 1
}

/// Ayarlardaki kısayolları `IHotkeyService` üzerinden kaydeder, basılınca eylemi bildirir.
/// Saf mantık: gerçek kayıt platform servisindedir, bu yüzden sahte servisle test edilir.
pub struct ShortcutManager {
    service: Arc<dyn IHotkeyService>,
    hook: Option<Arc<dyn IKeyboardHookService>>,
    state: Arc<Mutex<ManagerState>>,
    triggered: Arc<Event<ShortcutAction>>,
    service_subscription: Subscription,
    hook_subscription: Option<Subscription>,
    disposed: AtomicBool,
}

impl ShortcutManager {
    /// C#: `new ShortcutManager(service, hook = null)`.
    pub fn new(service: Arc<dyn IHotkeyService>, hook: Option<Arc<dyn IKeyboardHookService>>) -> Self {
        let state = Arc::new(Mutex::new(ManagerState::default()));
        let triggered: Arc<Event<ShortcutAction>> = Arc::new(Event::new());

        let service_subscription = {
            let state = Arc::clone(&state);
            let triggered = Arc::clone(&triggered);
            service.hotkey_pressed().subscribe(move |id| {
                let Some(action) = ShortcutAction::from_i32(*id - 1) else { return };
                let known = state.lock().unwrap().registered.contains(&action);
                if known {
                    triggered.invoke(&action);
                }
            })
        };

        let hook_subscription = hook.as_ref().map(|hook| {
            let state = Arc::clone(&state);
            let triggered = Arc::clone(&triggered);
            hook.hotkey_pressed().subscribe(move |id| {
                let Some(action) = ShortcutAction::from_i32(*id - 1) else { return };
                let known = state.lock().unwrap().hooked.contains(&action);
                if known {
                    triggered.invoke(&action);
                }
            })
        });

        ShortcutManager { service, hook, state, triggered, service_subscription, hook_subscription, disposed: AtomicBool::new(false) }
    }

    /// Kayıtlı bir kısayola basıldığında eylemi bildirir. (C#: `Triggered`)
    pub fn triggered(&self) -> &Event<ShortcutAction> {
        &self.triggered
    }

    /// Önce bütün kayıtları kaldırır, sonra istenen kısayolları sırayla kaydeder.
    /// Aynı kombinasyonu iki eylem isterse ilk sıradaki kazanır, sonraki `DuplicateOfOtherAction` olur.
    pub fn apply(&self, desired: &HashMap<ShortcutAction, String>) -> HashMap<ShortcutAction, ShortcutResult> {
        self.unregister_all();

        let mut results: HashMap<ShortcutAction, ShortcutResult> = HashMap::new();
        let mut taken: HashMap<Hotkey, ShortcutAction> = HashMap::new();
        let mut hook_bindings: HashMap<i32, Hotkey> = HashMap::new();
        let mut registered: HashSet<ShortcutAction> = HashSet::new();
        let mut hooked: HashSet<ShortcutAction> = HashSet::new();

        for &action in ShortcutBindings::all() {
            let text = desired.get(&action).map(String::as_str);
            if text::is_null_or_white_space(text) {
                results.insert(action, ShortcutResult::of(ShortcutStatus::Unassigned));
                continue;
            }

            let Some(hotkey) = Hotkey::try_parse(text) else {
                results.insert(action, ShortcutResult::of(ShortcutStatus::Invalid));
                continue;
            };

            if ShortcutConflicts::is_reserved_by_system(hotkey) {
                results.insert(action, ShortcutResult::of(ShortcutStatus::ReservedBySystem));
                continue;
            }

            if let Some(&owner) = taken.get(&hotkey) {
                results.insert(action, ShortcutResult::new(ShortcutStatus::DuplicateOfOtherAction, Some(owner)));
                continue;
            }

            if self.needs_hook(hotkey) {
                let r = self.use_hook(action, hotkey, &mut hook_bindings, &mut taken, &mut hooked);
                results.insert(action, r);
                continue;
            }

            let registration = self.service.register(id_of(action), hotkey);

            // Win'li bir kombinasyonu tutan büyük olasılıkla Windows'tur: kullanıcının seçimi kazanır.
            if registration == HotkeyRegistrationResult::AlreadyInUse && self.hook.is_some() && hotkey.modifiers.intersects(HotkeyModifiers::Win) {
                let r = self.use_hook(action, hotkey, &mut hook_bindings, &mut taken, &mut hooked);
                results.insert(action, r);
                continue;
            }

            results.insert(
                action,
                match registration {
                    HotkeyRegistrationResult::Success => ShortcutResult::of(ShortcutStatus::Registered),
                    HotkeyRegistrationResult::AlreadyInUse => ShortcutResult::of(ShortcutStatus::InUseByOtherApp),
                    HotkeyRegistrationResult::Failed => ShortcutResult::of(ShortcutStatus::Failed),
                },
            );

            if registration == HotkeyRegistrationResult::Success {
                taken.insert(hotkey, action);
                registered.insert(action);
            }
        }

        if let Some(hook) = &self.hook {
            if !hook.set_bindings(&hook_bindings) {
                for action in &hooked {
                    results.insert(*action, ShortcutResult::of(ShortcutStatus::Failed));
                }
                hooked.clear();
            }
        }

        let mut st = self.state.lock().unwrap();
        st.registered = registered;
        st.hooked = hooked;

        results
    }

    fn needs_hook(&self, hotkey: Hotkey) -> bool {
        hotkey.is_modifier_only() || (self.hook.is_some() && ShortcutConflicts::is_windows_owned(hotkey))
    }

    fn use_hook(
        &self,
        action: ShortcutAction,
        hotkey: Hotkey,
        bindings: &mut HashMap<i32, Hotkey>,
        taken: &mut HashMap<Hotkey, ShortcutAction>,
        hooked: &mut HashSet<ShortcutAction>,
    ) -> ShortcutResult {
        if self.hook.is_none() {
            return ShortcutResult::of(ShortcutStatus::Failed);
        }

        bindings.insert(id_of(action), hotkey);
        taken.insert(hotkey, action);
        hooked.insert(action);
        ShortcutResult::of(ShortcutStatus::Registered)
    }

    fn unregister_all(&self) {
        let registered: Vec<ShortcutAction> = {
            let mut st = self.state.lock().unwrap();
            let r = st.registered.drain().collect();
            st.hooked.clear();
            r
        };
        for action in registered {
            self.service.unregister(id_of(action));
        }
    }

    /// C#: `Dispose` (birden çok çağrıya dayanıklı; `Drop` da çağırır).
    pub fn dispose(&self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }

        self.service.hotkey_pressed().unsubscribe(self.service_subscription);
        self.unregister_all();
        self.service.dispose();
        if let Some(hook) = &self.hook {
            if let Some(sub) = self.hook_subscription {
                hook.hotkey_pressed().unsubscribe(sub);
            }
            hook.dispose();
        }
    }
}

impl Drop for ShortcutManager {
    fn drop(&mut self) {
        self.dispose();
    }
}

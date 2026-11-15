//! Karşılığı: `ShortcutTests.cs` (HotkeyTests, ShortcutDefaultsTests, ShortcutConflictsTests, ShortcutManagerTests,
//! ShortcutManagerHookTests).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::event::Event;
use crate::settings::{AppSettings, SettingsStore};
use crate::shortcuts::*;

fn parse(text: &str) -> Hotkey {
    Hotkey::try_parse(text).unwrap_or_else(|| panic!("{text}"))
}

// ---------------------------------------------------------------------------------------------------------------------
// HotkeyTests
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn try_parse_accepts_valid_text() {
    let ctrl_alt = HotkeyModifiers::Control | HotkeyModifiers::Alt;
    let cases: [(&str, HotkeyModifiers, i32); 11] = [
        ("Ctrl+Alt+N", ctrl_alt, 0x4E),
        ("ctrl+alt+space", ctrl_alt, 0x20),
        ("Win+Shift+F5", HotkeyModifiers::Win | HotkeyModifiers::Shift, 0x74),
        ("Control + Alt + 7", ctrl_alt, 0x37),
        ("Alt+PageDown", HotkeyModifiers::Alt, 0x22),
        ("N", HotkeyModifiers::None, 0x4E),
        ("F9", HotkeyModifiers::None, 0x78),
        ("Shift+N", HotkeyModifiers::Shift, 0x4E),
        ("Win", HotkeyModifiers::Win, 0),
        ("Win+Shift", HotkeyModifiers::Win | HotkeyModifiers::Shift, 0),
        ("Ctrl+Alt", ctrl_alt, 0),
    ];
    for (text, modifiers, vk) in cases {
        let hotkey = Hotkey::try_parse(text).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(Hotkey::new(modifiers, vk), hotkey, "{text}");
    }
}

#[test]
fn try_parse_rejects_invalid_text() {
    // "Ctrl+Alt+N+M": iki ana tuş
    for text in ["", "   ", "Ctrl+Alt+N+M", "Ctrl++N", "Ctrl+Alt+Foo", "Ctrl+F25"] {
        assert_eq!(None, Hotkey::try_parse(text), "{text:?}");
    }
}

#[test]
fn try_parse_rejects_null() {
    assert_eq!(None, Hotkey::try_parse(None));
}

#[test]
fn format_then_parse_round_trips() {
    for text in [
        "Ctrl+Alt+N",
        "Ctrl+Alt+Shift+Win+F12",
        "Alt+Left",
        "Ctrl+Alt+Space",
        "Win+Tab",
        "Win",
        "Shift+Win",
        "F9",
        "Shift+N",
    ] {
        let hotkey = Hotkey::try_parse(text).unwrap_or_else(|| panic!("{text}"));
        // "Shift+Win" kanonik sırada "Shift+Win"dir (Ctrl, Alt, Shift, Win).
        assert_eq!(text, hotkey.format());
        let again = Hotkey::try_parse(hotkey.format().as_str()).unwrap();
        assert_eq!(hotkey, again, "{text}");
    }
}

#[test]
fn format_orders_modifiers_canonically() {
    let hotkey = Hotkey::try_parse("Win+Alt+Ctrl+Shift+A").unwrap();
    assert_eq!("Ctrl+Alt+Shift+Win+A", hotkey.format());
}

#[test]
fn modifier_keys_are_not_valid_main_keys() {
    // VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN
    for vk in [0x11, 0x10, 0x12, 0x5B] {
        assert!(!Hotkey::new(HotkeyModifiers::Control, vk).is_valid(), "{vk:#x}");
    }
}

#[test]
fn modifier_values_match_win32_mod_flags() {
    assert_eq!(1, HotkeyModifiers::Alt.0);
    assert_eq!(2, HotkeyModifiers::Control.0);
    assert_eq!(4, HotkeyModifiers::Shift.0);
    assert_eq!(8, HotkeyModifiers::Win.0);
}

// ---------------------------------------------------------------------------------------------------------------------
// ShortcutDefaultsTests
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn defaults_parse_are_unique_and_avoid_win_shortcuts() {
    let mut seen: HashSet<Hotkey> = HashSet::new();
    for &action in ShortcutBindings::all() {
        // Boş varsayılan = atanmamış (kullanıcı kendisi bağlar); yalnız dolu olanlar çözülür ve tekil olmalı.
        if ShortcutBindings::default_text(action).is_empty() {
            continue;
        }

        let hotkey = Hotkey::try_parse(ShortcutBindings::default_text(action)).unwrap_or_else(|| panic!("{action:?}"));
        assert!(seen.insert(hotkey), "{action:?} varsayılanı başka eylemle aynı");
        // Arama paneli proje sahibinin kararıyla Win+Shift+Q kullanır; sistem çakışması yine aşağıda denetlenir.
        if action != ShortcutAction::OpenSearch {
            assert!(!hotkey.modifiers.intersects(HotkeyModifiers::Win), "{action:?} Win kullanıyor");
        }
        assert!(!ShortcutConflicts::is_reserved_by_system(hotkey), "{action:?} sistem kısayoluyla çakışıyor");
    }
}

#[test]
fn is_default_compares_the_combination_not_the_spelling() {
    let cases = [
        ("Win+Shift+Q", true),
        ("Shift+Win+Q", true),
        ("win + shift + q", true),
        ("Win+Shift+Z", false),
        ("Alt+F2", false),
        ("", false),
    ];
    for (text, expected) in cases {
        assert_eq!(expected, ShortcutBindings::is_default(ShortcutAction::OpenSearch, text), "{text:?}");
    }
}

#[test]
fn app_settings_defaults_match_shortcut_bindings_defaults() {
    let settings = AppSettings::default();
    for &action in ShortcutBindings::all() {
        assert_eq!(ShortcutBindings::default_text(action), ShortcutBindings::get_text(&settings, action), "{action:?}");
    }
}

#[test]
fn with_text_changes_only_the_given_action() {
    let settings = AppSettings::default();
    let changed = ShortcutBindings::with_text(&settings, ShortcutAction::OpenShelf, "Ctrl+Alt+Q");

    assert_eq!("Ctrl+Alt+Q", ShortcutBindings::get_text(&changed, ShortcutAction::OpenShelf));
    assert_eq!(
        ShortcutBindings::get_text(&settings, ShortcutAction::OpenMenu),
        ShortcutBindings::get_text(&changed, ShortcutAction::OpenMenu)
    );
    assert_ne!(settings, changed);
}

#[test]
fn shortcuts_survive_settings_save_and_load() {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("koy-shortcut-{nanos:x}-{}", std::process::id()));
    let result = std::panic::catch_unwind(|| {
        let store = SettingsStore::new(dir.join("settings.json"));
        let mut settings = ShortcutBindings::with_text(&AppSettings::default(), ShortcutAction::PlayPause, "Ctrl+Shift+F9");
        settings = ShortcutBindings::with_text(&settings, ShortcutAction::OpenMenu, "");

        store.save(&settings);
        let loaded = store.load();

        assert_eq!("Ctrl+Shift+F9", loaded.shortcuts.play_pause);
        assert_eq!("", loaded.shortcuts.open_menu);
        assert_eq!(settings, loaded);
    });
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// ShortcutConflictsTests
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn only_combinations_windows_never_gives_up_are_reserved() {
    for text in ["Ctrl+Alt+Delete", "Win+L"] {
        assert!(ShortcutConflicts::is_reserved_by_system(parse(text)), "{text}");
    }
}

#[test]
fn windows_shortcuts_can_be_overridden() {
    for text in ["Alt+Tab", "Alt+F4", "Ctrl+Esc", "Ctrl+Shift+Esc", "Win+D", "Win+S", "Win+Tab", "Win+Shift+S"] {
        assert!(!ShortcutConflicts::is_reserved_by_system(parse(text)), "{text}");
        assert!(ShortcutConflicts::is_windows_owned(parse(text)), "{text}");
    }
}

#[test]
fn ordinary_shortcuts_are_not_reserved() {
    for text in ["Ctrl+Alt+N", "Ctrl+Alt+Space", "Ctrl+Shift+F9", "Win+Alt+K"] {
        assert!(!ShortcutConflicts::is_reserved_by_system(parse(text)), "{text}");
    }
}

#[test]
fn check_reports_duplicate_with_the_other_action() {
    let current = ShortcutBindings::read_all(&AppSettings::default());

    let result = ShortcutConflicts::check(parse("Ctrl+Alt+R"), ShortcutAction::ToggleNotch, &current);

    let result = result.expect("sonuç bekleniyordu");
    assert_eq!(ShortcutStatus::DuplicateOfOtherAction, result.status);
    assert_eq!(Some(ShortcutAction::OpenShelf), result.conflicts_with);
}

#[test]
fn check_allows_keeping_the_own_shortcut() {
    let current = ShortcutBindings::read_all(&AppSettings::default());

    assert_eq!(None, ShortcutConflicts::check(parse("Ctrl+Alt+N"), ShortcutAction::ToggleNotch, &current));
}

#[test]
fn check_rejects_reserved_and_invalid() {
    let current = ShortcutBindings::read_all(&AppSettings::default());

    assert_eq!(
        Some(ShortcutStatus::ReservedBySystem),
        ShortcutConflicts::check(parse("Win+L"), ShortcutAction::OpenMenu, &current).map(|r| r.status)
    );
    assert_eq!(
        Some(ShortcutStatus::Invalid),
        ShortcutConflicts::check(Hotkey::default(), ShortcutAction::OpenMenu, &current).map(|r| r.status)
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// ShortcutManagerTests
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct FakeHotkeyState {
    registered: HashMap<i32, Hotkey>,
    taken_by_others: HashSet<Hotkey>,
    log: Vec<String>,
    disposed: bool,
}

#[derive(Default)]
struct FakeHotkeyService {
    state: Mutex<FakeHotkeyState>,
    hotkey_pressed: Event<i32>,
}

impl FakeHotkeyService {
    fn press(&self, id: i32) {
        self.hotkey_pressed.invoke(&id);
    }
    fn registered(&self) -> HashMap<i32, Hotkey> {
        self.state.lock().unwrap().registered.clone()
    }
    fn log(&self) -> Vec<String> {
        self.state.lock().unwrap().log.clone()
    }
    fn clear_log(&self) {
        self.state.lock().unwrap().log.clear();
    }
    fn take_by_other(&self, hotkey: Hotkey) {
        self.state.lock().unwrap().taken_by_others.insert(hotkey);
    }
    fn disposed(&self) -> bool {
        self.state.lock().unwrap().disposed
    }
}

impl IHotkeyService for FakeHotkeyService {
    fn hotkey_pressed(&self) -> &Event<i32> {
        &self.hotkey_pressed
    }

    fn register(&self, id: i32, hotkey: Hotkey) -> HotkeyRegistrationResult {
        let mut st = self.state.lock().unwrap();
        st.log.push(format!("reg {hotkey}"));
        if st.taken_by_others.contains(&hotkey) {
            return HotkeyRegistrationResult::AlreadyInUse;
        }
        st.registered.insert(id, hotkey);
        HotkeyRegistrationResult::Success
    }

    fn unregister(&self, id: i32) {
        let mut st = self.state.lock().unwrap();
        st.log.push(format!("unreg {id}"));
        st.registered.remove(&id);
    }

    fn dispose(&self) {
        self.state.lock().unwrap().disposed = true;
    }
}

/// Varsayılanı dolu (kayıt edilecek) eylem sayısı; boş varsayılanlar atanmamış gelir.
fn default_bound_count() -> usize {
    ShortcutBindings::all().iter().filter(|a| !ShortcutBindings::default_text(**a).is_empty()).count()
}

fn desired(settings: Option<&AppSettings>) -> HashMap<ShortcutAction, String> {
    match settings {
        Some(s) => ShortcutBindings::read_all(s),
        None => ShortcutBindings::read_all(&AppSettings::default()),
    }
}

fn new_manager(fake: &Arc<FakeHotkeyService>) -> ShortcutManager {
    ShortcutManager::new(fake.clone(), None)
}

#[test]
fn apply_registers_all_defaults() {
    let fake = Arc::new(FakeHotkeyService::default());
    let manager = new_manager(&fake);

    let results = manager.apply(&desired(None));

    let with_default: Vec<ShortcutAction> =
        ShortcutBindings::all().iter().copied().filter(|a| !ShortcutBindings::default_text(*a).is_empty()).collect();
    assert_eq!(with_default.len(), fake.registered().len());
    for a in &with_default {
        assert_eq!(ShortcutStatus::Registered, results[a].status, "{a:?}");
    }
    assert_eq!(ShortcutStatus::Unassigned, results[&ShortcutAction::OpenKoyWindow].status);
}

#[test]
fn apply_reports_other_app_holding_the_shortcut_and_keeps_the_rest() {
    let fake = Arc::new(FakeHotkeyService::default());
    fake.take_by_other(parse("Ctrl+Alt+R"));
    let manager = new_manager(&fake);

    let results = manager.apply(&desired(None));

    assert_eq!(ShortcutStatus::InUseByOtherApp, results[&ShortcutAction::OpenShelf].status);
    assert!(results[&ShortcutAction::OpenShelf].is_error());
    assert_eq!(ShortcutStatus::Registered, results[&ShortcutAction::ToggleNotch].status);
    assert_eq!(default_bound_count() - 1, fake.registered().len());
}

#[test]
fn apply_treats_empty_text_as_unassigned_without_error() {
    let fake = Arc::new(FakeHotkeyService::default());
    let manager = new_manager(&fake);
    let settings = ShortcutBindings::with_text(&AppSettings::default(), ShortcutAction::OpenMenu, "");

    let results = manager.apply(&desired(Some(&settings)));

    assert_eq!(ShortcutStatus::Unassigned, results[&ShortcutAction::OpenMenu].status);
    assert!(!results[&ShortcutAction::OpenMenu].is_error());
    assert_eq!(default_bound_count() - 1, fake.registered().len());
}

#[test]
fn apply_marks_unparseable_text_invalid_and_never_registers_it() {
    let fake = Arc::new(FakeHotkeyService::default());
    let manager = new_manager(&fake);
    let settings = ShortcutBindings::with_text(&AppSettings::default(), ShortcutAction::OpenMenu, "Ctrl+Banana");

    let results = manager.apply(&desired(Some(&settings)));

    assert_eq!(ShortcutStatus::Invalid, results[&ShortcutAction::OpenMenu].status);
    assert!(!fake.log().iter().any(|l| l.contains("Banana")));
}

#[test]
fn apply_gives_a_duplicate_to_the_first_action_only() {
    let fake = Arc::new(FakeHotkeyService::default());
    let manager = new_manager(&fake);
    let settings = ShortcutBindings::with_text(&AppSettings::default(), ShortcutAction::OpenMenu, "Ctrl+Alt+N");

    let results = manager.apply(&desired(Some(&settings)));

    assert_eq!(ShortcutStatus::Registered, results[&ShortcutAction::ToggleNotch].status);
    assert_eq!(ShortcutStatus::DuplicateOfOtherAction, results[&ShortcutAction::OpenMenu].status);
    assert_eq!(Some(ShortcutAction::ToggleNotch), results[&ShortcutAction::OpenMenu].conflicts_with);
}

#[test]
fn apply_does_not_try_to_register_system_reserved_shortcuts() {
    let fake = Arc::new(FakeHotkeyService::default());
    let manager = new_manager(&fake);
    let settings = ShortcutBindings::with_text(&AppSettings::default(), ShortcutAction::OpenMenu, "Win+L");

    let results = manager.apply(&desired(Some(&settings)));

    assert_eq!(ShortcutStatus::ReservedBySystem, results[&ShortcutAction::OpenMenu].status);
    assert!(!fake.log().iter().any(|l| l == "reg Win+L"));
}

#[test]
fn reapply_unregisters_old_registrations_first() {
    let fake = Arc::new(FakeHotkeyService::default());
    let manager = new_manager(&fake);
    manager.apply(&desired(None));
    fake.clear_log();

    let settings = ShortcutBindings::with_text(&AppSettings::default(), ShortcutAction::ToggleNotch, "Ctrl+Alt+Q");
    manager.apply(&desired(Some(&settings)));

    assert_eq!("unreg", fake.log()[0].split(' ').next().unwrap());
    assert_eq!(default_bound_count(), fake.registered().len());
    let values: Vec<Hotkey> = fake.registered().values().copied().collect();
    let ctrl_alt = HotkeyModifiers::Control | HotkeyModifiers::Alt;
    assert!(values.contains(&Hotkey::new(ctrl_alt, 0x51)));
    assert!(!values.contains(&Hotkey::new(ctrl_alt, 0x4E)));
}

#[test]
fn pressing_a_registered_hotkey_raises_triggered_with_its_action() {
    let fake = Arc::new(FakeHotkeyService::default());
    let manager = new_manager(&fake);
    manager.apply(&desired(None));
    let triggered: Arc<Mutex<Vec<ShortcutAction>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&triggered);
    manager.triggered().subscribe(move |a| sink.lock().unwrap().push(*a));

    let target = Hotkey::new(HotkeyModifiers::Control | HotkeyModifiers::Alt, 0x20);
    let id = *fake.registered().iter().find(|(_, v)| **v == target).unwrap().0;
    fake.press(id);

    assert_eq!(vec![ShortcutAction::PlayPause], *triggered.lock().unwrap());
}

#[test]
fn pressing_an_unregistered_id_is_ignored() {
    let fake = Arc::new(FakeHotkeyService::default());
    let manager = new_manager(&fake);
    manager.apply(&desired(Some(&ShortcutBindings::with_text(&AppSettings::default(), ShortcutAction::OpenMenu, ""))));
    let triggered: Arc<Mutex<Vec<ShortcutAction>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&triggered);
    manager.triggered().subscribe(move |a| sink.lock().unwrap().push(*a));

    fake.press(ShortcutAction::OpenMenu as i32 + 1);
    fake.press(999);

    assert!(triggered.lock().unwrap().is_empty());
}

#[test]
fn dispose_unregisters_everything_and_disposes_the_service() {
    let fake = Arc::new(FakeHotkeyService::default());
    let manager = new_manager(&fake);
    manager.apply(&desired(None));

    manager.dispose();

    assert!(fake.registered().is_empty());
    assert!(fake.disposed());
}

// ---------------------------------------------------------------------------------------------------------------------
// ShortcutManagerHookTests
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct FakeHotkeys {
    taken: Mutex<HashSet<Hotkey>>,
    registered: Mutex<Vec<Hotkey>>,
    hotkey_pressed: Event<i32>,
}

impl IHotkeyService for FakeHotkeys {
    fn hotkey_pressed(&self) -> &Event<i32> {
        &self.hotkey_pressed
    }

    fn register(&self, _id: i32, hotkey: Hotkey) -> HotkeyRegistrationResult {
        if self.taken.lock().unwrap().contains(&hotkey) {
            return HotkeyRegistrationResult::AlreadyInUse;
        }
        self.registered.lock().unwrap().push(hotkey);
        HotkeyRegistrationResult::Success
    }

    fn unregister(&self, _id: i32) {}

    fn dispose(&self) {}
}

struct FakeHook {
    bindings: Mutex<HashMap<i32, Hotkey>>,
    installs: bool,
    hotkey_pressed: Event<i32>,
}

impl FakeHook {
    fn new(installs: bool) -> Self {
        FakeHook { bindings: Mutex::new(HashMap::new()), installs, hotkey_pressed: Event::new() }
    }
    fn bindings(&self) -> HashMap<i32, Hotkey> {
        self.bindings.lock().unwrap().clone()
    }
    fn press(&self, id: i32) {
        self.hotkey_pressed.invoke(&id);
    }
}

impl IKeyboardHookService for FakeHook {
    fn hotkey_pressed(&self) -> &Event<i32> {
        &self.hotkey_pressed
    }

    fn set_bindings(&self, bindings: &HashMap<i32, Hotkey>) -> bool {
        *self.bindings.lock().unwrap() = bindings.clone();
        self.installs || bindings.is_empty()
    }

    fn begin_capture(&self, _on_captured: Box<dyn Fn(Hotkey) + Send + Sync>) -> bool {
        true
    }

    fn end_capture(&self) {}

    fn dispose(&self) {}
}

fn search(text: &str) -> HashMap<ShortcutAction, String> {
    HashMap::from([(ShortcutAction::OpenSearch, text.to_string())])
}

#[test]
fn windows_owned_and_modifier_only_use_the_hook() {
    for text in ["Win", "Win+Shift", "Win+S", "Alt+Tab"] {
        let hotkeys = Arc::new(FakeHotkeys::default());
        let hook = Arc::new(FakeHook::new(true));
        let manager = ShortcutManager::new(hotkeys.clone(), Some(hook.clone()));

        let results = manager.apply(&search(text));

        assert_eq!(ShortcutStatus::Registered, results[&ShortcutAction::OpenSearch].status, "{text}");
        assert!(hook.bindings().values().any(|h| *h == parse(text)), "{text}");
        assert!(hotkeys.registered.lock().unwrap().is_empty(), "{text}");
    }
}

#[test]
fn ordinary_shortcut_does_not_install_the_hook() {
    let hotkeys = Arc::new(FakeHotkeys::default());
    let hook = Arc::new(FakeHook::new(true));
    let manager = ShortcutManager::new(hotkeys.clone(), Some(hook.clone()));

    manager.apply(&search("Win+Shift+Q"));

    assert!(hook.bindings().is_empty());
    assert_eq!(1, hotkeys.registered.lock().unwrap().len());
}

#[test]
fn win_combination_held_by_windows_falls_back_to_the_hook() {
    let hotkeys = Arc::new(FakeHotkeys::default());
    hotkeys.taken.lock().unwrap().insert(parse("Win+Shift+Space"));
    let hook = Arc::new(FakeHook::new(true));
    let manager = ShortcutManager::new(hotkeys.clone(), Some(hook.clone()));

    let results = manager.apply(&search("Win+Shift+Space"));

    assert_eq!(ShortcutStatus::Registered, results[&ShortcutAction::OpenSearch].status);
    assert_eq!(1, hook.bindings().len());
}

#[test]
fn combination_held_by_another_app_is_still_an_error() {
    let hotkeys = Arc::new(FakeHotkeys::default());
    hotkeys.taken.lock().unwrap().insert(parse("Ctrl+Alt+F9"));
    let hook = Arc::new(FakeHook::new(true));
    let manager = ShortcutManager::new(hotkeys.clone(), Some(hook.clone()));

    let results = manager.apply(&search("Ctrl+Alt+F9"));

    assert_eq!(ShortcutStatus::InUseByOtherApp, results[&ShortcutAction::OpenSearch].status);
    assert!(hook.bindings().is_empty());
}

#[test]
fn combinations_windows_never_gives_up_are_rejected() {
    for text in ["Win+L", "Ctrl+Alt+Delete"] {
        let hook = Arc::new(FakeHook::new(true));
        let manager = ShortcutManager::new(Arc::new(FakeHotkeys::default()), Some(hook.clone()));

        let results = manager.apply(&search(text));

        assert_eq!(ShortcutStatus::ReservedBySystem, results[&ShortcutAction::OpenSearch].status, "{text}");
    }
}

#[test]
fn hook_press_triggers_the_action() {
    let hook = Arc::new(FakeHook::new(true));
    let manager = ShortcutManager::new(Arc::new(FakeHotkeys::default()), Some(hook.clone()));
    manager.apply(&search("Win"));
    let fired: Arc<Mutex<Option<ShortcutAction>>> = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&fired);
    manager.triggered().subscribe(move |a| *sink.lock().unwrap() = Some(*a));

    let id = *hook.bindings().keys().next().unwrap();
    assert_eq!(1, hook.bindings().len());
    hook.press(id);

    assert_eq!(Some(ShortcutAction::OpenSearch), *fired.lock().unwrap());
}

#[test]
fn hook_that_cannot_install_is_reported_as_failed() {
    let hook = Arc::new(FakeHook::new(false));
    let manager = ShortcutManager::new(Arc::new(FakeHotkeys::default()), Some(hook.clone()));

    let results = manager.apply(&search("Win"));

    assert_eq!(ShortcutStatus::Failed, results[&ShortcutAction::OpenSearch].status);
}

#[test]
fn modifier_only_without_a_hook_fails_instead_of_reaching_register_hotkey() {
    let hotkeys = Arc::new(FakeHotkeys::default());
    let manager = ShortcutManager::new(hotkeys.clone(), None);

    let results = manager.apply(&search("Win"));

    assert_eq!(ShortcutStatus::Failed, results[&ShortcutAction::OpenSearch].status);
    assert!(hotkeys.registered.lock().unwrap().is_empty());
}

#[test]
fn typing_keys_are_flagged() {
    let cases = [
        ("N", true),
        ("Shift+N", true),
        ("Space", true),
        ("F9", false),
        ("Shift+F9", false),
        ("Ctrl+N", false),
        ("Win", false),
    ];
    for (text, expected) in cases {
        assert_eq!(expected, parse(text).captures_typing(), "{text}");
    }
}

#[test]
fn default_settings_do_not_need_the_hook() {
    let hook = Arc::new(FakeHook::new(true));
    let manager = ShortcutManager::new(Arc::new(FakeHotkeys::default()), Some(hook.clone()));

    manager.apply(&ShortcutBindings::read_all(&AppSettings::default()));

    assert!(hook.bindings().is_empty());
}

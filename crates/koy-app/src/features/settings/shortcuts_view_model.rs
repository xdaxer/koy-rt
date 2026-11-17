//! Karşılığı: `koy/src/App/Features/Settings/ShortcutsViewModel.cs` (ShortcutRowViewModel ile).
//!
//! Kısayollar sayfasının mantığı: satırlar, yakalama, kaydetme, varsayılana dönme ve kayıt hatalarının satıra yansıması.
//! Kısayollar ayarlara yazılır, uygulama kancasıyla (`SettingsHooks::apply_shortcuts`) yeniden kaydedilir. Tek başına
//! Win ve Windows'un kısayolları klavye kancasından yakalanır; kanca yoksa kutunun tuş olayları kullanılır.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use koy_core::shortcuts::{Hotkey, HotkeyModifiers, ShortcutAction, ShortcutBindings, ShortcutConflicts, ShortcutResult, ShortcutStatus};

use crate::features::shortcuts::shortcut_controller::ShortcutController;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;

use super::settings_view_model::SettingsViewModel;

#[derive(Clone, Debug)]
pub struct ShortcutRow {
    pub action: ShortcutAction,
    pub title: String,
    pub text: String,
    pub is_capturing: bool,
    pub error_text: Option<String>,
}

impl ShortcutRow {
    pub fn has_shortcut(&self) -> bool {
        !self.text.trim().is_empty()
    }

    pub fn is_default(&self) -> bool {
        ShortcutBindings::is_default(self.action, &self.text)
    }

    /// Rozet olarak gösterilecek tuş adları (Ctrl, Alt, N...).
    pub fn key_tokens(&self) -> Vec<String> {
        self.text
            .split('+')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| if t == "Space" { Strings::get("KeySpace") } else { t.to_string() })
            .collect()
    }
}

pub struct ShortcutsViewModel {
    vm: Rc<SettingsViewModel>,
    rows: RefCell<Vec<ShortcutRow>>,
    controller: RefCell<Option<Rc<ShortcutController>>>,
    hook_capture_row: Cell<Option<usize>>,
    pub changed: Notifier,
}

const VK_ESCAPE: i32 = 0x1B;
const VK_BACK: i32 = 0x08;
const VK_DELETE: i32 = 0x2E;

impl ShortcutsViewModel {
    pub fn new(vm: Rc<SettingsViewModel>) -> Rc<Self> {
        let rows = ShortcutBindings::all()
            .iter()
            .map(|a| ShortcutRow { action: *a, title: title_of(*a), text: String::new(), is_capturing: false, error_text: None })
            .collect();
        let me = Rc::new(ShortcutsViewModel {
            vm,
            rows: RefCell::new(rows),
            controller: RefCell::new(None),
            hook_capture_row: Cell::new(None),
            changed: Notifier::default(),
        });
        me.load();
        me
    }

    /// Denetleyici uygulama açılışında kurulur; kayıt sonuçları satırlara yansır.
    pub fn set_controller(self: &Rc<Self>, controller: Rc<ShortcutController>) {
        let me = Rc::downgrade(self);
        controller.on_results_changed(move || {
            if let Some(m) = me.upgrade() {
                crate::infra::dispatch::post_local(move || m.show_registration_results());
            }
        });
        *self.controller.borrow_mut() = Some(controller);
        self.show_registration_results();
    }

    pub fn rows(&self) -> Vec<ShortcutRow> {
        self.rows.borrow().clone()
    }

    /// Ayarlardaki metinleri satırlara yükler.
    pub fn load(&self) {
        let count = self.rows.borrow().len();
        for i in 0..count {
            self.cancel_capture(i);
        }
        let settings = self.vm.settings();
        for row in self.rows.borrow_mut().iter_mut() {
            row.text = ShortcutBindings::get_text(&settings, row.action);
            row.title = title_of(row.action);
        }
        self.show_registration_results();
    }

    pub fn start_capture(self: &Rc<Self>, index: usize) {
        // Başka satır yakalıyorsa bırakılır.
        let count = self.rows.borrow().len();
        for i in 0..count {
            if i != index {
                self.cancel_capture(i);
            }
        }
        {
            let mut rows = self.rows.borrow_mut();
            let Some(row) = rows.get_mut(index) else { return };
            row.error_text = None;
            row.is_capturing = true;
        }
        let controller = self.controller.borrow().clone();
        if let Some(c) = controller {
            let me = UiRef::new(self);
            let started = c.begin_capture(move |hotkey| {
                crate::infra::dispatch::post(move || {
                    if let Some(m) = me.upgrade() {
                        m.handle_captured(index, hotkey);
                    }
                });
            });
            if started {
                self.hook_capture_row.set(Some(index));
            }
        }
        self.changed.notify();
    }

    pub fn cancel_capture(&self, index: usize) {
        if let Some(row) = self.rows.borrow_mut().get_mut(index) {
            row.is_capturing = false;
        }
        if self.hook_capture_row.get() == Some(index) {
            self.hook_capture_row.set(None);
            if let Some(c) = self.controller.borrow().as_ref() {
                c.end_capture();
            }
        }
        self.changed.notify();
    }

    pub fn reset_to_default(&self, index: usize) {
        let Some(action) = self.rows.borrow().get(index).map(|r| r.action) else { return };
        self.assign(index, ShortcutBindings::default_text(action));
    }

    fn is_capturing(&self, index: usize) -> bool {
        self.rows.borrow().get(index).is_some_and(|r| r.is_capturing)
    }

    /// Klavye kancasından gelen kombinasyon.
    fn handle_captured(&self, index: usize, hotkey: Hotkey) {
        if self.is_capturing(index) {
            self.complete(index, hotkey);
        }
    }

    /// Yakalama kutusuna gelen tuş (Slint tuş metni ve değiştiriciler). true: tuş işlendi.
    pub fn handle_key(&self, index: usize, key_text: &str, ctrl: bool, alt: bool, shift: bool, win: bool) -> bool {
        if !self.is_capturing(index) {
            return false;
        }
        let Some(vk) = vk_of(key_text) else {
            // Yalnız değiştirici basılı ya da tanınmayan tuş: kombinasyon henüz bitmedi.
            return true;
        };
        let mut m = HotkeyModifiers::None;
        if ctrl {
            m = m | HotkeyModifiers::Control;
        }
        if alt {
            m = m | HotkeyModifiers::Alt;
        }
        if shift {
            m = m | HotkeyModifiers::Shift;
        }
        if win {
            m = m | HotkeyModifiers::Win;
        }
        self.complete(index, Hotkey::new(m, vk));
        true
    }

    fn complete(&self, index: usize, hotkey: Hotkey) {
        if hotkey.modifiers == HotkeyModifiers::None {
            if hotkey.virtual_key == VK_ESCAPE {
                self.cancel_capture(index);
                return;
            }
            if hotkey.virtual_key == VK_BACK || hotkey.virtual_key == VK_DELETE {
                self.cancel_capture(index);
                self.assign(index, "");
                return;
            }
        }
        if !hotkey.is_valid() {
            // Yakalama sürer: kullanıcı yeni bir kombinasyon deneyebilir.
            if let Some(row) = self.rows.borrow_mut().get_mut(index) {
                row.error_text = Some(Strings::get("ShortcutErrInvalid"));
            }
            self.changed.notify();
            return;
        }
        self.cancel_capture(index);
        self.assign(index, &hotkey.format());
    }

    /// Yeni kombinasyonu denetler, kaydeder ve yeniden kayıt ister. Sorun varsa satırda gösterilir.
    pub fn assign(&self, index: usize, text: &str) {
        let Some(action) = self.rows.borrow().get(index).map(|r| r.action) else { return };
        let settings = self.vm.store().load();
        if !text.is_empty() {
            let Some(hotkey) = Hotkey::try_parse(text) else {
                self.set_error(index, Some(Strings::get("ShortcutErrInvalid")));
                return;
            };
            // Windows'a ait ya da başka bir Koy eylemine atanmış kombinasyon kaydedilmez.
            if let Some(problem) = ShortcutConflicts::check(hotkey, action, &ShortcutBindings::read_all(&settings)) {
                let d = self.describe(&problem);
                self.set_error(index, Some(d));
                return;
            }
        }
        {
            let mut rows = self.rows.borrow_mut();
            rows[index].text = text.to_string();
            rows[index].error_text = None;
        }
        // Yeniden kayıt sonucu ResultsChanged ile satırlara döner.
        self.vm.commit_external("Shortcuts", ShortcutBindings::with_text(&settings, action, text));
        if self.controller.borrow().is_none() {
            self.show_registration_results();
        }
        self.changed.notify();
    }

    fn set_error(&self, index: usize, error: Option<String>) {
        if let Some(row) = self.rows.borrow_mut().get_mut(index) {
            row.error_text = error;
        }
        self.changed.notify();
    }

    fn show_registration_results(&self) {
        let results = self.controller.borrow().as_ref().map(|c| c.results());
        let descriptions: Vec<Option<String>> = self
            .rows
            .borrow()
            .iter()
            .map(|row| {
                if row.is_capturing {
                    return row.error_text.clone();
                }
                match results.as_ref().and_then(|r| r.get(&row.action)) {
                    Some(result) if result.is_error() => Some(self.describe(result)),
                    _ => match Hotkey::try_parse(row.text.as_str()) {
                        Some(h) if h.captures_typing() => Some(Strings::get("ShortcutWarnTyping")),
                        _ => None,
                    },
                }
            })
            .collect();
        for (row, d) in self.rows.borrow_mut().iter_mut().zip(descriptions) {
            row.error_text = d;
        }
        self.changed.notify();
    }

    fn describe(&self, result: &ShortcutResult) -> String {
        match result.status {
            ShortcutStatus::Invalid => Strings::get("ShortcutErrInvalid"),
            ShortcutStatus::ReservedBySystem => Strings::get("ShortcutErrReserved"),
            ShortcutStatus::DuplicateOfOtherAction => {
                let other = result.conflicts_with.map(title_of).unwrap_or_default();
                Strings::format("ShortcutErrDuplicate", &[&other])
            }
            ShortcutStatus::InUseByOtherApp => Strings::get("ShortcutErrInUse"),
            ShortcutStatus::Failed => Strings::get("ShortcutErrFailed"),
            _ => String::new(),
        }
    }
}

fn title_of(action: ShortcutAction) -> String {
    Strings::get(&format!("Shortcut{action:?}"))
}

/// Slint tuş metnini sanal tuş koduna çevirir (WPF `KeyInterop.VirtualKeyFromKey` karşılığı). Değiştiriciler None.
fn vk_of(text: &str) -> Option<i32> {
    use slint::platform::Key;
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_none() {
        if c.is_ascii_alphabetic() {
            return Some(c.to_ascii_uppercase() as i32);
        }
        if c.is_ascii_digit() {
            return Some(c as i32);
        }
        let named: &[(Key, i32)] = &[
            (Key::Backspace, 0x08),
            (Key::Tab, 0x09),
            (Key::Return, 0x0D),
            (Key::Escape, 0x1B),
            (Key::Space, 0x20),
            (Key::PageUp, 0x21),
            (Key::PageDown, 0x22),
            (Key::End, 0x23),
            (Key::Home, 0x24),
            (Key::LeftArrow, 0x25),
            (Key::UpArrow, 0x26),
            (Key::RightArrow, 0x27),
            (Key::DownArrow, 0x28),
            (Key::Insert, 0x2D),
            (Key::Delete, 0x2E),
            (Key::F1, 0x70),
            (Key::F2, 0x71),
            (Key::F3, 0x72),
            (Key::F4, 0x73),
            (Key::F5, 0x74),
            (Key::F6, 0x75),
            (Key::F7, 0x76),
            (Key::F8, 0x77),
            (Key::F9, 0x78),
            (Key::F10, 0x79),
            (Key::F11, 0x7A),
            (Key::F12, 0x7B),
        ];
        for (k, vk) in named {
            if char::from(k.clone()) == c {
                return Some(*vk);
            }
        }
        // Noktalama (OEM) tuşları ABD düzeninde.
        let oem = match c {
            ';' => 0xBA,
            '=' => 0xBB,
            ',' => 0xBC,
            '-' => 0xBD,
            '.' => 0xBE,
            '/' => 0xBF,
            '`' => 0xC0,
            '[' => 0xDB,
            '\\' => 0xDC,
            ']' => 0xDD,
            '\'' => 0xDE,
            _ => return None,
        };
        return Some(oem);
    }
    None
}

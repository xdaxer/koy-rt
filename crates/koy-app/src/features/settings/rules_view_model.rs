//! Karşılığı: `koy/src/App/Features/Settings/RulesViewModel.cs`.
//!
//! Kurallar bölümü (Genel sayfası): uygulamaya (exe) göre profil kuralı ekle, aç/kapat, sil. Karar mantığı Core'dadır
//! (`RuleListEditor`, `ContextProfileSwitcher`); bu sınıf yalnız metin, durum ve çalışan uygulama listesi. Yeni kural
//! kapalı eklenir.

use std::cell::RefCell;
use std::rc::Rc;

use koy_core::rules::{RuleEditResult, RuleListEditor};

use crate::features::rules::context_rule_service::ContextRuleService;
use crate::infra::observable::Notifier;
use crate::resources::strings::Strings;

use super::profile_service::ProfileService;

#[derive(Clone, Debug)]
pub struct RuleItem {
    pub index: i32,
    pub title: String,
    pub profile_text: String,
    pub is_enabled: bool,
}

#[derive(Clone, Debug)]
pub struct RuleProfileOption {
    pub id: String,
    pub name: String,
}

#[derive(Default)]
struct State {
    items: Vec<RuleItem>,
    profile_options: Vec<RuleProfileOption>,
    running_apps: Vec<String>,
    process_text: String,
    selected_profile: Option<usize>,
    status_text: String,
    status_is_error: bool,
}

pub struct RulesViewModel {
    profiles: Rc<ProfileService>,
    rules: Rc<ContextRuleService>,
    st: RefCell<State>,
    pub changed: Notifier,
}

impl RulesViewModel {
    pub fn new(profiles: Rc<ProfileService>, rules: Rc<ContextRuleService>) -> Rc<Self> {
        let me = Rc::new(RulesViewModel { profiles, rules, st: RefCell::new(State::default()), changed: Notifier::default() });
        me.refresh();
        me.refresh_running_apps();
        me
    }

    pub fn items(&self) -> Vec<RuleItem> {
        self.st.borrow().items.clone()
    }

    pub fn profile_options(&self) -> Vec<RuleProfileOption> {
        self.st.borrow().profile_options.clone()
    }

    pub fn running_apps(&self) -> Vec<String> {
        self.st.borrow().running_apps.clone()
    }

    pub fn selected_profile(&self) -> i32 {
        self.st.borrow().selected_profile.map_or(-1, |i| i as i32)
    }

    pub fn process_text(&self) -> String {
        self.st.borrow().process_text.clone()
    }

    pub fn status(&self) -> (String, bool) {
        let st = self.st.borrow();
        (st.status_text.clone(), st.status_is_error)
    }

    pub fn set_process_text(&self, text: &str) {
        self.st.borrow_mut().process_text = text.to_string();
    }

    /// Çalışan uygulama listesinden seçim: kutuya yazılır.
    pub fn pick_app(&self, index: usize) {
        let app = self.st.borrow().running_apps.get(index).cloned();
        if let Some(a) = app {
            self.st.borrow_mut().process_text = a;
            self.changed.notify();
        }
    }

    pub fn select_profile(&self, index: usize) {
        self.st.borrow_mut().selected_profile = Some(index);
        self.changed.notify();
    }

    /// Kural listesini ve profil seçeneklerini (dil / profil değişince) yeniden okur.
    pub fn refresh(&self) {
        let selected_id = {
            let st = self.st.borrow();
            st.selected_profile.and_then(|i| st.profile_options.get(i)).map(|p| p.id.clone())
        };
        let options: Vec<RuleProfileOption> =
            self.profiles.list().into_iter().map(|p| RuleProfileOption { id: p.id, name: p.name }).collect();
        let selected = selected_id.and_then(|id| options.iter().position(|o| o.id == id)).or(if options.is_empty() { None } else { Some(0) });
        let items = self
            .rules
            .load_drafts()
            .iter()
            .enumerate()
            .map(|(i, draft)| {
                let name = options
                    .iter()
                    .find(|o| Some(&o.id) == draft.profile_id.as_ref())
                    .map(|o| o.name.clone())
                    .unwrap_or_else(|| Strings::get("RulesMissingProfile"));
                // Tek süreç koşullu kuralda exe adı; elle yazılmış gelişmiş kuralda genel ad.
                let title = match RuleListEditor::process_of(draft) {
                    Some(p) => format!("{p}.exe"),
                    None => format!("{} ({})", draft.name.clone().unwrap_or_default(), Strings::get("RulesAdvancedRule")),
                };
                RuleItem { index: i as i32, title, profile_text: Strings::format("RulesUsesProfile", &[&name]), is_enabled: draft.enabled }
            })
            .collect();
        {
            let mut st = self.st.borrow_mut();
            st.profile_options = options;
            st.selected_profile = selected;
            st.items = items;
        }
        self.changed.notify();
    }

    /// Penceresi olan çalışan uygulamaların exe adları (bir kez, düğmeyle; yoklama yok).
    pub fn refresh_running_apps(&self) {
        let mut names = running_app_names();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        self.st.borrow_mut().running_apps = names;
        self.changed.notify();
    }

    pub fn add(&self) {
        let (process, profile_id) = {
            let st = self.st.borrow();
            (st.process_text.clone(), st.selected_profile.and_then(|i| st.profile_options.get(i)).map(|p| p.id.clone()))
        };
        let (result, saved) = self.rules.edit(|rules| RuleListEditor::add_process_rule(rules, Some(&process), profile_id.as_deref()));
        if !result.is_success() {
            self.report(&format!("RulesError{:?}", result.error), true);
            return;
        }
        if !saved {
            self.report("RulesSaveFailed", true);
            return;
        }
        self.st.borrow_mut().process_text.clear();
        self.refresh();
        self.report("RulesAdded", false);
    }

    pub fn set_enabled(&self, index: usize, enabled: bool) {
        let Some(i) = self.st.borrow().items.get(index).map(|it| it.index) else { return };
        let (result, saved) = self.rules.edit(|rules| RuleListEditor::set_enabled(rules, i, enabled));
        if !result.is_success() || !saved {
            self.report_edit_failure(&result);
            self.refresh();
        }
    }

    pub fn delete(&self, index: usize) {
        let Some(i) = self.st.borrow().items.get(index).map(|it| it.index) else { return };
        let (result, saved) = self.rules.edit(|rules| RuleListEditor::delete(rules, i));
        if !result.is_success() || !saved {
            self.report_edit_failure(&result);
        }
        self.refresh();
    }

    fn report_edit_failure(&self, result: &RuleEditResult) {
        if result.is_success() {
            self.report("RulesSaveFailed", true);
        } else {
            self.report(&format!("RulesError{:?}", result.error), true);
        }
    }

    fn report(&self, key: &str, error: bool) {
        {
            let mut st = self.st.borrow_mut();
            st.status_text = Strings::get(key);
            st.status_is_error = error;
        }
        self.changed.notify();
    }
}

/// Görünür, sahipsiz üst düzey penceresi olan süreçlerin exe adları (C# `Process.MainWindowHandle != 0`), Koy hariç.
fn running_app_names() -> Vec<String> {
    use koy_platform::shell::WindowStyles;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GW_OWNER, GetWindow, IsWindowVisible};
    let mut handles: Vec<isize> = Vec::new();
    unsafe extern "system" fn collect(h: HWND, l: LPARAM) -> windows::core::BOOL {
        // SAFETY: l, çağıranın yaşayan vektörünü gösterir.
        let v = unsafe { &mut *(l.0 as *mut Vec<isize>) };
        let owned = unsafe { GetWindow(h, GW_OWNER) }.is_ok_and(|o| !o.is_invalid());
        if unsafe { IsWindowVisible(h) }.as_bool() && !owned {
            v.push(h.0 as isize);
        }
        windows::core::BOOL(1)
    }
    // SAFETY: geri çağırma yalnız bu çağrı sürerken vektöre yazar.
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut handles as *mut _ as isize));
    }
    let own = std::process::id();
    handles
        .into_iter()
        .filter(|h| WindowStyles::get_window_process_id(*h) != own)
        .filter_map(WindowStyles::get_window_process_name)
        .collect()
}

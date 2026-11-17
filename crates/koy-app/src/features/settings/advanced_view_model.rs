//! Karşılığı: `KoyWindowViewModel` Gelişmiş bölümü (`RebuildAdvancedRows`, `ApplyAdvanced`, `ResetAdvanced`) ve
//! `AdvancedSettingRow.cs`.
//!
//! Kendi arayüzü olmayan bütün ayarlar bölüm bazında tablo olarak listelenir. Değer doğrulanmadan yazılmaz: Core'a
//! gider (`AdvancedSettings::try_set`), geçersizse satır kırmızı ipucu gösterir ve dosyaya dokunulmaz. Tablo yalnız
//! sayfası açıkken tutulur.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use koy_core::settings::{AdvancedSettingError, AdvancedSettingKind, AdvancedSettings, SettingsKeyMap};

use crate::infra::observable::Notifier;
use crate::resources::strings::Strings;

use super::settings_view_model::SettingsViewModel;

#[derive(Clone, Debug)]
pub struct AdvancedRow {
    /// Grup başlığı satırı mı (yalnız `title` dolu).
    pub is_header: bool,
    pub key: String,
    pub title: String,
    pub kind: AdvancedSettingKind,
    pub value: String,
    pub default_text: String,
    pub choices: Vec<String>,
    pub is_changed: bool,
    pub error: String,
}

pub struct AdvancedViewModel {
    vm: Rc<SettingsViewModel>,
    filter: RefCell<String>,
    rows: RefCell<Vec<AdvancedRow>>,
    errors: RefCell<HashMap<String, String>>,
    pub changed: Notifier,
}

/// Kendi arayüzü olan ayarlar Gelişmiş tablosuna girmez (VM özellikleri düz v3 adlarıyla SettingsKeyMap'te).
fn dedicated_ui_keys() -> Vec<String> {
    SettingsKeyMap::v3_to_v4().values().map(|(section, name)| format!("{section}.{name}")).collect()
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.to_lowercase().contains(&needle.to_lowercase())
}

impl AdvancedViewModel {
    pub fn new(vm: Rc<SettingsViewModel>) -> Rc<Self> {
        Rc::new(AdvancedViewModel {
            vm,
            filter: RefCell::new(String::new()),
            rows: RefCell::new(Vec::new()),
            errors: RefCell::new(HashMap::new()),
            changed: Notifier::default(),
        })
    }

    pub fn rows(&self) -> Vec<AdvancedRow> {
        self.rows.borrow().clone()
    }

    pub fn filter(&self) -> String {
        self.filter.borrow().clone()
    }

    pub fn set_filter(&self, f: &str) {
        *self.filter.borrow_mut() = f.to_string();
        self.rebuild(true);
    }

    /// Sayfa kapalıyken tablo bırakılır.
    pub fn rebuild(&self, page_open: bool) {
        if !page_open {
            self.rows.borrow_mut().clear();
            self.errors.borrow_mut().clear();
            self.changed.notify();
            return;
        }
        let filter = self.filter.borrow().trim().to_string();
        let dedicated = dedicated_ui_keys();
        let dedicated: Vec<&str> = dedicated.iter().map(String::as_str).collect();
        let language = self.vm.settings().general.language;
        let all = AdvancedSettings::list(&self.vm.store().load(), &dedicated);
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|r| {
                filter.is_empty()
                    || contains_ci(&r.key, &filter)
                    || contains_ci(&r.get_friendly_title(Some(&language)), &filter)
                    || contains_ci(&r.get_section_title(Some(&language)), &filter)
            })
            .collect();
        let groups = AdvancedSettings::group_by_section(&filtered, Some(&language));
        let errors = self.errors.borrow();
        let mut rows = Vec::new();
        for g in groups {
            rows.push(AdvancedRow {
                is_header: true,
                key: g.section.clone(),
                title: g.title.clone(),
                kind: AdvancedSettingKind::Text,
                value: String::new(),
                default_text: String::new(),
                choices: Vec::new(),
                is_changed: g.has_changed(),
                error: String::new(),
            });
            for item in g.items {
                let default_shown = if item.default.is_empty() { "\"\"".to_string() } else { item.default.clone() };
                rows.push(AdvancedRow {
                    is_header: false,
                    title: item.get_friendly_title(Some(&language)),
                    kind: item.kind,
                    value: item.value.clone(),
                    default_text: Strings::format("AdvancedDefault", &[&default_shown]),
                    choices: item.choices.clone(),
                    is_changed: item.is_changed(),
                    error: errors.get(&item.key).cloned().unwrap_or_default(),
                    key: item.key,
                });
            }
        }
        drop(errors);
        *self.rows.borrow_mut() = rows;
        self.changed.notify();
    }

    /// Anahtar ve seçim hemen yazılır; metin kutusu Enter ya da odak kaybıyla (değer değiştiyse).
    pub fn apply(&self, key: &str, text: &str) {
        let unchanged = self.rows.borrow().iter().any(|r| !r.is_header && r.key == key && r.value == text);
        if unchanged {
            return;
        }
        let (error, updated) = AdvancedSettings::try_set(&self.vm.store().load(), key, Some(text));
        self.show(key, error);
        if error == AdvancedSettingError::None {
            self.vm.commit_loaded(updated);
        }
        self.rebuild(true);
    }

    pub fn reset(&self, key: &str) {
        let (error, updated) = AdvancedSettings::reset(&self.vm.store().load(), key);
        self.show(key, error);
        if error == AdvancedSettingError::None {
            self.vm.commit_loaded(updated);
        }
        self.rebuild(true);
    }

    fn show(&self, key: &str, error: AdvancedSettingError) {
        let mut errors = self.errors.borrow_mut();
        if error == AdvancedSettingError::None {
            errors.remove(key);
        } else {
            errors.insert(key.to_string(), Strings::get("AdvancedInvalid"));
        }
    }
}

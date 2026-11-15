//! Karşılığı: `Input/InputBindingValidator.cs`.

use std::collections::{HashMap, HashSet};

use super::input_binding::{InputBinding, InputTrigger};
use crate::shortcuts::{Hotkey, ShortcutAction, ShortcutBindings, ShortcutConflicts};
use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputIssueKind {
    /// Aynı eylem aynı tetikleyiciye iki kez bağlanmış; ikincisi atılır.
    Duplicate,

    /// Aynı tetikleyici başka bir eyleme zaten bağlı; sonraki atılır.
    Conflict,

    /// Windows'un hiçbir uygulamaya bırakmadığı kısayol (Win+L, Ctrl+Alt+Del).
    ReservedBySystem,

    /// RunCustomAction bağı var olmayan bir özel eyleme işaret ediyor.
    UnknownCustomAction,

    /// Kısayol, Koy'un yerleşik bir kısayoluyla (Ctrl+Alt+N gibi) aynı; yerleşik olan kalır.
    ConflictsWithBuiltIn,

    /// Bağ sayısı `InputBindingValidator::MAX_BINDINGS` sınırını aştı.
    TooMany,
}

/// - `index`: Sorunlu bağın giriş listesindeki sırası.
/// - `conflicts_with_index`: Çakıştığı bağın sırası (Duplicate ve Conflict için).
/// - `built_in`: Çakıştığı yerleşik kısayol (ConflictsWithBuiltIn için).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBindingIssue {
    pub kind: InputIssueKind,
    pub index: i32,
    pub binding: InputBinding,
    pub conflicts_with_index: Option<i32>,
    pub built_in: Option<ShortcutAction>,
}

impl InputBindingIssue {
    pub fn new(kind: InputIssueKind, index: i32, binding: InputBinding) -> Self {
        InputBindingIssue { kind, index, binding, conflicts_with_index: None, built_in: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBindingReport {
    pub accepted: Vec<InputBinding>,
    pub issues: Vec<InputBindingIssue>,
}

/// Çözülmüş bağları denetler. Kural: giriş sırasında ilk gelen kazanır. Aynı tetikleyici tekrar gelirse
/// aynı eylemse Duplicate, farklı eylemse Conflict olur. Bir çakışan bağ kabul listesine girmez.
/// Yerleşik kısayollar (`ShortcutBindings`) her zaman önce gelir: ShortcutManager de ilk sıradakini tutar.
pub struct InputBindingValidator;

impl InputBindingValidator {
    /// Kabul edilecek en çok bağ; fazlası TooMany olur (elle bozulmuş dosya listeyi şişirmesin).
    pub const MAX_BINDINGS: usize = 64;

    /// `built_in_shortcuts`: Yerleşik kısayolların metni (`ShortcutBindings::read_all`); `None` ise denetlenmez.
    pub fn validate(
        bindings: &[InputBinding],
        custom_action_ids: &HashSet<String>,
        built_in_shortcuts: Option<&HashMap<ShortcutAction, String>>,
    ) -> InputBindingReport {
        let mut accepted: Vec<InputBinding> = Vec::new();
        let mut issues: Vec<InputBindingIssue> = Vec::new();
        let mut owners: HashMap<String, usize> = HashMap::new();

        // CustomActionParser kimlikleri büyük-küçük harf duyarsız tekil tutar; burada da öyle eşleşmeli.
        let known_ids: Vec<&String> = custom_action_ids.iter().collect();

        for (i, binding) in bindings.iter().enumerate() {
            let index = i as i32;

            if accepted.len() >= Self::MAX_BINDINGS {
                issues.push(InputBindingIssue::new(InputIssueKind::TooMany, index, binding.clone()));
                continue;
            }

            if binding.action == super::InputAction::RunCustomAction
                && !binding.custom_action_id.as_deref().is_some_and(|id| known_ids.iter().any(|k| text::eq_ignore_case(k, id)))
            {
                issues.push(InputBindingIssue::new(InputIssueKind::UnknownCustomAction, index, binding.clone()));
                continue;
            }

            if let InputTrigger::Shortcut(shortcut) = &binding.trigger {
                if ShortcutConflicts::is_reserved_by_system(shortcut.hotkey) {
                    issues.push(InputBindingIssue::new(InputIssueKind::ReservedBySystem, index, binding.clone()));
                    continue;
                }
            }

            if let (InputTrigger::Shortcut(own), Some(built_in)) = (&binding.trigger, built_in_shortcuts) {
                if let Some(found) = Self::find_built_in(own.hotkey, built_in) {
                    let mut issue = InputBindingIssue::new(InputIssueKind::ConflictsWithBuiltIn, index, binding.clone());
                    issue.built_in = Some(found);
                    issues.push(issue);
                    continue;
                }
            }

            let key = Self::key_of(&binding.trigger);
            if let Some(&first) = owners.get(&key) {
                let previous = &bindings[first];
                let same_action = previous.action == binding.action
                    && match (&previous.custom_action_id, &binding.custom_action_id) {
                        (None, None) => true,
                        (Some(a), Some(b)) => text::eq_ignore_case(a, b),
                        _ => false,
                    };
                let mut issue = InputBindingIssue::new(
                    if same_action { InputIssueKind::Duplicate } else { InputIssueKind::Conflict },
                    index,
                    binding.clone(),
                );
                issue.conflicts_with_index = Some(first as i32);
                issues.push(issue);
                continue;
            }

            owners.insert(key, i);
            accepted.push(binding.clone());
        }

        InputBindingReport { accepted, issues }
    }

    /// Aynı kombinasyonu tutan yerleşik eylem (ShortcutConflicts.Check ile aynı karşılaştırma); yoksa `None`.
    /// C#'ta sözlük ekleme sırasıyla gezilir; Rust `HashMap` sırasız olduğundan eylemler tanım sırasıyla
    /// (`ShortcutBindings::all()`, C#'ta sözlüğü kuran sıra) gezilir.
    fn find_built_in(hotkey: Hotkey, built_in: &HashMap<ShortcutAction, String>) -> Option<ShortcutAction> {
        for &action in ShortcutBindings::all() {
            let Some(value) = built_in.get(&action) else {
                continue;
            };
            if let Some(other) = Hotkey::try_parse(value.as_str()) {
                if other == hotkey {
                    return Some(action);
                }
            }
        }

        None
    }

    /// Tetikleyici kimliği. Köşede süre ve geri itme mesafesi kimliğe girmez: aynı köşe iki kez bağlanamaz.
    pub(crate) fn key_of(trigger: &InputTrigger) -> String {
        match trigger {
            InputTrigger::Shortcut(s) => format!("S:{}:{}", s.hotkey.modifiers.0, s.hotkey.virtual_key),
            InputTrigger::HotCorner(h) => format!("C:{:?}", h.corner),
            InputTrigger::Mouse(m) => format!("M:{:?}:{:?}", m.surface, m.gesture),
        }
    }
}

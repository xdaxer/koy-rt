//! Windows yerleşik `ISpellCheckerFactory` COM arayüzüyle yazım denetimi yapar.
//! Panel kapalıyken bellekte COM nesnesi tutmaz ve boşta sıfır CPU kullanır.

use std::collections::HashSet;
use std::sync::Mutex;

use koy_core::search::{ISpellCheckerService, SpellCheckResult};
use koy_core::text;
use windows::Win32::Globalization::{
    CORRECTIVE_ACTION_GET_SUGGESTIONS, CORRECTIVE_ACTION_REPLACE, ISpellChecker, ISpellCheckerFactory, ISpellingError,
    SpellCheckerFactory,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::core::{HSTRING, PWSTR};

use super::com_apartment::ensure_mta;

const MAX_SUGGESTIONS: usize = 8;

struct State {
    factory: Option<ISpellCheckerFactory>,
    primary: Option<ISpellChecker>,
    fallback_en: Option<ISpellChecker>,
    initialized: bool,
}

// SAFETY: yazım denetleyicisi nesneleri serbest iş parçacıklıdır (MTA'da oluşturulur) ve yalnız kilit altında kullanılır.
unsafe impl Send for State {}

pub struct WindowsSpellCheckerService {
    gate: Mutex<State>,
}

impl Default for WindowsSpellCheckerService {
    fn default() -> Self {
        WindowsSpellCheckerService { gate: Mutex::new(State { factory: None, primary: None, fallback_en: None, initialized: false }) }
    }
}

/// `CultureInfo.CurrentUICulture.Name` (bölgesiz dil adına Koy'un kullandığı bölge eklenir).
fn current_ui_culture_name() -> String {
    match crate::locale::current_ui_two_letter().as_str() {
        "tr" => "tr-TR".to_string(),
        "en" => "en-US".to_string(),
        other => other.to_string(),
    }
}

unsafe fn take_pwstr(p: PWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: COM'un CoTaskMemAlloc ile ayırdığı dize; okunup serbest bırakılır.
    unsafe {
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

fn try_create_checker(factory: &ISpellCheckerFactory, language_tag: &str) -> Option<ISpellChecker> {
    if language_tag.trim().is_empty() {
        return None;
    }
    let tag = HSTRING::from(language_tag);
    // SAFETY: fabrika geçerli; dil etiketi sıfırla biten dize.
    unsafe {
        if factory.IsSupported(&tag).ok()?.as_bool() {
            return factory.CreateSpellChecker(&tag).ok();
        }
    }
    None
}

/// C# `ApplyReplacement`: UTF-16 konumlarıyla.
fn apply_replacement(full: &[u16], start: usize, length: usize, replacement: &str) -> String {
    if start == 0 && length == full.len() {
        return replacement.to_string();
    }
    if start + length > full.len() {
        return replacement.to_string();
    }
    let mut out: Vec<u16> = full[..start].to_vec();
    out.extend(replacement.encode_utf16());
    out.extend_from_slice(&full[start + length..]);
    String::from_utf16_lossy(&out)
}

/// `HashSet<string>(StringComparer.OrdinalIgnoreCase).Add`.
fn add_unique(seen: &mut HashSet<String>, suggestions: &mut Vec<String>, candidate: String) {
    if seen.insert(text::to_upper_invariant(&candidate)) {
        suggestions.push(candidate);
    }
}

fn collect_suggestions(checker: &ISpellChecker, word: &str, full: &[u16], start: usize, length: usize, suggestions: &mut Vec<String>, seen: &mut HashSet<String>) {
    // SAFETY: COM çağrıları; dönen dizeler serbest bırakılır.
    unsafe {
        let Ok(enum_string) = checker.Suggest(&HSTRING::from(word)) else { return };
        let mut buffer = [PWSTR::null(); MAX_SUGGESTIONS];
        while suggestions.len() < MAX_SUGGESTIONS {
            let mut fetched = 0u32;
            let hr = enum_string.Next(&mut buffer, Some(&mut fetched));
            if fetched == 0 {
                break;
            }
            for slot in buffer.iter_mut().take(fetched as usize) {
                let candidate = take_pwstr(std::mem::replace(slot, PWSTR::null())).unwrap_or_default();
                if suggestions.len() < MAX_SUGGESTIONS {
                    add_unique(seen, suggestions, apply_replacement(full, start, length, &candidate));
                }
            }
            if hr.0 != 0 {
                break;
            }
        }
    }
}

fn check_with(checker: &ISpellChecker, input: &str) -> SpellCheckResult {
    let full: Vec<u16> = input.encode_utf16().collect();
    // SAFETY: COM çağrıları; arayüzler Drop ile bırakılır.
    unsafe {
        let Ok(errors) = checker.Check(&HSTRING::from(input)) else { return SpellCheckResult::new(true, true, Vec::new()) };
        let mut suggestions = Vec::new();
        let mut seen = HashSet::new();
        let mut has_error = false;
        loop {
            let mut error: Option<ISpellingError> = None;
            if errors.Next(&mut error).0 != 0 {
                break;
            }
            let Some(error) = error else { break };
            has_error = true;
            let start = error.StartIndex().unwrap_or(0) as usize;
            let length = error.Length().unwrap_or(0) as usize;
            let action = error.CorrectiveAction().unwrap_or_default();
            let replacement = error.Replacement().ok().and_then(|p| take_pwstr(p));

            if action == CORRECTIVE_ACTION_REPLACE && !text::is_null_or_white_space(replacement.as_deref()) {
                add_unique(&mut seen, &mut suggestions, apply_replacement(&full, start, length, &replacement.unwrap_or_default()));
            } else if action == CORRECTIVE_ACTION_GET_SUGGESTIONS && start + length <= full.len() {
                let word = String::from_utf16_lossy(&full[start..start + length]);
                collect_suggestions(checker, &word, &full, start, length, &mut suggestions, &mut seen);
            }
            if suggestions.len() >= MAX_SUGGESTIONS {
                break;
            }
        }
        SpellCheckResult::new(true, !has_error, suggestions)
    }
}

impl WindowsSpellCheckerService {
    fn ensure_initialized(st: &mut State) {
        if st.initialized {
            return;
        }
        st.initialized = true;
        ensure_mta();
        // SAFETY: COM bu iş parçacığında başlatıldı.
        let Ok(factory) = (unsafe { CoCreateInstance::<_, ISpellCheckerFactory>(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER) }) else {
            return;
        };
        let ui_tag = current_ui_culture_name();
        st.primary = try_create_checker(&factory, &ui_tag).or_else(|| try_create_checker(&factory, "en-US"));
        if st.primary.is_some() && !text::eq_ignore_case(&ui_tag, "en-US") {
            st.fallback_en = try_create_checker(&factory, "en-US");
        }
        st.factory = Some(factory);
    }
}

impl ISpellCheckerService for WindowsSpellCheckerService {
    fn check(&self, input: &str) -> SpellCheckResult {
        if input.trim().is_empty() {
            return SpellCheckResult::new(true, true, Vec::new());
        }
        let mut st = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        Self::ensure_initialized(&mut st);
        let Some(primary) = st.primary.as_ref() else { return SpellCheckResult::new(false, true, Vec::new()) };

        let primary_result = check_with(primary, input);
        if primary_result.is_correct {
            return primary_result;
        }
        if let Some(fallback) = st.fallback_en.as_ref() {
            let en_result = check_with(fallback, input);
            if en_result.is_correct {
                return en_result;
            }
            if primary_result.suggestions.is_empty() && !en_result.suggestions.is_empty() {
                return en_result;
            }
        }
        primary_result
    }

    fn release(&self) {
        let mut st = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        st.primary = None;
        st.fallback_en = None;
        st.factory = None;
        st.initialized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_uses_utf16_positions() {
        let full: Vec<u16> = "çok güzle gün".encode_utf16().collect();
        assert_eq!(apply_replacement(&full, 4, 5, "güzel"), "çok güzel gün");
        assert_eq!(apply_replacement(&full, 0, full.len(), "x"), "x");
    }

    #[test]
    fn english_checker_suggests_corrections() {
        crate::locale::set_current_ui_culture("en-US");
        let service = WindowsSpellCheckerService::default();
        let ok = service.check("hello");
        assert!(ok.is_supported && ok.is_correct);
        let wrong = service.check("helllo");
        assert!(!wrong.is_correct);
        assert!(!wrong.suggestions.is_empty());
        service.release();
    }
}

//! Arama paneli durumunu Slint yapısına çevirir; kutudaki tuşları görünüm modeline iletir (C# `Box_PreviewKeyDown`).

use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, VecModel};

use koy_core::search::{SearchGlyph, SearchHistoryMode};

use super::search_factory::glyph_path;
use super::search_view_model::SearchViewModel;
use crate::resources::strings::Strings;
use crate::{SearchActionData, SearchData, SearchRowData, SearchText};

/// Genel simgeler (çizilmiş yolu olmayan glifler) `Icons` global'inden okunur.
pub struct GlyphFallback {
    pub power: slint::SharedString,
    pub gauge: slint::SharedString,
    pub layers: slint::SharedString,
    pub app_window: slint::SharedString,
}

fn glyph(g: SearchGlyph, f: &GlyphFallback) -> slint::SharedString {
    if let Some(p) = glyph_path(g) {
        return p.into();
    }
    match g {
        SearchGlyph::Power => f.power.clone(),
        SearchGlyph::Process => f.gauge.clone(),
        SearchGlyph::Window => f.layers.clone(),
        _ => f.app_window.clone(),
    }
}

pub fn to_data(vm: &SearchViewModel, fallback: &GlyphFallback, scroll_to: i32, caret: (i32, i32)) -> SearchData {
    let s = vm.state();
    let rows: Vec<SearchRowData> = s
        .rows
        .iter()
        .map(|r| SearchRowData {
            index: r.index,
            header: r.header.clone().unwrap_or_default().into(),
            title: r.title().into(),
            subtitle: r.subtitle().unwrap_or_default().into(),
            glyph: glyph(r.glyph(), fallback),
            has_icon: r.icon.is_some(),
            icon: r.icon.clone().unwrap_or_default(),
            selected: r.is_selected,
            actions: ModelRc::new(VecModel::from(
                r.actions.iter().map(|a| SearchActionData { label: a.label.clone().into(), active: a.is_active }).collect::<Vec<_>>(),
            )),
        })
        .collect();
    SearchData {
        query: s.query_text.clone().into(),
        suggestion: s.suggestion_text.clone().into(),
        show_placeholder: s.show_placeholder,
        show_empty_hint: s.show_empty_hint,
        is_searching: s.is_searching,
        has_history: s.has_history,
        is_pinned: s.is_pinned,
        rows: ModelRc::new(VecModel::from(rows)),
        scroll_to,
        caret_request: caret.0,
        caret_serial: caret.1,
    }
}

fn is(text: &str, key: Key) -> bool {
    let k: slint::SharedString = key.into();
    text == k.as_str()
}

/// Kutudaki tuş. İşlendiyse true (tuş kutuya gitmez).
pub fn handle_key(vm: &SearchViewModel, text: &str, ctrl: bool, shift: bool, at_start: bool, at_end: bool) -> bool {
    let lower = text.to_lowercase();
    if is(text, Key::Escape) {
        vm.request_close();
    } else if is(text, Key::Return) {
        vm.activate(shift);
    } else if (is(text, Key::UpArrow) && ctrl) || is(text, Key::PageUp) {
        vm.move_category(-1);
    } else if (is(text, Key::DownArrow) && ctrl) || is(text, Key::PageDown) {
        vm.move_category(1);
    } else if is(text, Key::UpArrow) || (ctrl && lower == "k") {
        vm.move_selection(-1);
    } else if is(text, Key::DownArrow) || (ctrl && lower == "j") {
        vm.show_history_or_move_down();
    } else if is(text, Key::Home) && at_start {
        vm.select_first();
    } else if is(text, Key::End) && at_end {
        vm.select_last();
    } else if is(text, Key::RightArrow) && at_end && vm.history_mode() == SearchHistoryMode::Suggestion && vm.accept_suggestion() {
    } else if is(text, Key::Tab) || is(text, Key::Backtab) {
        let shift = shift || is(text, Key::Backtab);
        if !(!shift && vm.history_mode() == SearchHistoryMode::Suggestion && vm.accept_suggestion()) {
            let direction = if shift { -1 } else { 1 };
            if !vm.cycle_action(direction) {
                vm.move_selection(direction);
                if shift {
                    vm.focus_last_action();
                }
            }
        }
    } else {
        return false;
    }
    true
}

/// Hemen tamamlama kipinde yazılanın tamamlanmış hâli; tamamlama yoksa boş.
pub fn completion(vm: &SearchViewModel, typed: &str) -> String {
    if vm.history_mode() != SearchHistoryMode::ImmediateCompletion || typed.is_empty() {
        return String::new();
    }
    let suggestion = vm.suggestion();
    if suggestion.chars().count() <= typed.chars().count() {
        return String::new();
    }
    let tail: String = suggestion.chars().skip(typed.chars().count()).collect();
    format!("{typed}{tail}")
}

pub fn install_text<C: ComponentHandle>(c: &C, help_glyph: &str)
where
    for<'a> SearchText<'a>: slint::Global<'a, C>,
{
    let t = c.global::<SearchText>();
    t.set_placeholder(Strings::get("SearchPlaceholder").into());
    t.set_empty_hint(Strings::get("SearchEmptyHint").into());
    t.set_searching(Strings::get("SearchSearching").into());
    t.set_help_glyph(help_glyph.into());
}

//! Çentikteki arama paneli. Durum ve kararlar `SearchPanelController`'da; bu tür satırları ve simgeleri hazırlar.
//! Karşılığı: `Features/Search/SearchViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use koy_core::event::Subscription;
use koy_core::search::{SearchActivation, SearchActivationResult, SearchGlyph, SearchHistoryMode, SearchPanelController, SearchResult};

use crate::infra::dispatch;
use crate::infra::image_decode;
use crate::infra::observable::{LocalEvent, Notifier};
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;

const ICON_DECODE_SIZE: u32 = 48;

#[derive(Clone)]
pub struct SearchActionRow {
    pub label: String,
    pub index: i32,
    pub is_active: bool,
}

#[derive(Clone)]
pub struct SearchRow {
    pub result: SearchResult,
    pub index: i32,
    pub header: Option<String>,
    pub actions: Vec<SearchActionRow>,
    pub is_selected: bool,
    pub icon: Option<slint::Image>,
    pub is_confirming: bool,
}

impl SearchRow {
    pub fn title(&self) -> &str {
        &self.result.title
    }
    pub fn glyph(&self) -> SearchGlyph {
        self.result.glyph
    }
    pub fn subtitle(&self) -> Option<String> {
        if self.is_confirming {
            Some(match &self.result.confirmation_detail {
                Some(d) => Strings::format("SearchConfirmRun", &[d]),
                None => Strings::get("SearchConfirmHint"),
            })
        } else {
            self.result.subtitle.clone()
        }
    }
}

#[derive(Clone, Default)]
pub struct SearchState {
    pub rows: Vec<SearchRow>,
    pub query_text: String,
    pub suggestion_text: String,
    pub is_pinned: bool,
    pub has_history: bool,
    /// Kutu boş: gri "Ara…" yazısı.
    pub show_placeholder: bool,
    pub show_empty_hint: bool,
    pub is_searching: bool,
    pub is_open: bool,
}

pub struct SearchViewModel {
    panel: SearchPanelController,
    save_pinned: Box<dyn Fn(bool)>,
    icons: RefCell<HashMap<String, Option<slint::Image>>>,
    /// Gösterilen sonuçların kimlikleri (C#'taki ReferenceEquals denetimi: liste değişmediyse satırlar korunur).
    shown_ids: RefCell<Option<Vec<String>>>,
    state: RefCell<SearchState>,
    subs: RefCell<Vec<Subscription>>,
    disposed: Cell<bool>,
    pub changed: Notifier,
    /// Panel kapanmalı; değer odağın önceki pencereye dönüp dönmeyeceği.
    pub close_requested: LocalEvent<bool>,
    /// İmleç şu konuma gitmeli.
    pub caret_requested: LocalEvent<i32>,
    /// Seçili satır değişti; görünür yapılmalı.
    pub selection_changed: LocalEvent<i32>,
    /// Ayarlar düğmesi (App verir: Koy penceresi Çentik sayfasında açılır).
    pub open_settings_handler: RefCell<Option<Box<dyn Fn()>>>,
}

impl SearchViewModel {
    pub fn new(panel: SearchPanelController, pinned: bool, save_pinned: Box<dyn Fn(bool)>) -> Rc<Self> {
        let vm = Rc::new(SearchViewModel {
            panel,
            save_pinned,
            icons: RefCell::new(HashMap::new()),
            shown_ids: RefCell::new(None),
            state: RefCell::new(SearchState { is_pinned: pinned, show_placeholder: true, show_empty_hint: true, ..Default::default() }),
            subs: RefCell::new(Vec::new()),
            disposed: Cell::new(false),
            changed: Notifier::default(),
            close_requested: LocalEvent::default(),
            caret_requested: LocalEvent::default(),
            selection_changed: LocalEvent::default(),
            open_settings_handler: RefCell::new(None),
        });
        // Denetleyici olayları kaynak iş parçacıklarından `post` ile arayüze taşınmış olarak gelir; yine de
        // ödünç çakışması olmasın diye bir sonraki tura bırakılır.
        let me = UiRef::new(&vm);
        let s1 = vm.panel.changed().subscribe(move |_| {
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    vm.refresh();
                }
            });
        });
        let s2 = vm.panel.query_replaced().subscribe(move |caret: &i32| {
            let caret = *caret;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    vm.sync_text();
                    vm.changed.notify();
                    vm.caret_requested.invoke(&caret);
                }
            });
        });
        let s3 = vm.panel.auto_activated().subscribe(move |r: &SearchResult| {
            let restores = r.restores_focus;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && !vm.state.borrow().is_pinned
                {
                    vm.close_requested.invoke(&restores);
                }
            });
        });
        vm.subs.borrow_mut().extend([s1, s2, s3]);
        vm
    }

    pub fn state(&self) -> std::cell::Ref<'_, SearchState> {
        self.state.borrow()
    }

    pub fn is_open(&self) -> bool {
        self.panel.is_open()
    }

    pub fn suggestion(&self) -> String {
        self.panel.suggestion()
    }

    pub fn history_mode(&self) -> SearchHistoryMode {
        self.panel.history_mode()
    }

    pub fn open(self: &Rc<Self>, initial_query: Option<&str>) {
        self.panel.open();
        if let Some(q) = initial_query {
            self.panel.replace_query(q, true);
        }
        self.sync_text();
        self.refresh();
    }

    pub fn close(&self) {
        self.panel.close();
        self.icons.borrow_mut().clear();
        self.state.borrow_mut().is_open = false;
    }

    /// Kutuya yazıldı (görünümden).
    pub fn set_query_text(&self, value: &str) {
        if self.state.borrow().query_text == value {
            return;
        }
        self.state.borrow_mut().query_text = value.to_string();
        self.panel.set_query(Some(value));
    }

    pub fn set_pinned(&self, pinned: bool) {
        if self.state.borrow().is_pinned == pinned {
            return;
        }
        self.state.borrow_mut().is_pinned = pinned;
        (self.save_pinned)(pinned);
        self.changed.notify();
    }

    pub fn move_selection(&self, delta: i32) {
        self.panel.move_selection(delta);
    }

    pub fn move_category(&self, direction: i32) {
        self.panel.move_category(direction);
    }

    pub fn select_first(&self) {
        self.panel.select(0);
    }

    pub fn select_last(&self) {
        self.panel.select(self.panel.results().len() as i32 - 1);
    }

    pub fn cycle_action(&self, direction: i32) -> bool {
        self.panel.cycle_action(direction)
    }

    pub fn focus_last_action(&self) {
        self.panel.focus_last_action();
    }

    pub fn show_history_or_move_down(&self) {
        if self.state.borrow().query_text.is_empty() && !self.panel.showing_history() && self.panel.has_history() {
            self.panel.toggle_history();
        } else {
            self.panel.move_selection(1);
        }
    }

    /// Gri öneriyi kabul eder.
    pub fn accept_suggestion(&self) -> bool {
        let suggestion = self.panel.suggestion();
        if suggestion.chars().count() <= self.state.borrow().query_text.chars().count() {
            return false;
        }
        self.panel.replace_query(&suggestion, false);
        true
    }

    /// Enter. Boş kutuda paneli kapatır.
    pub fn activate(&self, first_action: bool) {
        if self.state.borrow().query_text.is_empty() && !self.panel.showing_history() {
            self.close_requested.invoke(&true);
            return;
        }
        let r = self.panel.activate(first_action);
        self.handle(r);
    }

    pub fn activate_row(&self, index: i32) {
        self.panel.select(index);
        let r = self.panel.activate(false);
        self.handle(r);
    }

    pub fn run_row_action(&self, row: i32, action: i32) {
        self.panel.select(row);
        while self.panel.active_action() < action && self.panel.cycle_action(1) {}
        let r = self.panel.activate(false);
        self.handle(r);
    }

    pub fn request_close(&self) {
        self.close_requested.invoke(&true);
    }

    pub fn toggle_history(&self) {
        self.panel.toggle_history();
    }

    pub fn toggle_help(&self) {
        let q = if self.state.borrow().query_text.starts_with('?') { "" } else { "?" };
        self.panel.replace_query(q, false);
    }

    pub fn toggle_pin(&self) {
        let p = !self.state.borrow().is_pinned;
        self.set_pinned(p);
    }

    pub fn open_settings(&self) {
        self.close_requested.invoke(&false);
        if let Some(f) = self.open_settings_handler.borrow().as_ref() {
            f();
        }
    }

    fn handle(&self, activation: SearchActivationResult) {
        if activation.outcome == SearchActivation::Executed
            && let Some(result) = activation.result
            && !self.state.borrow().is_pinned
        {
            self.close_requested.invoke(&result.restores_focus);
        }
    }

    fn sync_text(&self) {
        self.state.borrow_mut().query_text = self.panel.query();
    }

    fn refresh(self: &Rc<Self>) {
        let results = self.panel.results();
        let ids: Vec<String> = results.iter().map(|r| r.id.clone()).collect();
        if self.shown_ids.borrow().as_ref() != Some(&ids) {
            self.rebuild_rows(results);
            *self.shown_ids.borrow_mut() = Some(ids);
        }
        let selected = self.panel.selected_index();
        let pending = self.panel.pending_confirmation_id();
        let active_action = self.panel.active_action();
        let suggestion = self.panel.suggestion();
        let busy = self.panel.is_busy();
        {
            let mut st = self.state.borrow_mut();
            for (i, row) in st.rows.iter_mut().enumerate() {
                row.is_selected = i as i32 == selected;
                row.is_confirming = pending.as_deref() == Some(row.result.id.as_str());
                let sel = row.is_selected;
                for a in row.actions.iter_mut() {
                    a.is_active = sel && a.index == active_action;
                }
            }
            let q_len = st.query_text.chars().count();
            st.suggestion_text = if suggestion.chars().count() > q_len {
                format!("{}{}", st.query_text, suggestion.chars().skip(q_len).collect::<String>())
            } else {
                String::new()
            };
            st.has_history = self.panel.has_history();
            st.show_placeholder = st.query_text.is_empty();
            st.show_empty_hint = st.rows.is_empty() && st.query_text.is_empty();
            st.is_searching = st.rows.is_empty() && !st.query_text.is_empty() && busy;
            st.is_open = self.panel.is_open();
        }
        self.changed.notify();
        if selected >= 0 {
            self.selection_changed.invoke(&selected);
        }
    }

    fn rebuild_rows(self: &Rc<Self>, results: Vec<SearchResult>) {
        let mut category: Option<String> = None;
        let mut rows = Vec::with_capacity(results.len());
        for (i, result) in results.into_iter().enumerate() {
            let header = if category.as_deref() != Some(result.category.as_str()) { Some(result.category.clone()) } else { None };
            category = Some(result.category.clone());
            let actions = result.actions.iter().enumerate().map(|(ai, a)| SearchActionRow { label: a.label.clone(), index: ai as i32, is_active: false }).collect();
            let icon = self.icons.borrow().get(&result.id).cloned().flatten();
            rows.push(SearchRow { result, index: i as i32, header, actions, is_selected: false, icon, is_confirming: false });
        }
        let to_load: Vec<(String, std::sync::Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>)> = rows
            .iter()
            .filter(|r| !self.icons.borrow().contains_key(&r.result.id))
            .filter_map(|r| r.result.load_icon.clone().map(|l| (r.result.id.clone(), l)))
            .collect();
        self.state.borrow_mut().rows = rows;
        for (id, load) in to_load {
            self.load_icon(id, load);
        }
    }

    fn load_icon(self: &Rc<Self>, id: String, load: std::sync::Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>) {
        let me = UiRef::new(self);
        std::thread::spawn(move || {
            let bgra = load().and_then(|b| image_decode::decode(&b, Some(ICON_DECODE_SIZE)));
            dispatch::post(move || {
                let Some(vm) = me.upgrade() else { return };
                if vm.disposed.get() || !vm.is_open() {
                    return;
                }
                let image = bgra.map(|b| b.to_image());
                vm.icons.borrow_mut().insert(id.clone(), image.clone());
                for row in vm.state.borrow_mut().rows.iter_mut().filter(|r| r.result.id == id) {
                    row.icon = image.clone();
                }
                vm.changed.notify();
            });
        });
    }

    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        let subs: Vec<Subscription> = self.subs.borrow_mut().drain(..).collect();
        if let [a, b, c] = subs[..] {
            self.panel.changed().unsubscribe(a);
            self.panel.query_replaced().unsubscribe(b);
            self.panel.auto_activated().unsubscribe(c);
        }
        self.panel.dispose();
    }
}

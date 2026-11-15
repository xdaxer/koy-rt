use super::search_util::{panic_message, utf16_len};
use super::{
    ISearchSource, SearchAction, SearchGlyph, SearchHistory, SearchHistoryMode, SearchLayout, SearchQuery, SearchResult,
};
use crate::diagnostics::{CrashError, CrashLog};
use crate::event::{Event, Subscription};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, Weak};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SearchActivation {
    #[default]
    None,

    /// Çalıştırıldı; panel kapanmalı.
    Executed,

    /// Onay bekliyor; panel açık kalır.
    NeedsConfirmation,

    /// Sorgu değişti; panel açık kalır.
    QueryUpdated,

    /// Sonuçlar henüz gelmedi; ilk sonuç gelince çalıştırılacak.
    Deferred,
}

#[derive(Debug, Clone)]
pub struct SearchActivationResult {
    pub outcome: SearchActivation,
    pub result: Option<SearchResult>,
}

impl SearchActivationResult {
    pub fn new(outcome: SearchActivation, result: Option<SearchResult>) -> Self {
        SearchActivationResult { outcome, result }
    }
}

/// `Action<Action>`: kaynak olaylarını UI iş parçacığına taşır.
pub type PostFn = Arc<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>;

/// `Func<SearchQuery, SearchResult?>`: hiç sonuç yoksa gösterilecek satır.
pub type FallbackFn = Arc<dyn Fn(&SearchQuery) -> Option<SearchResult> + Send + Sync>;

struct State {
    query: SearchQuery,
    untrimmed: String,
    prior: String,
    activate_when_ready: bool,
    pending_action: i32,
    disposed: bool,
    retain_prior_search: bool,
    history_mode: SearchHistoryMode,
    is_open: bool,
    results: Vec<SearchResult>,
    selected_index: i32,
    active_action: i32,
    pending_confirmation_id: Option<String>,
    query_from_clipboard: bool,
    showing_history: bool,
}

struct Inner {
    sources: Vec<Arc<dyn ISearchSource>>,
    post: PostFn,
    history: Option<Arc<SearchHistory>>,
    fallback: Option<FallbackFn>,
    history_category: String,
    remove_label: String,
    state: Mutex<State>,
    subscriptions: Mutex<Vec<Subscription>>,
    changed: Event<()>,
    query_replaced: Event<i32>,
    auto_activated: Event<SearchResult>,
}

/// Arama panelinin durumu: sorgu, sonuçlar, seçili satır ve eylem, onay, geçmiş.
///
/// İş parçacığı notu: durum bir kilitle korunur; olaylar ve kaynak/eylem geri çağrıları kilit dışında çalışır
/// (işleyiciler denetleyiciyi yeniden çağırabilir). Kaynak olayları `post` ile taşınır.
pub struct SearchPanelController {
    inner: Arc<Inner>,
}

enum Step {
    Done(SearchActivationResult, bool),
    RunAction(SearchAction, SearchResult),
    RunResult(SearchResult),
}

impl SearchPanelController {
    /// - `sources`: sonuç kaynakları.
    /// - `post`: kaynak olaylarını UI iş parçacığına taşır; verilmezse olduğu yerde çalışır.
    /// - `history`: çalıştırılan aramalar; yoksa geçmiş kapalıdır.
    /// - `fallback`: hiç sonuç yoksa gösterilecek satır (varsayılan web araması).
    ///
    /// C# varsayılanları: `historyCategory = ""`, `removeLabel = ""`.
    pub fn new(
        sources: Vec<Arc<dyn ISearchSource>>,
        post: Option<PostFn>,
        history: Option<Arc<SearchHistory>>,
        fallback: Option<FallbackFn>,
        history_category: &str,
        remove_label: &str,
    ) -> Self {
        let post: PostFn = post.unwrap_or_else(|| Arc::new(|action: Box<dyn FnOnce() + Send>| action()));
        SearchPanelController {
            inner: Arc::new(Inner {
                sources,
                post,
                history,
                fallback,
                history_category: history_category.to_string(),
                remove_label: remove_label.to_string(),
                state: Mutex::new(State {
                    query: SearchQuery::new(Some(""), false),
                    untrimmed: String::new(),
                    prior: String::new(),
                    activate_when_ready: false,
                    pending_action: -1,
                    disposed: false,
                    retain_prior_search: true,
                    history_mode: SearchHistoryMode::Suggestion,
                    is_open: false,
                    results: Vec::new(),
                    selected_index: -1,
                    active_action: -1,
                    pending_confirmation_id: None,
                    query_from_clipboard: false,
                    showing_history: false,
                }),
                subscriptions: Mutex::new(Vec::new()),
                changed: Event::new(),
                query_replaced: Event::new(),
                auto_activated: Event::new(),
            }),
        }
    }

    pub fn changed(&self) -> &Event<()> {
        &self.inner.changed
    }

    /// Sorgu program tarafından değişti; yazı kutusu güncellenmeli (değer: yeni metnin uzunluğu).
    pub fn query_replaced(&self) -> &Event<i32> {
        &self.inner.query_replaced
    }

    /// Bekletilen Enter, sonuç gelince çalıştı; panel kapanmalı.
    pub fn auto_activated(&self) -> &Event<SearchResult> {
        &self.inner.auto_activated
    }

    pub fn retain_prior_search(&self) -> bool {
        self.inner.state.lock().unwrap().retain_prior_search
    }

    pub fn set_retain_prior_search(&self, value: bool) {
        self.inner.state.lock().unwrap().retain_prior_search = value;
    }

    pub fn history_mode(&self) -> SearchHistoryMode {
        self.inner.state.lock().unwrap().history_mode
    }

    pub fn set_history_mode(&self, value: SearchHistoryMode) {
        self.inner.state.lock().unwrap().history_mode = value;
    }

    pub fn is_open(&self) -> bool {
        self.inner.state.lock().unwrap().is_open
    }

    /// Yazı kutusundaki metin (boşluklar dahil).
    pub fn query(&self) -> String {
        self.inner.state.lock().unwrap().untrimmed.clone()
    }

    pub fn results(&self) -> Vec<SearchResult> {
        self.inner.state.lock().unwrap().results.clone()
    }

    pub fn selected_index(&self) -> i32 {
        self.inner.state.lock().unwrap().selected_index
    }

    /// Seçili satırın vurgulu eylemi; -1 satırın kendisi.
    pub fn active_action(&self) -> i32 {
        self.inner.state.lock().unwrap().active_action
    }

    pub fn pending_confirmation_id(&self) -> Option<String> {
        self.inner.state.lock().unwrap().pending_confirmation_id.clone()
    }

    /// Metin panodan geldi; komut olarak çalıştırmak onay ister.
    pub fn query_from_clipboard(&self) -> bool {
        self.inner.state.lock().unwrap().query_from_clipboard
    }

    pub fn showing_history(&self) -> bool {
        self.inner.state.lock().unwrap().showing_history
    }

    pub fn is_busy(&self) -> bool {
        self.inner.is_busy()
    }

    pub fn has_history(&self) -> bool {
        let mode = self.inner.state.lock().unwrap().history_mode;
        self.inner.has_history(mode)
    }

    pub fn selected(&self) -> Option<SearchResult> {
        let st = self.inner.state.lock().unwrap();
        Inner::selected(&st).cloned()
    }

    /// Yazılanı tamamlayan geçmiş kaydı; yoksa boş.
    pub fn suggestion(&self) -> String {
        let (mode, untrimmed) = {
            let st = self.inner.state.lock().unwrap();
            (st.history_mode, st.untrimmed.clone())
        };
        if mode == SearchHistoryMode::Disabled || untrimmed.is_empty() {
            return String::new();
        }
        self.inner.history.as_ref().and_then(|h| h.suggest(&untrimmed)).unwrap_or_default()
    }

    pub fn open(&self) {
        let inner = &self.inner;
        {
            let mut st = inner.state.lock().unwrap();
            if st.is_open || st.disposed {
                return;
            }

            st.is_open = true;
            Inner::reset_selection(&mut st);
            st.results = Vec::new();
            st.showing_history = false;
        }

        for source in &inner.sources {
            let weak: Weak<Inner> = Arc::downgrade(inner);
            let sub = source.changed().subscribe(move |_| {
                if let Some(strong) = weak.upgrade() {
                    let post = Arc::clone(&strong.post);
                    post(Box::new(move || strong.on_source_changed()));
                }
            });
            inner.subscriptions.lock().unwrap().push(sub);
            let source = Arc::clone(source);
            CrashLog::guard("Arama kaynağı hazırlığı", move || source.prepare());
        }

        let nonempty = {
            let mut st = inner.state.lock().unwrap();
            let prior = if st.retain_prior_search { st.prior.clone() } else { String::new() };
            Inner::set_text(&mut st, &prior);
            !st.untrimmed.is_empty()
        };
        if nonempty {
            inner.refresh();
        }
        inner.changed.invoke(&());
    }

    pub fn close(&self) {
        let inner = &self.inner;
        {
            let mut st = inner.state.lock().unwrap();
            if !st.is_open {
                return;
            }

            st.is_open = false;
            // Pano metni bir sonraki açılışa taşınmaz.
            st.prior = if st.query_from_clipboard { String::new() } else { st.untrimmed.clone() };
            st.query_from_clipboard = false;
            Inner::set_text(&mut st, "");
            st.results = Vec::new();
            st.showing_history = false;
            Inner::reset_selection(&mut st);
        }

        let subs: Vec<Subscription> = std::mem::take(&mut *inner.subscriptions.lock().unwrap());
        for (source, sub) in inner.sources.iter().zip(subs) {
            source.changed().unsubscribe(sub);
            let source = Arc::clone(source);
            CrashLog::guard("Arama kaynağı bırakma", move || source.release());
        }

        inner.changed.invoke(&());
    }

    pub fn set_query(&self, text: Option<&str>) {
        self.inner.set_query(text);
    }

    /// Boş kutuda aşağı ok ya da düğme: son aramaları gösterir / gizler.
    pub fn toggle_history(&self) {
        let inner = &self.inner;
        let mode = {
            let st = inner.state.lock().unwrap();
            if !st.is_open || st.query.raw.chars().count() > 0 {
                return;
            }
            st.history_mode
        };
        if !inner.has_history(mode) {
            return;
        }

        let showing = {
            let st = inner.state.lock().unwrap();
            !st.showing_history
        };
        let rows = if showing { inner.build_history_rows() } else { Vec::new() };
        {
            let mut st = inner.state.lock().unwrap();
            st.showing_history = showing;
            st.results = rows;
            st.selected_index = if st.results.is_empty() { -1 } else { 0 };
            st.active_action = -1;
        }
        inner.changed.invoke(&());
    }

    pub fn move_selection(&self, delta: i32) {
        {
            let mut st = self.inner.state.lock().unwrap();
            if st.results.is_empty() {
                return;
            }

            let n = st.results.len() as i32;
            st.selected_index = ((st.selected_index + delta) % n + n) % n;
            st.active_action = -1;
            Inner::clear_pending(&mut st);
        }
        self.inner.changed.invoke(&());
    }

    /// Bir sonraki ya da önceki kategorinin ilk satırına atlar, sona gelince başa döner.
    pub fn move_category(&self, direction: i32) {
        let target = {
            let st = self.inner.state.lock().unwrap();
            if st.results.is_empty() {
                return;
            }

            let current = st.selected_index.max(0) as usize;
            let category = st.results[current].category.clone();
            let starts: Vec<usize> =
                (0..st.results.len()).filter(|&i| i == 0 || st.results[i].category != st.results[i - 1].category).collect();
            let position = starts.iter().rposition(|&i| i <= current).map_or(-1, |p| p as i32);

            // Yukarı giderken önce bulunulan kategorinin başına değil, bir önceki kategoriye atlanır.
            let n = starts.len() as i32;
            let next = ((position + direction.signum()) % n + n) % n;
            if st.results[starts[next as usize]].category != category || starts.len() == 1 {
                Some(starts[next as usize] as i32)
            } else {
                None
            }
        };

        if let Some(index) = target {
            self.select(index);
        }
    }

    pub fn select(&self, index: i32) {
        {
            let mut st = self.inner.state.lock().unwrap();
            if index < 0 || index as usize >= st.results.len() || index == st.selected_index {
                return;
            }

            st.selected_index = index;
            st.active_action = -1;
            Inner::clear_pending(&mut st);
        }
        self.inner.changed.invoke(&());
    }

    /// Tab: satırın eylemleri arasında gezer. Eylemlerin dışına çıkıyorsa false döner, satır değişmelidir.
    pub fn cycle_action(&self, direction: i32) -> bool {
        {
            let mut st = self.inner.state.lock().unwrap();
            let Some(result) = Inner::selected(&st) else { return false };

            let next = st.active_action + direction.signum();
            if next < -1 || next >= result.actions.len() as i32 {
                return false;
            }

            st.active_action = next;
            Inner::clear_pending(&mut st);
        }
        self.inner.changed.invoke(&());
        true
    }

    /// Üst tuşuyla satıra dönerken son eylemin vurgulanması için.
    pub fn focus_last_action(&self) {
        {
            let mut st = self.inner.state.lock().unwrap();
            let count = match Inner::selected(&st) {
                Some(r) if !r.actions.is_empty() => r.actions.len(),
                _ => return,
            };
            st.active_action = count as i32 - 1;
        }
        self.inner.changed.invoke(&());
    }

    /// Seçili satırı ya da vurgulu eylemini çalıştırır. Sonuçlar henüz gelmediyse ilk sonuca ertelenir.
    /// `first_action`: Üst+Enter: satırın ilk eylemi (C# varsayılanı `false`).
    pub fn activate(&self, first_action: bool) -> SearchActivationResult {
        self.inner.activate(first_action)
    }

    /// Sorguyu program tarafından değiştirir ve yazı kutusuna bildirir.
    /// `from_clipboard`: Metin panodan geldi; komut olarak çalıştırmak onay ister (C# varsayılanı `false`).
    pub fn replace_query(&self, text: &str, from_clipboard: bool) {
        self.inner.replace_query(text, from_clipboard);
    }

    pub fn clear_history(&self) {
        if let Some(h) = &self.inner.history {
            h.clear();
        }
        {
            let mut st = self.inner.state.lock().unwrap();
            if !st.showing_history {
                return;
            }

            st.showing_history = false;
            st.results = Vec::new();
            Inner::reset_selection(&mut st);
        }
        self.inner.changed.invoke(&());
    }

    pub fn dispose(&self) {
        if self.inner.state.lock().unwrap().disposed {
            return;
        }

        self.close();
        self.inner.state.lock().unwrap().disposed = true;
    }
}

impl Drop for SearchPanelController {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl Inner {
    fn selected(st: &State) -> Option<&SearchResult> {
        if st.selected_index >= 0 && (st.selected_index as usize) < st.results.len() {
            Some(&st.results[st.selected_index as usize])
        } else {
            None
        }
    }

    fn is_busy(&self) -> bool {
        self.sources.iter().any(|s| s.is_busy())
    }

    fn has_history(&self, mode: SearchHistoryMode) -> bool {
        mode != SearchHistoryMode::Disabled && self.history.as_ref().is_some_and(|h| !h.entries().is_empty())
    }

    fn set_text(st: &mut State, text: &str) {
        st.untrimmed = text.to_string();
        st.query = SearchQuery::new(Some(text), st.query_from_clipboard);
    }

    fn clear_pending(st: &mut State) {
        st.pending_confirmation_id = None;
        st.pending_action = -1;
    }

    fn reset_selection(st: &mut State) {
        st.selected_index = -1;
        st.active_action = -1;
        Self::clear_pending(st);
        st.activate_when_ready = false;
    }

    fn on_source_changed(&self) {
        let proceed = {
            let st = self.state.lock().unwrap();
            st.is_open && !st.showing_history
        };
        if proceed {
            self.refresh();
        }
    }

    fn set_query(&self, text: Option<&str>) {
        {
            let mut st = self.state.lock().unwrap();
            if !st.is_open {
                return;
            }

            let value = text.unwrap_or("");
            if value == st.untrimmed {
                return;
            }

            // Kutu boşalınca pano kaynağı biter; kullanıcı yazdıkça değişmez.
            if value.is_empty() {
                st.query_from_clipboard = false;
            }
            Self::set_text(&mut st, value);
            st.showing_history = false;
            st.activate_when_ready = false;
            Self::clear_pending(&mut st);
        }
        self.refresh();
    }

    fn replace_query(&self, text: &str, from_clipboard: bool) {
        let same = {
            let mut st = self.state.lock().unwrap();
            st.query_from_clipboard = from_clipboard;
            if st.is_open && text == st.untrimmed {
                // Aynı metin ama kaynağı değişmiş olabilir: sorgu yeniden kurulur.
                st.query = SearchQuery::new(Some(&st.untrimmed.clone()), from_clipboard);
                true
            } else {
                false
            }
        };

        if same {
            self.refresh();
        } else {
            self.set_query(Some(text));
        }

        self.query_replaced.invoke(&(utf16_len(text) as i32));
    }

    fn build_history_rows(&self) -> Vec<SearchResult> {
        let Some(history) = &self.history else { return Vec::new() };
        history
            .entries()
            .into_iter()
            .take(SearchHistory::VISIBLE_ENTRIES as usize)
            .enumerate()
            .map(|(index, entry)| {
                let mut result = SearchResult::new(format!("history:{index}"), self.history_category.clone(), entry.clone(), || {});
                result.glyph = SearchGlyph::History;
                result.replaces_query = Some(entry);
                result.skip_history = true;
                let history = Arc::clone(history);
                result.actions = vec![SearchAction::new("remove", self.remove_label.clone(), move || history.remove_at(index as i32))];
                result
            })
            .collect()
    }

    fn remember(&self, result: &SearchResult) {
        let (mode, showing, query, untrimmed, from_clipboard) = {
            let st = self.state.lock().unwrap();
            (st.history_mode, st.showing_history, st.query.clone(), st.untrimmed.clone(), st.query_from_clipboard)
        };

        // Başında boşluk olan arama, kabuktaki gibi geçmişe yazılmaz; pano metni (parola olabilir) hiç yazılmaz.
        if result.skip_history || mode == SearchHistoryMode::Disabled || showing || query.is_empty() || untrimmed.starts_with(' ') || from_clipboard {
            return;
        }

        if let Some(h) = &self.history {
            h.add(&query.raw);
        }
    }

    fn finish(&self, result: SearchResult, stays_open: bool) -> SearchActivationResult {
        self.remember(&result);
        if stays_open {
            // Geçmiş satırından kayıt silindi: liste yerinde tazelenir.
            let mode = self.state.lock().unwrap().history_mode;
            let rows = if self.has_history(mode) { self.build_history_rows() } else { Vec::new() };
            {
                let mut st = self.state.lock().unwrap();
                let index = st.selected_index;
                st.showing_history = !rows.is_empty();
                st.results = rows;
                st.selected_index = i32::min(index, st.results.len() as i32 - 1);
                st.active_action = -1;
            }
            self.changed.invoke(&());
            return SearchActivationResult::new(SearchActivation::QueryUpdated, Some(result));
        }

        SearchActivationResult::new(SearchActivation::Executed, Some(result))
    }

    fn activate(&self, first_action: bool) -> SearchActivationResult {
        let busy = self.is_busy();
        let step = {
            let mut st = self.state.lock().unwrap();
            match Self::selected(&st).cloned() {
                None => {
                    if st.query.is_empty() || !busy {
                        Step::Done(SearchActivationResult::new(SearchActivation::None, None), false)
                    } else {
                        st.activate_when_ready = true;
                        Step::Done(SearchActivationResult::new(SearchActivation::Deferred, None), false)
                    }
                }
                Some(result) => {
                    let action_index = if first_action && !result.actions.is_empty() { 0 } else { st.active_action };
                    if action_index >= 0 && (action_index as usize) < result.actions.len() {
                        let action = result.actions[action_index as usize].clone();
                        let confirmed = st.pending_confirmation_id.as_deref() == Some(result.id.as_str()) && st.pending_action == action_index;
                        if action.requires_confirmation && !confirmed {
                            st.pending_confirmation_id = Some(result.id.clone());
                            st.pending_action = action_index;
                            Step::Done(SearchActivationResult::new(SearchActivation::NeedsConfirmation, Some(result)), true)
                        } else {
                            Self::clear_pending(&mut st);
                            Step::RunAction(action, result)
                        }
                    } else {
                        let confirmed = st.pending_confirmation_id.as_deref() == Some(result.id.as_str()) && st.pending_action == -1;
                        if result.requires_confirmation && !confirmed {
                            st.pending_confirmation_id = Some(result.id.clone());
                            st.pending_action = -1;
                            Step::Done(SearchActivationResult::new(SearchActivation::NeedsConfirmation, Some(result)), true)
                        } else {
                            Self::clear_pending(&mut st);
                            Step::RunResult(result)
                        }
                    }
                }
            }
        };

        match step {
            Step::Done(r, fire_changed) => {
                if fire_changed {
                    self.changed.invoke(&());
                }
                r
            }
            Step::RunAction(action, result) => {
                let execute = Arc::clone(&action.execute);
                CrashLog::guard("Arama eylemi", move || execute());
                let stays_open = self.state.lock().unwrap().showing_history;
                self.finish(result, stays_open)
            }
            Step::RunResult(result) => {
                let execute = Arc::clone(&result.execute);
                CrashLog::guard("Arama sonucu çalıştırma", move || execute());

                if let Some(replacement) = result.replaces_query.clone() {
                    self.remember(&result);
                    self.replace_query(&replacement, false);
                    return SearchActivationResult::new(SearchActivation::QueryUpdated, Some(result));
                }

                self.finish(result, false)
            }
        }
    }

    fn refresh(&self) {
        let (query, previous) = {
            let st = self.state.lock().unwrap();
            (st.query.clone(), Self::selected(&st).cloned())
        };

        let mut found: Vec<SearchResult> = Vec::new();
        if !query.is_empty() {
            for source in &self.sources {
                match catch_unwind(AssertUnwindSafe(|| source.search(&query))) {
                    Ok(list) => found.extend(list),
                    // Bir kaynağın hatası diğerlerinin sonuçlarını götürmesin.
                    Err(payload) => {
                        CrashLog::write("Arama kaynağı", &CrashError::exception("Panic", &panic_message(payload.as_ref()), ""));
                    }
                }
            }
        }

        // Aynı hedefe giden sonuçlar tekilleşir; en önemli olan kalır.
        let mut unique: Vec<SearchResult> = Vec::new();
        for r in found {
            match unique.iter().position(|u| u.id == r.id) {
                Some(i) => {
                    let u = &unique[i];
                    // OrderByDescending(CategoryRelevance).ThenByDescending(Relevance).First(): eşitlikte ilk görülen.
                    let better = r.category_relevance > u.category_relevance
                        || (r.category_relevance == u.category_relevance && r.relevance > u.relevance);
                    if better {
                        unique[i] = r;
                    }
                }
                None => unique.push(r),
            }
        }

        if unique.is_empty() && !query.is_empty() && !self.is_busy() {
            if let Some(fallback) = self.fallback.as_ref().and_then(|f| f(&query)) {
                unique.push(fallback);
            }
        }

        let arranged = SearchLayout::arrange(unique, SearchLayout::MAX_RESULTS);

        {
            let mut st = self.state.lock().unwrap();
            st.results = arranged;

            // Liste tazelenince kullanıcının seçimi korunur; dokunmadıysa en üstteki seçili olur.
            let kept = match &previous {
                Some(p) if st.selected_index > 0 => st.results.iter().position(|r| r.id == p.id).map_or(-1, |i| i as i32),
                _ => -1,
            };
            let selection_moved = kept < 0 || previous.as_ref().is_none_or(|p| st.results[kept as usize].id != p.id);
            st.selected_index = if kept >= 0 {
                kept
            } else if !st.results.is_empty() {
                0
            } else {
                -1
            };
            if selection_moved {
                st.active_action = -1;
            }

            if let Some(pending) = st.pending_confirmation_id.clone() {
                if st.results.iter().all(|r| r.id != pending) {
                    Self::clear_pending(&mut st);
                }
            }
        }

        self.changed.invoke(&());

        let (waiting, has_results) = {
            let st = self.state.lock().unwrap();
            (st.activate_when_ready, !st.results.is_empty())
        };
        if !waiting {
            return;
        }

        if has_results {
            self.state.lock().unwrap().activate_when_ready = false;
            let result = self.activate(false);
            if result.outcome == SearchActivation::Executed {
                if let Some(executed) = result.result {
                    self.auto_activated.invoke(&executed);
                }
            }
        } else if !self.is_busy() {
            self.state.lock().unwrap().activate_when_ready = false;
        }
    }
}

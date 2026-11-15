use super::search_sources::{AppEntry, AppIconFn};
use super::{
    is_canceled, log_service_error, AppSearchSource, IInstalledAppsService, ISearchSource, InstalledApp, SearchDragData,
    SearchDragKind, SearchGlyph, SearchMatcher, SearchQuery, SearchRelevance, SearchResult, SearchSourceBase, SearchText,
    Translate,
};
use crate::cancel::CancellationTokenSource;
use crate::event::Event;
use crate::jump_list::{IJumpListService, JumpListEntry, JumpListSectionKind};
use crate::text;
use crate::win_path;
use std::collections::HashSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct CachedAction {
    app: InstalledApp,
    section_label: String,
    entry: JumpListEntry,
    folded_title: String,
}

struct JumpState {
    cts: Option<CancellationTokenSource>,
    /// `_fetching` (OrdinalIgnoreCase): anahtar büyük harfe çevrilmiş.
    fetching: HashSet<String>,
    apps: Vec<AppEntry>,
    /// `_loaded` (OrdinalIgnoreCase): uygulama kimliği (büyük harf) → eylemler; ekleme sırası korunur.
    loaded: Vec<(String, Vec<CachedAction>)>,
}

struct JumpInner {
    apps_service: Arc<dyn IInstalledAppsService>,
    jump_list_service: Arc<dyn IJumpListServiceShared>,
    changed: Arc<Event<()>>,
    state: Mutex<JumpState>,
}

/// `IJumpListService` + `Send + Sync` (arka plan iş parçacığından çağrılır).
pub trait IJumpListServiceShared: IJumpListService + Send + Sync {}

impl<T: IJumpListService + Send + Sync> IJumpListServiceShared for T {}

/// Kurulu uygulamaların Windows Atlama Listesi (Jump List) görev ve son öğelerini arar.
/// Yalnız sorguyla eşleşen en üstteki birkaç uygulama için tembel yükleme yapar; boşta çalışmaz.
pub struct JumpListSearchSource {
    pub base: SearchSourceBase,
    load_icon: Option<AppIconFn>,
    inner: Arc<JumpInner>,
}

const MIN_QUERY_LENGTH: usize = 2;
const MAX_APPS_TO_FETCH: usize = 3;
const MAX_RESULTS: usize = 8;

impl JumpListSearchSource {
    pub fn new(
        apps_service: Arc<dyn IInstalledAppsService>,
        jump_list_service: Arc<dyn IJumpListServiceShared>,
        t: Translate,
        load_icon: Option<AppIconFn>,
    ) -> Self {
        let name = t("SearchCatJumpList");
        let base = SearchSourceBase::new("jumplist", name);
        let inner = Arc::new(JumpInner {
            apps_service,
            jump_list_service,
            changed: base.changed_handle(),
            state: Mutex::new(JumpState { cts: None, fetching: HashSet::new(), apps: Vec::new(), loaded: Vec::new() }),
        });
        JumpListSearchSource { base, load_icon, inner }
    }

    fn key(id: &str) -> String {
        text::to_upper_invariant(id)
    }

    fn first_word(text: &str) -> &str {
        match text.find(' ').filter(|&i| i > 0) {
            Some(space) => &text[..space],
            None => text,
        }
    }

    fn build_drag(entry: &JumpListEntry) -> Option<SearchDragData> {
        let launch = entry.launch.as_ref()?;
        let target = &launch.target;
        if text::is_null_or_white_space(Some(target)) {
            return None;
        }

        if text::starts_with_ignore_case(target, "http://") || text::starts_with_ignore_case(target, "https://") {
            return Some(SearchDragData::new(SearchDragKind::Url, target.clone()));
        }

        if launch.is_document && win_path::is_path_rooted(target) {
            return Some(SearchDragData::new(SearchDragKind::File, target.clone()));
        }

        None
    }

    fn ensure_fetched(&self, app: &InstalledApp) {
        let key = Self::key(&app.id);
        let token;
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.loaded.iter().any(|(k, _)| *k == key) {
                return;
            }
            let Some(cts) = state.cts.as_ref() else { return };
            let t = cts.token();
            if !state.fetching.insert(key.clone()) {
                return;
            }
            token = t;
        }

        let inner = Arc::clone(&self.inner);
        let app = app.clone();
        std::thread::spawn(move || Self::fetch_app_jump_list(inner, app, token));
    }

    /// C# `LoadAppsAsync`.
    fn load_apps(inner: Arc<JumpInner>, cts: CancellationTokenSource) {
        let token = cts.token();
        match catch_unwind(AssertUnwindSafe(|| inner.apps_service.load(&token))) {
            Ok(Ok(apps)) => {
                if cts.is_cancellation_requested() {
                    return;
                }

                inner.state.lock().unwrap().apps = AppSearchSource::build_entries(apps);
                inner.changed.invoke(&());
            }
            Ok(Err(e)) if is_canceled(&e) => {}
            Ok(Err(e)) => log_service_error("Atlama listesi uygulamaları", &e),
            Err(payload) => {
                log_service_error("Atlama listesi uygulamaları", &super::search_util::panic_message(payload.as_ref()).into())
            }
        }
    }

    /// C# `FetchAppJumpListAsync`.
    fn fetch_app_jump_list(inner: Arc<JumpInner>, app: InstalledApp, token: crate::cancel::CancellationToken) {
        let outcome = catch_unwind(AssertUnwindSafe(|| inner.jump_list_service.get(&app.id)));
        match outcome {
            Ok(sections) => {
                if !token.is_cancellation_requested() {
                    let mut list = Vec::new();
                    for section in &sections {
                        let label = match &section.name {
                            Some(n) if section.kind == JumpListSectionKind::Custom && !text::is_null_or_white_space(Some(n)) => n.clone(),
                            _ => String::new(),
                        };
                        for entry in &section.entries {
                            if entry.is_separator() || text::is_null_or_white_space(Some(&entry.title)) {
                                continue;
                            }
                            list.push(CachedAction {
                                app: app.clone(),
                                section_label: label.clone(),
                                entry: entry.clone(),
                                folded_title: SearchText::fold_str(&entry.title),
                            });
                        }
                    }

                    let non_empty = !list.is_empty();
                    {
                        let mut state = inner.state.lock().unwrap();
                        let key = Self::key(&app.id);
                        match state.loaded.iter_mut().find(|(k, _)| *k == key) {
                            Some((_, v)) => *v = list,
                            None => state.loaded.push((key, list)),
                        }
                    }
                    if non_empty {
                        inner.changed.invoke(&());
                    }
                }
            }
            Err(payload) => {
                log_service_error("Atlama listesi okuma", &super::search_util::panic_message(payload.as_ref()).into())
            }
        }

        // finally
        inner.state.lock().unwrap().fetching.remove(&Self::key(&app.id));
    }
}

impl Drop for JumpListSearchSource {
    fn drop(&mut self) {
        ISearchSource::release(self);
    }
}

impl ISearchSource for JumpListSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {
        let cts = {
            let mut state = self.inner.state.lock().unwrap();
            if state.cts.is_some() {
                return;
            }
            let cts = CancellationTokenSource::new();
            state.cts = Some(cts.clone());
            cts
        };

        let inner = Arc::clone(&self.inner);
        std::thread::spawn(move || Self::load_apps(inner, cts));
    }

    fn release(&self) {
        let cts = {
            let mut state = self.inner.state.lock().unwrap();
            let cts = state.cts.take();
            state.fetching.clear();
            state.loaded.clear();
            cts
        };

        if let Some(cts) = cts {
            cts.cancel();
        }
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let apps = self.inner.state.lock().unwrap().apps.clone();
        if query.folded.chars().count() < MIN_QUERY_LENGTH || apps.is_empty() {
            return Vec::new();
        }

        let first_word = Self::first_word(&query.folded);
        let app_query =
            if first_word.chars().count() >= MIN_QUERY_LENGTH { SearchQuery::from_text(first_word) } else { query.clone() };

        let mut scored: Vec<(&AppEntry, f64)> = apps
            .iter()
            .map(|a| {
                let score = f64::max(
                    f64::max(SearchMatcher::score(query, &a.name), SearchMatcher::score(query, &a.key) * 0.6),
                    f64::max(SearchMatcher::score(&app_query, &a.name), SearchMatcher::score(&app_query, &a.key) * 0.6) * 0.85,
                );
                (a, score)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(MAX_APPS_TO_FETCH);

        for (app_entry, _) in &scored {
            self.ensure_fetched(&app_entry.app);
        }

        let remainder = if first_word.len() < query.folded.len() {
            let rest: String = query.folded.chars().skip(first_word.chars().count() + 1).collect();
            Some(SearchQuery::from_text(rest.trim()))
        } else {
            None
        };

        let loaded = self.inner.state.lock().unwrap().loaded.clone();
        let mut results: Vec<(SearchResult, f64)> = Vec::new();
        for (_, actions) in &loaded {
            for item in actions {
                let app_name = SearchText::fold_str(&item.app.name);
                let app_score = f64::max(SearchMatcher::score(&app_query, &app_name), SearchMatcher::score(query, &app_name));
                let title_score = match &remainder {
                    Some(r) => f64::max(SearchMatcher::score(r, &item.folded_title), SearchMatcher::score(query, &item.folded_title)),
                    None => SearchMatcher::score(query, &item.folded_title),
                };

                let combined = if remainder.is_some() && app_score > 0.0 {
                    if title_score > 0.0 { f64::max(title_score, app_score * 0.7) } else { app_score * 0.45 }
                } else {
                    f64::max(title_score, app_score * 0.45)
                };

                if combined <= 0.0 {
                    continue;
                }

                let entry = item.entry.clone();
                let app = item.app.clone();
                let service = Arc::clone(&self.inner.jump_list_service);
                let launching = entry.clone();
                let mut result = SearchResult::new(
                    format!("jumplist:{}:{}", app.id, entry.title),
                    self.base.name(),
                    entry.title.clone(),
                    move || service.launch(&launching, None),
                );
                result.subtitle =
                    Some(if item.section_label.is_empty() { app.name.clone() } else { format!("{} · {}", app.name, item.section_label) });
                result.glyph = SearchGlyph::App;
                result.category_relevance = if title_score >= 0.8 { SearchRelevance::HIGH } else { SearchRelevance::LOW };
                result.relevance = combined;
                result.load_icon = self.load_icon.clone().map(|f| {
                    let app = app.clone();
                    Arc::new(move || f(&app)) as Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>
                });
                result.drag = Self::build_drag(&entry);
                results.push((result, combined));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.into_iter().take(MAX_RESULTS).map(|r| r.0).collect()
    }
}

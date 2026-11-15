use super::support::TempDir;
use crate::search::*;
use crate::search_source_base_methods;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

/// C# testindeki `FakeSource`.
struct FakeSource {
    base: SearchSourceBase,
    prepared: AtomicI32,
    released: AtomicI32,
    items: Mutex<Vec<SearchResult>>,
    throws: AtomicBool,
    busy: AtomicBool,
}

impl FakeSource {
    fn new(results: Vec<SearchResult>) -> Arc<FakeSource> {
        Arc::new(FakeSource {
            base: SearchSourceBase::new("fake", "Fake"),
            prepared: AtomicI32::new(0),
            released: AtomicI32::new(0),
            items: Mutex::new(results),
            throws: AtomicBool::new(false),
            busy: AtomicBool::new(false),
        })
    }

    fn set_items(&self, items: Vec<SearchResult>) {
        *self.items.lock().unwrap() = items;
    }

    fn raise(&self) {
        self.base.raise_changed();
    }
}

impl ISearchSource for FakeSource {
    search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    fn prepare(&self) {
        self.prepared.fetch_add(1, Ordering::SeqCst);
    }

    fn release(&self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }

    fn search(&self, _query: &SearchQuery) -> Vec<SearchResult> {
        if self.throws.load(Ordering::SeqCst) {
            panic!("kaynak hatası");
        }
        self.items.lock().unwrap().clone()
    }
}

fn r(id: &str) -> SearchResult {
    r_full(id, "A", SearchRelevance::MODERATE, 0.5, || {})
}

fn r_rel(id: &str, relevance: f64) -> SearchResult {
    r_full(id, "A", SearchRelevance::MODERATE, relevance, || {})
}

fn r_cat(id: &str, category: &str, category_relevance: i32, relevance: f64) -> SearchResult {
    r_full(id, category, category_relevance, relevance, || {})
}

fn r_exec(id: &str, execute: impl Fn() + Send + Sync + 'static) -> SearchResult {
    r_full(id, "A", SearchRelevance::MODERATE, 0.5, execute)
}

fn r_full(id: &str, category: &str, category_relevance: i32, relevance: f64, execute: impl Fn() + Send + Sync + 'static) -> SearchResult {
    SearchResult { category_relevance, relevance, ..SearchResult::new(id, category, id, execute) }
}

fn ids(list: &[SearchResult]) -> Vec<String> {
    list.iter().map(|r| r.id.clone()).collect()
}

fn dyn_sources(list: &[&Arc<FakeSource>]) -> Vec<Arc<dyn ISearchSource>> {
    list.iter().map(|s| Arc::clone(s) as Arc<dyn ISearchSource>).collect()
}

fn new_panel(source: &Arc<FakeSource>) -> SearchPanelController {
    SearchPanelController::new(dyn_sources(&[source]), None, None, None, "", "")
}

fn open_with(results: Vec<SearchResult>) -> SearchPanelController {
    let panel = new_panel(&FakeSource::new(results));
    panel.open();
    panel.set_query(Some("x"));
    panel
}

struct Fixture {
    dir: TempDir,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture { dir: TempDir::new("koy-tests-") }
    }

    fn history(&self) -> Arc<SearchHistory> {
        Arc::new(SearchHistory::new(self.dir.path("history.txt")))
    }

    fn open_with_history(&self, history: &Arc<SearchHistory>, results: Vec<SearchResult>) -> SearchPanelController {
        let panel = SearchPanelController::new(
            dyn_sources(&[&FakeSource::new(results)]),
            None,
            Some(Arc::clone(history)),
            None,
            "Son",
            "Sil",
        );
        panel.open();
        panel
    }
}

// --- Sıralama ---

#[test]
fn categories_sort_by_importance_and_stay_together() {
    let list = SearchLayout::arrange(
        vec![
            r_cat("low1", "L", SearchRelevance::LOW, 0.9),
            r_cat("high1", "H", SearchRelevance::HIGH, 0.1),
            r_cat("low2", "L", SearchRelevance::LOW, 0.95),
            r_cat("high2", "H", SearchRelevance::MODERATE, 0.8),
        ],
        SearchLayout::MAX_RESULTS,
    );

    assert_eq!(vec!["high2", "high1", "low2", "low1"], ids(&list));
}

#[test]
fn apps_stay_on_top_and_files_at_the_bottom_whatever_their_importance() {
    let list = SearchLayout::arrange(
        vec![
            SearchResult { placement: SearchCategoryPlacement::Bottom, ..r_cat("file", "F", SearchRelevance::HIGHEST, 0.9) },
            r_cat("session", "S", SearchRelevance::LOW, 0.9),
            r_cat("calc", "C", SearchRelevance::HIGH, 0.9),
            SearchResult { placement: SearchCategoryPlacement::Top, ..r_cat("app", "A", SearchRelevance::MODERATE, 0.1) },
        ],
        SearchLayout::MAX_RESULTS,
    );

    assert_eq!(vec!["app", "calc", "session", "file"], ids(&list));
}

#[test]
fn single_category_may_use_whole_limit() {
    let list = SearchLayout::arrange((0..40).map(|i| r(&format!("a{i}"))).collect(), SearchLayout::MAX_RESULTS);

    assert_eq!(SearchLayout::MAX_RESULTS as usize, list.len());
}

#[test]
fn first_category_gets_at_most_half_and_every_category_is_shown() {
    let mut results = Vec::new();
    for c in ["A", "B", "C"] {
        for i in 0..20 {
            let relevance = match c {
                "A" => SearchRelevance::HIGH,
                "B" => SearchRelevance::MODERATE,
                _ => SearchRelevance::LOW,
            };
            results.push(r_cat(&format!("{c}{i}"), c, relevance, 0.5));
        }
    }

    let list = SearchLayout::arrange(results, SearchLayout::MAX_RESULTS);

    assert_eq!(8, list.iter().filter(|r| r.category == "A").count()); // 15 / 2 yukarı
    assert!(list.iter().any(|r| r.category == "C"));
    assert!(list.len() <= SearchLayout::MAX_RESULTS as usize);
}

// --- Panel yaşam döngüsü ---

#[test]
fn open_prepares_and_close_releases_sources() {
    let source = FakeSource::new(vec![]);
    let panel = new_panel(&source);

    panel.open();
    panel.open();
    assert_eq!(1, source.prepared.load(Ordering::SeqCst));

    panel.close();
    panel.close();
    assert_eq!(1, source.released.load(Ordering::SeqCst));
}

#[test]
fn prior_search_is_restored_by_default() {
    let panel = open_with(vec![r("a")]);
    panel.set_query(Some("son arama"));

    panel.close();
    panel.open();

    assert_eq!("son arama", panel.query());
    assert_eq!(1, panel.results().len());
}

#[test]
fn prior_search_can_be_turned_off() {
    let panel = new_panel(&FakeSource::new(vec![r("a")]));
    panel.set_retain_prior_search(false);
    panel.open();
    panel.set_query(Some("gizli"));

    panel.close();
    panel.open();

    assert_eq!("", panel.query());
    assert!(panel.results().is_empty());
}

#[test]
fn empty_query_shows_nothing() {
    let panel = open_with(vec![r("a")]);

    panel.set_query(Some("   "));

    assert!(panel.results().is_empty());
    assert_eq!(-1, panel.selected_index());
}

// --- Seçim ve eylemler ---

#[test]
fn first_result_is_selected_and_selection_wraps() {
    let panel = open_with(vec![r_rel("a", 0.9), r_rel("b", 0.5)]);
    assert_eq!(0, panel.selected_index());

    panel.move_selection(-1);
    assert_eq!(1, panel.selected_index());

    panel.move_selection(1);
    assert_eq!(0, panel.selected_index());
}

#[test]
fn category_jump_goes_to_next_group_and_wraps() {
    let panel = open_with(vec![
        r_cat("a1", "A", SearchRelevance::HIGH, 0.9),
        r_cat("a2", "A", SearchRelevance::HIGH, 0.8),
        r_cat("b1", "B", SearchRelevance::LOW, 0.5),
    ]);

    panel.move_category(1);
    assert_eq!(Some("b1".to_string()), panel.selected().map(|s| s.id));

    panel.move_category(1);
    assert_eq!(Some("a1".to_string()), panel.selected().map(|s| s.id));

    panel.move_category(-1);
    assert_eq!(Some("b1".to_string()), panel.selected().map(|s| s.id));
}

#[test]
fn activate_runs_selected_result() {
    let ran = Arc::new(AtomicI32::new(0));
    let counter = Arc::clone(&ran);
    let result = open_with(vec![r_exec("a", move || {
        counter.fetch_add(1, Ordering::SeqCst);
    })])
    .activate(false);

    assert_eq!(SearchActivation::Executed, result.outcome);
    assert_eq!(1, ran.load(Ordering::SeqCst));
}

#[test]
fn activate_with_no_result_does_nothing() {
    assert_eq!(SearchActivation::None, SearchPanelController::new(vec![], None, None, None, "", "").activate(false).outcome);
}

#[test]
fn tab_cycles_actions_then_reports_end() {
    let item = SearchResult { actions: vec![SearchAction::new("x", "X", || {}), SearchAction::new("y", "Y", || {})], ..r("a") };
    let panel = open_with(vec![item]);

    assert!(panel.cycle_action(1));
    assert_eq!(0, panel.active_action());
    assert!(panel.cycle_action(1));
    assert_eq!(1, panel.active_action());
    assert!(!panel.cycle_action(1));

    assert!(panel.cycle_action(-1));
    assert!(panel.cycle_action(-1));
    assert_eq!(-1, panel.active_action());
    assert!(!panel.cycle_action(-1));
}

fn counter() -> (Arc<AtomicI32>, impl Fn() + Send + Sync + 'static) {
    let c = Arc::new(AtomicI32::new(0));
    let inner = Arc::clone(&c);
    (c, move || {
        inner.fetch_add(1, Ordering::SeqCst);
    })
}

#[test]
fn enter_on_highlighted_action_runs_it_instead_of_the_row() {
    let (row, row_fn) = counter();
    let (action, action_fn) = counter();
    let panel = open_with(vec![SearchResult { actions: vec![SearchAction::new("x", "X", action_fn)], ..r_exec("a", row_fn) }]);

    panel.cycle_action(1);
    panel.activate(false);

    assert_eq!((0, 1), (row.load(Ordering::SeqCst), action.load(Ordering::SeqCst)));
}

#[test]
fn shift_enter_runs_first_action() {
    let (row, row_fn) = counter();
    let (action, action_fn) = counter();
    let panel = open_with(vec![SearchResult { actions: vec![SearchAction::new("x", "X", action_fn)], ..r_exec("a", row_fn) }]);

    panel.activate(true);

    assert_eq!((0, 1), (row.load(Ordering::SeqCst), action.load(Ordering::SeqCst)));
}

#[test]
fn moving_selection_resets_highlighted_action() {
    let item = SearchResult { actions: vec![SearchAction::new("x", "X", || {})], ..r_rel("a", 0.9) };
    let panel = open_with(vec![item, r("b")]);
    panel.cycle_action(1);

    panel.move_selection(1);

    assert_eq!(-1, panel.active_action());
}

// --- Onay ---

#[test]
fn dangerous_result_needs_second_activation() {
    let (ran, ran_fn) = counter();
    let panel = open_with(vec![SearchResult { requires_confirmation: true, ..r_exec("kapat", ran_fn) }]);

    assert_eq!(SearchActivation::NeedsConfirmation, panel.activate(false).outcome);
    assert_eq!(0, ran.load(Ordering::SeqCst));
    assert_eq!(Some("kapat".to_string()), panel.pending_confirmation_id());

    assert_eq!(SearchActivation::Executed, panel.activate(false).outcome);
    assert_eq!(1, ran.load(Ordering::SeqCst));
}

#[test]
fn moving_selection_cancels_confirmation() {
    let (ran, ran_fn) = counter();
    let first = SearchResult { requires_confirmation: true, relevance: 0.9, ..r_exec("kapat", ran_fn) };
    let panel = open_with(vec![first, r("b")]);
    panel.activate(false);

    panel.move_selection(1);
    panel.move_selection(-1);

    assert!(panel.pending_confirmation_id().is_none());
    assert_eq!(SearchActivation::NeedsConfirmation, panel.activate(false).outcome);
    assert_eq!(0, ran.load(Ordering::SeqCst));
}

#[test]
fn typing_cancels_confirmation() {
    let panel = open_with(vec![SearchResult { requires_confirmation: true, ..r("kapat") }]);
    panel.activate(false);

    panel.set_query(Some("xy"));

    assert!(panel.pending_confirmation_id().is_none());
}

// --- Kaynak olayları ---

#[test]
fn failing_source_does_not_lose_other_results() {
    let failing = FakeSource::new(vec![]);
    failing.throws.store(true, Ordering::SeqCst);
    let good = FakeSource::new(vec![r("a")]);
    let panel = SearchPanelController::new(dyn_sources(&[&failing, &good]), None, None, None, "", "");
    panel.open();

    panel.set_query(Some("x"));

    assert_eq!(1, panel.results().len());
}

#[test]
fn duplicate_targets_keep_the_most_important_one() {
    let panel = open_with(vec![r_cat("path:x", "Recent", SearchRelevance::LOW, 0.5), r_cat("path:x", "Places", SearchRelevance::HIGH, 0.5)]);

    let results = panel.results();
    assert_eq!(1, results.len());
    assert_eq!("Places", results[0].category);
}

#[test]
fn untouched_selection_follows_new_top_result() {
    let source = FakeSource::new(vec![r_rel("a", 0.5)]);
    let panel = new_panel(&source);
    panel.open();
    panel.set_query(Some("x"));

    source.set_items(vec![r_rel("yeni", 0.9), r_rel("a", 0.5)]);
    source.raise();

    assert_eq!(Some("yeni".to_string()), panel.selected().map(|s| s.id));
}

#[test]
fn moved_selection_survives_a_refresh() {
    let source = FakeSource::new(vec![r_rel("a", 0.9), r_rel("b", 0.5)]);
    let panel = new_panel(&source);
    panel.open();
    panel.set_query(Some("x"));
    panel.move_selection(1);

    source.set_items(vec![r_rel("yeni", 0.95), r_rel("a", 0.9), r_rel("b", 0.5)]);
    source.raise();

    assert_eq!(Some("b".to_string()), panel.selected().map(|s| s.id));
}

#[test]
fn source_events_go_through_post_and_stop_after_close() {
    let posted = Arc::new(AtomicI32::new(0));
    let source = FakeSource::new(vec![r("a")]);
    let counter = Arc::clone(&posted);
    let post: PostFn = Arc::new(move |action: Box<dyn FnOnce() + Send>| {
        counter.fetch_add(1, Ordering::SeqCst);
        action();
    });
    let panel = SearchPanelController::new(dyn_sources(&[&source]), Some(post), None, None, "", "");
    panel.open();
    source.raise();
    assert_eq!(1, posted.load(Ordering::SeqCst));

    panel.close();
    source.raise();
    assert_eq!(1, posted.load(Ordering::SeqCst));
}

#[test]
fn enter_before_results_runs_first_result_when_it_arrives() {
    let (ran, ran_fn) = counter();
    let source = FakeSource::new(vec![]);
    source.busy.store(true, Ordering::SeqCst);
    let panel = new_panel(&source);
    let auto: Arc<Mutex<Option<SearchResult>>> = Arc::default();
    let sink = Arc::clone(&auto);
    panel.auto_activated().subscribe(move |r| *sink.lock().unwrap() = Some(r.clone()));
    panel.open();
    panel.set_query(Some("x"));

    assert_eq!(SearchActivation::Deferred, panel.activate(false).outcome);

    source.set_items(vec![r_exec("a", ran_fn)]);
    source.busy.store(false, Ordering::SeqCst);
    source.raise();

    assert_eq!(1, ran.load(Ordering::SeqCst));
    assert_eq!(Some("a".to_string()), auto.lock().unwrap().as_ref().map(|r| r.id.clone()));
}

#[test]
fn typing_cancels_deferred_enter() {
    let (ran, ran_fn) = counter();
    let source = FakeSource::new(vec![]);
    source.busy.store(true, Ordering::SeqCst);
    let panel = new_panel(&source);
    panel.open();
    panel.set_query(Some("x"));
    panel.activate(false);

    panel.set_query(Some("xy"));
    source.set_items(vec![r_exec("a", ran_fn)]);
    source.raise();

    assert_eq!(0, ran.load(Ordering::SeqCst));
}

#[test]
fn fallback_is_used_only_when_nothing_found_and_search_finished() {
    let source = FakeSource::new(vec![]);
    source.busy.store(true, Ordering::SeqCst);
    let fallback: FallbackFn = Arc::new(|_| Some(r("web")));
    let panel = SearchPanelController::new(dyn_sources(&[&source]), None, None, Some(fallback), "", "");
    panel.open();
    panel.set_query(Some("x"));
    assert!(panel.results().is_empty());

    source.busy.store(false, Ordering::SeqCst);
    source.raise();
    assert_eq!(vec!["web"], ids(&panel.results()));

    source.set_items(vec![r("a")]);
    source.raise();
    assert_eq!(vec!["a"], ids(&panel.results()));
}

#[test]
fn replacing_result_updates_query_and_keeps_panel_open() {
    let panel = open_with(vec![SearchResult { replaces_query: Some("96".to_string()), ..r("a") }]);
    let cursor = Arc::new(AtomicI32::new(-1));
    let sink = Arc::clone(&cursor);
    panel.query_replaced().subscribe(move |c| sink.store(*c, Ordering::SeqCst));

    let result = panel.activate(false);

    assert_eq!(SearchActivation::QueryUpdated, result.outcome);
    assert_eq!("96", panel.query());
    assert_eq!(2, cursor.load(Ordering::SeqCst));
}

// --- Geçmiş ---

#[test]
fn history_keeps_newest_first_without_duplicates_and_caps() {
    let fx = Fixture::new();
    let history = fx.history();
    for i in 0..SearchHistory::MAX_ENTRIES + 5 {
        history.add(&format!("q{i}"));
    }
    history.add("q10");

    assert_eq!(SearchHistory::MAX_ENTRIES as usize, history.entries().len());
    assert_eq!("q10", history.entries()[0]);
    assert_eq!(1, history.entries().iter().filter(|e| *e == "q10").count());
}

#[test]
fn history_persists_between_instances() {
    let fx = Fixture::new();
    fx.history().add("kalıcı");

    assert_eq!("kalıcı", fx.history().entries()[0]);
}

#[test]
fn history_suggests_newest_match_ignoring_case_and_turkish_letters() {
    let fx = Fixture::new();
    let history = fx.history();
    history.add("Şirket raporu");
    history.add("şirket planı");

    assert_eq!(Some("şirket planı".to_string()), history.suggest("sir"));
    assert!(history.suggest("yok").is_none());
    assert!(history.suggest("").is_none());
}

#[test]
fn executed_search_is_remembered_trimmed() {
    let fx = Fixture::new();
    let history = fx.history();
    let panel = fx.open_with_history(&history, vec![r("a")]);
    panel.set_query(Some("  chrome "));
    panel.set_query(Some("chrome "));

    panel.activate(false);

    assert_eq!("chrome", history.entries()[0]);
}

#[test]
fn search_starting_with_space_is_not_remembered() {
    let fx = Fixture::new();
    let history = fx.history();
    let panel = fx.open_with_history(&history, vec![r("a")]);
    panel.set_query(Some(" gizli"));

    panel.activate(false);

    assert!(history.entries().is_empty());
}

#[test]
fn disabled_mode_records_and_suggests_nothing() {
    let fx = Fixture::new();
    let history = fx.history();
    history.add("eski");
    let panel = fx.open_with_history(&history, vec![r("a")]);
    panel.set_history_mode(SearchHistoryMode::Disabled);
    panel.set_query(Some("eski ara"));

    panel.activate(false);

    assert_eq!(1, history.entries().len());
    assert_eq!("", panel.suggestion());
    assert!(!panel.has_history());
}

#[test]
fn result_marked_skip_is_not_remembered() {
    let fx = Fixture::new();
    let history = fx.history();
    let panel = fx.open_with_history(&history, vec![SearchResult { skip_history: true, ..r("a") }]);
    panel.set_query(Some("?"));

    panel.activate(false);

    assert!(history.entries().is_empty());
}

#[test]
fn suggestion_comes_from_history() {
    let fx = Fixture::new();
    let history = fx.history();
    history.add("notepad");
    let panel = fx.open_with_history(&history, vec![]);

    panel.set_query(Some("not"));

    assert_eq!("notepad", panel.suggestion());
}

#[test]
fn history_list_toggles_on_empty_box_and_entry_fills_query() {
    let fx = Fixture::new();
    let history = fx.history();
    history.add("eski");
    let panel = fx.open_with_history(&history, vec![r("a")]);

    panel.toggle_history();
    assert!(panel.showing_history());
    let results = panel.results();
    assert_eq!(1, results.len());
    assert_eq!("eski", results[0].title);

    let result = panel.activate(false);

    assert_eq!(SearchActivation::QueryUpdated, result.outcome);
    assert_eq!("eski", panel.query());
    assert!(!panel.showing_history());
}

#[test]
fn history_list_shows_at_most_twenty() {
    let fx = Fixture::new();
    let history = fx.history();
    for i in 0..40 {
        history.add(&format!("q{i}"));
    }
    let panel = fx.open_with_history(&history, vec![]);

    panel.toggle_history();

    assert_eq!(SearchHistory::VISIBLE_ENTRIES as usize, panel.results().len());
}

#[test]
fn history_entry_can_be_removed_and_list_stays_open() {
    let fx = Fixture::new();
    let history = fx.history();
    history.add("bir");
    history.add("iki");
    let panel = fx.open_with_history(&history, vec![]);
    panel.toggle_history();

    let result = panel.activate(true);

    assert_eq!(SearchActivation::QueryUpdated, result.outcome);
    assert_eq!(vec!["bir"], history.entries());
    assert!(panel.showing_history());
    assert_eq!(1, panel.results().len());
}

#[test]
fn removing_last_history_entry_closes_the_list() {
    let fx = Fixture::new();
    let history = fx.history();
    history.add("tek");
    let panel = fx.open_with_history(&history, vec![]);
    panel.toggle_history();

    panel.activate(true);

    assert!(!panel.showing_history());
    assert!(panel.results().is_empty());
}

#[test]
fn clear_history_empties_store_and_list() {
    let fx = Fixture::new();
    let history = fx.history();
    history.add("a");
    let panel = fx.open_with_history(&history, vec![]);
    panel.toggle_history();

    panel.clear_history();

    assert!(history.entries().is_empty());
    assert!(!panel.showing_history());
}

// --- Geçmiş gizliliği ---

#[test]
fn history_never_saves_control_characters() {
    for term in ["a\nb", "a\r\nb", "a\u{7}b", ""] {
        let fx = Fixture::new();
        let history = fx.history();
        history.add("güvenli");
        history.add(term);

        assert_eq!(vec!["güvenli"], history.entries(), "{term:?}");
        assert_eq!(vec!["güvenli"], SearchHistory::new(fx.dir.path("history.txt")).entries(), "{term:?}");
    }
}

#[test]
fn history_skips_overlong_entries() {
    let fx = Fixture::new();
    let history = fx.history();
    history.add(&"x".repeat(SearchHistory::MAX_ENTRY_LENGTH as usize + 1));
    history.add(&"y".repeat(SearchHistory::MAX_ENTRY_LENGTH as usize));

    assert_eq!(1, history.entries().len());
}

#[test]
fn history_ignores_corrupted_lines_when_loading() {
    let fx = Fixture::new();
    std::fs::create_dir_all(&fx.dir.0).unwrap();
    std::fs::write(fx.dir.path("history.txt"), "iyi\n\u{7}kötü\nbaşka\n").unwrap();

    assert_eq!(vec!["iyi", "başka"], fx.history().entries());
}

#[test]
fn history_is_off_by_default_for_new_settings() {
    assert_eq!(SearchHistoryMode::Disabled, crate::settings::AppSettings::default().search.history_mode);
}

#[test]
fn clipboard_query_is_not_saved_to_history() {
    let fx = Fixture::new();
    let history = fx.history();
    let panel = SearchPanelController::new(
        dyn_sources(&[&FakeSource::new(vec![r("a")])]),
        None,
        Some(Arc::clone(&history)),
        None,
        "",
        "",
    );
    panel.set_history_mode(SearchHistoryMode::Suggestion);
    panel.open();
    panel.replace_query("gizli parola", true);

    panel.activate(false);

    assert!(history.entries().is_empty());
}

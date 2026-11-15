use super::support::{q, translate, FakeShell, Signal};
use crate::cancel::{CancellationToken, OperationCanceled};
use crate::clipboard::{ClipboardFilter, ClipboardItem, ClipboardKind, Guid, IClipboardHistoryService};
use crate::clock::Culture;
use crate::event::Event;
use crate::search::*;
use crate::search_source_base_methods;
use crate::shell::{OpenWindowInfo, WindowRect};
use chrono::{NaiveDate, NaiveDateTime};
use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

fn t() -> Translate {
    translate(|key| {
        if key.starts_with("SearchKw") {
            let text = match key {
                "SearchKwPlaces" => "yerler",
                "SearchKwKill" => "kill",
                "SearchKwHelp" => "yardım",
                "SearchKwBookmarks" => "yer imleri",
                "SearchKwDate" => "tarih;date",
                "SearchKwTime" => "saat;time",
                _ => key,
            };
            text.to_string()
        } else if key == "SearchWebFor" {
            format!("{key}|{{0}}|{{1}}")
        } else {
            format!("{key}|{{0}}")
        }
    })
}

fn tr() -> Culture {
    Culture::tr_tr()
}

struct FakeApps {
    apps: Vec<InstalledApp>,
    gate: Arc<Mutex<bool>>,
}

impl FakeApps {
    fn new(apps: Vec<InstalledApp>) -> Arc<FakeApps> {
        Arc::new(FakeApps { apps, gate: Arc::new(Mutex::new(false)) })
    }

    /// `Gate.SetResult()`.
    fn open_gate(&self) {
        *self.gate.lock().unwrap() = true;
    }
}

impl IInstalledAppsService for FakeApps {
    fn load(&self, cancel: &CancellationToken) -> Result<Vec<InstalledApp>, ServiceError> {
        loop {
            if cancel.is_cancellation_requested() {
                return Err(Box::new(OperationCanceled));
            }
            if *self.gate.lock().unwrap() {
                return Ok(self.apps.clone());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

#[derive(Default)]
struct FakeCommands {
    executed: Mutex<Vec<SystemCommand>>,
}

impl ISystemCommandService for FakeCommands {
    fn execute(&self, command: SystemCommand) {
        self.executed.lock().unwrap().push(command);
    }
}

struct FakeFiles {
    items: Vec<IndexedItem>,
    terms: Mutex<Vec<String>>,
}

impl FakeFiles {
    fn new(items: Vec<IndexedItem>) -> Arc<FakeFiles> {
        Arc::new(FakeFiles { items, terms: Mutex::new(Vec::new()) })
    }
}

impl IFileSearchService for FakeFiles {
    fn search(&self, term: &str, _limit: i32, _cancel: &CancellationToken) -> Result<Vec<IndexedItem>, ServiceError> {
        self.terms.lock().unwrap().push(term.to_string());
        Ok(self.items.clone())
    }
}

fn titles(results: &[SearchResult]) -> Vec<String> {
    results.iter().map(|r| r.title.clone()).collect()
}

/// `source.Changed += (_, _) => changed.TrySetResult()`.
fn watch(source: &dyn ISearchSource) -> Signal {
    let signal = Signal::new();
    let handle = signal.handle();
    source.changed().subscribe(move |_| handle.set());
    signal
}

// --- Hesap ---

#[test]
fn calculator_enter_replaces_query_and_action_copies() {
    let shell = FakeShell::new();
    let results = CalculatorSearchSource::new(shell.clone(), t(), tr()).search(&q("1000*2"));
    assert_eq!(1, results.len());
    let result = &results[0];

    assert_eq!("2.000", result.title);
    assert_eq!(Some("2000"), result.replaces_query.as_deref());
    assert_eq!(SearchRelevance::HIGH, result.category_relevance);

    (result.actions[0].execute)();
    assert_eq!(vec!["copy 2000"], shell.log());
}

#[test]
fn calculator_ignores_plain_words() {
    assert!(CalculatorSearchSource::new(FakeShell::new(), t(), tr()).search(&q("chrome")).is_empty());
}

// --- Oturum ---

fn session(service: Arc<FakeCommands>) -> SessionSearchSource {
    let entry = |command, title: &str, keywords: &[&str]| {
        SystemCommandEntry::new(command, title, keywords.iter().map(|k| k.to_string()).collect())
    };
    SessionSearchSource::new(
        service,
        vec![
            entry(SystemCommand::Lock, "Kilitle", &["kilitle", "lock"]),
            entry(SystemCommand::Restart, "Yeniden başlat", &["yeniden başlat", "restart"]),
            entry(SystemCommand::Shutdown, "Kapat", &["kapat", "shutdown"]),
            entry(SystemCommand::SignOut, "Oturumu kapat", &["oturumu kapat", "logout"]),
        ],
        t(),
    )
}

fn single(mut results: Vec<SearchResult>) -> SearchResult {
    assert_eq!(1, results.len(), "tek sonuç beklendi");
    results.remove(0)
}

#[test]
fn dangerous_session_commands_need_confirmation() {
    let source = session(Arc::new(FakeCommands::default()));

    assert!(!single(source.search(&q("kilit"))).requires_confirmation);
    assert!(single(source.search(&q("yeniden"))).requires_confirmation);
    assert!(single(source.search(&q("kapat"))).requires_confirmation);
    assert!(single(source.search(&q("logout"))).requires_confirmation);
}

#[test]
fn exact_keyword_is_highest_and_prefix_is_low() {
    let source = session(Arc::new(FakeCommands::default()));

    assert_eq!(SearchRelevance::HIGHEST, single(source.search(&q("kilitle"))).category_relevance);
    let partial = single(source.search(&q("kil")));
    assert_eq!(SearchRelevance::LOW, partial.category_relevance);
    assert!((0.9 * 3.0 / 7.0 - partial.relevance).abs() < 0.0005);
}

#[test]
fn session_keyword_must_start_with_what_is_typed() {
    assert!(session(Arc::new(FakeCommands::default())).search(&q("apat")).is_empty());
}

#[test]
fn session_runs_through_service() {
    let service = Arc::new(FakeCommands::default());

    (single(session(service.clone()).search(&q("lock"))).execute)();

    assert_eq!(vec![SystemCommand::Lock], *service.executed.lock().unwrap());
}

// --- Uygulamalar ---

#[test]
fn secondary_key_comes_from_package_or_exe() {
    let cases = [
        ("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", "microsoft.windowscalculator"),
        (r"{6D809377-6AF0-444B-8957-A3773F02200E}\Notepad++\notepad++.exe", "notepad++"),
    ];
    for (id, expected) in cases {
        assert_eq!(expected, AppSearchSource::secondary_key(id), "{id}");
    }
}

#[test]
fn app_list_filter() {
    let cases = [
        ("App", "app.exe", true),
        ("  ", "app.exe", false),
        ("Uninstall Foo", "unins.exe", false),
        ("Foo'yu Kaldır", "unins.exe", false),
        ("Benioku", r"C:\a\readme.txt", false),
        ("Oyun", r"C:\a\oyun.url", true),
    ];
    for (name, id, expected) in cases {
        assert_eq!(expected, AppSearchSource::should_list(&InstalledApp::new(id, name)), "{name} / {id}");
    }
}

#[test]
fn app_source_lists_after_loading_dedupes_and_finds_by_package() {
    let service = FakeApps::new(vec![
        InstalledApp::new("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", "Hesap Makinesi"),
        InstalledApp::new("dup1", "Çift"),
        InstalledApp::new("dup2", "çift"),
    ]);
    let launched: Arc<Mutex<Vec<InstalledApp>>> = Arc::default();
    let sink = Arc::clone(&launched);
    let source = AppSearchSource::new(service.clone(), Arc::new(move |app| sink.lock().unwrap().push(app)), t(), None);
    let changed = watch(&source);

    source.prepare();
    assert!(source.search(&q("hesap")).is_empty());

    service.open_gate();
    changed.wait(5);

    assert_eq!(1, source.search(&q("cift")).len());
    assert_eq!(SearchRelevance::HIGHEST, single(source.search(&q("hesap makinesi"))).category_relevance);
    assert_eq!(SearchCategoryPlacement::Top, single(source.search(&q("hesap makinesi"))).placement);

    (single(source.search(&q("calc"))).execute)();
    let launched = launched.lock().unwrap();
    assert_eq!(1, launched.len());
    assert_eq!("Hesap Makinesi", launched[0].name);
}

// --- Pencereler ---

#[test]
fn window_source_uses_snapshot_and_releases_it() {
    let activated: Arc<Mutex<Vec<isize>>> = Arc::default();
    let calls = Arc::new(AtomicI32::new(0));
    let counter = Arc::clone(&calls);
    let sink = Arc::clone(&activated);
    let source = WindowSearchSource::new(
        Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            vec![
                OpenWindowInfo::new(42, "Rapor - Word", Some(r"C:\Office\WINWORD.EXE".to_string()), 1, WindowRect::default()),
                OpenWindowInfo::new(43, "  ", None, 2, WindowRect::default()),
            ]
        }),
        Arc::new(move |h| sink.lock().unwrap().push(h)),
        t(),
    );

    source.prepare();
    let result = single(source.search(&q("rapor")));
    source.search(&q("rapo"));
    (result.execute)();

    assert_eq!(1, calls.load(Ordering::SeqCst));
    assert_eq!(Some("WINWORD"), result.subtitle.as_deref());
    assert_eq!(vec![42isize], *activated.lock().unwrap());

    source.release();
    assert!(source.search(&q("rapor")).is_empty());
}

// --- Komut satırı ---

fn path_shell() -> Arc<FakeShell> {
    let shell = FakeShell::new();
    shell.set_env("PATH", r"C:\Windows;C:\Tools");
    shell.set_env("PATHEXT", ".EXE;.CMD");
    shell.add_file(r"C:\Windows\ipconfig.EXE");
    shell.add_file(r"C:\Tools\build.CMD");
    shell
}

#[test]
fn shell_finds_program_on_path_and_passes_arguments() {
    let shell = path_shell();
    let result = single(ShellCommandSearchSource::new(shell.clone(), t()).search(&q("ipconfig /all")));

    assert_eq!(SearchRelevance::HIGHEST, result.category_relevance);
    (result.execute)();

    assert_eq!(vec![r"run C:\Windows\ipconfig.EXE /all"], shell.log());
}

#[test]
fn shell_run_in_terminal_keeps_original_line() {
    let shell = path_shell();

    (single(ShellCommandSearchSource::new(shell.clone(), t()).search(&q("build -x"))).actions[0].execute)();

    assert_eq!(vec!["term build -x"], shell.log());
}

#[test]
fn shell_ignores_unknown_commands() {
    assert!(ShellCommandSearchSource::new(path_shell(), t()).search(&q("merhaba dünya")).is_empty());
}

#[test]
fn shell_accepts_explicit_extension() {
    assert_eq!(1, ShellCommandSearchSource::new(path_shell(), t()).search(&q("ipconfig.exe")).len());
}

#[test]
fn shell_splits_command() {
    let cases: [(&str, &str, Option<&str>); 4] = [
        ("notepad a.txt", "notepad", Some("a.txt")),
        ("\"C:\\Program Files\\x.exe\" -v", r"C:\Program Files\x.exe", Some("-v")),
        ("tek", "tek", None),
        ("\"kapanmamış", "", None),
    ];
    for (text, command, arguments) in cases {
        assert_eq!((command.to_string(), arguments.map(str::to_string)), ShellCommandSearchSource::split_command(text), "{text}");
    }
}

// --- Konumlar ---

#[test]
fn existing_folder_opens_with_highest_importance() {
    let shell = FakeShell::new();
    shell.add_directory(r"C:\Users");

    let result = single(LocationSearchSource::new(shell.clone(), t()).search(&q(r"C:\Users")));
    (result.execute)();

    assert_eq!(SearchRelevance::HIGHEST, result.category_relevance);
    assert_eq!(vec![r"open C:\Users"], shell.log());
}

#[test]
fn environment_variables_expand_in_paths() {
    let shell = FakeShell::new();
    shell.set_env("USERPROFILE", r"C:\Users\ali");
    shell.add_directory(r"C:\Users\ali\Belgeler");

    let result = single(LocationSearchSource::new(shell.clone(), t()).search(&q(r"%USERPROFILE%\Belgeler")));
    (result.execute)();

    assert_eq!(vec![r"open C:\Users\ali\Belgeler"], shell.log());
}

#[test]
fn executable_path_is_left_to_shell_source() {
    let shell = FakeShell::new();
    shell.add_file(r"C:\a\x.exe");

    assert!(LocationSearchSource::new(shell, t()).search(&q(r"C:\a\x.exe")).is_empty());
}

#[test]
fn web_addresses_open() {
    for (text, expected) in [("https://koy.dev/a", "https://koy.dev/a"), ("www.koy.dev", "https://www.koy.dev/")] {
        let shell = FakeShell::new();

        let result = single(LocationSearchSource::new(shell.clone(), t()).search(&q(text)));
        (result.execute)();

        assert_eq!(SearchRelevance::LOW, result.category_relevance, "{text}");
        assert_eq!(vec![format!("open {expected}")], shell.log(), "{text}");
    }
}

#[test]
fn mailto_is_recognised() {
    let result = single(LocationSearchSource::new(FakeShell::new(), t()).search(&q("mailto:a@b.c")));
    assert!(result.title.contains("a@b.c"));
}

#[test]
fn other_schemes_and_plain_text_are_ignored() {
    for text in ["javascript:alert(1)", "file:///C:/x", "merhaba dünya", "ms-settings:display"] {
        assert!(LocationSearchSource::new(FakeShell::new(), t()).search(&q(text)).is_empty(), "{text}");
    }
}

// --- Yerler ---

fn places() -> PlacesSearchSource {
    let source = PlacesSearchSource::new(
        Box::new(|| vec![SearchPlace::new("Belgeler", r"C:\B", None), SearchPlace::new("Resimler", r"C:\R", None)]),
        FakeShell::new(),
        t(),
    );
    source.prepare();
    source
}

#[test]
fn place_keyword_lists_all() {
    assert_eq!(2, places().search(&q("yerler")).len());
}

#[test]
fn place_exact_beats_contains() {
    let source = places();

    assert_eq!(SearchRelevance::HIGHEST, single(source.search(&q("belgeler"))).category_relevance);
    assert_eq!(SearchRelevance::HIGH, single(source.search(&q("belge"))).category_relevance);
}

// --- Son belgeler ---

#[test]
fn recent_ranks_by_match_position_and_offers_folder() {
    let shell = FakeShell::new();
    let source = RecentDocumentsSearchSource::new(
        Box::new(|| vec![r"C:\d\rapor.docx".to_string(), r"C:\d\yillik-rapor.docx".to_string(), r"C:\d\plan.txt".to_string()]),
        shell.clone(),
        t(),
    );
    source.prepare();

    let results = source.search(&q("rapor"));

    assert_eq!(2, results.len());
    assert_eq!(SearchRelevance::HIGHEST, results[0].category_relevance); // uzantısız tam ad
    assert_eq!(SearchRelevance::LOW, results[1].category_relevance);
    for r in &results {
        assert_eq!(SearchCategoryPlacement::Bottom, r.placement);
    }

    (results[1].actions[0].execute)();
    assert_eq!(vec![r"folder C:\d\yillik-rapor.docx"], shell.log());
}

// --- Dosyalar ---

#[test]
fn file_category_comes_from_extension() {
    let cases = [
        ("a.mp3", false, "SearchCatAudio"),
        ("a.PNG", false, "SearchCatImages"),
        ("a.mkv", false, "SearchCatVideos"),
        ("a.xlsx", false, "SearchCatSpreadsheets"),
        ("a.pptx", false, "SearchCatPresentations"),
        ("a.pdf", false, "SearchCatDocuments"),
        ("a.zip", false, "SearchCatArchives"),
        ("a.txt", false, "SearchCatTexts"),
        ("a.xyz", false, "SearchCatFiles"),
        ("klasör", true, "SearchCatFolders"),
    ];
    for (path, folder, expected) in cases {
        assert_eq!(expected, FileSearchSource::category_key(path, folder), "{path}");
    }
}

fn search_files(source: &FileSearchSource, text: &str) -> Vec<SearchResult> {
    let changed = watch(source);
    assert!(source.search(&q(text)).is_empty());
    assert!(source.is_busy());
    changed.wait(5);
    assert!(!source.is_busy());
    source.search(&q(text))
}

#[test]
fn file_results_arrive_after_search_with_importance_and_folder_action() {
    let shell = FakeShell::new();
    let source = FileSearchSource::new(
        FakeFiles::new(vec![IndexedItem::new(r"C:\d\rapor.pdf", false), IndexedItem::new(r"C:\d\yillik.pdf", false)]),
        shell.clone(),
        t(),
        None,
    );

    let results = search_files(&source, "rapor");

    assert_eq!(SearchRelevance::HIGHEST, results.iter().find(|r| r.title == "rapor.pdf").unwrap().category_relevance);
    assert_eq!(SearchRelevance::LOW, results.iter().find(|r| r.title == "yillik.pdf").unwrap().category_relevance);
    for r in &results {
        assert_eq!(SearchCategoryPlacement::Bottom, r.placement);
    }

    (results[0].actions[0].execute)();
    assert!(shell.log().iter().any(|l| l.starts_with("folder ")));
}

#[test]
fn each_file_type_shows_at_most_ten() {
    let items = (0..25).map(|i| IndexedItem::new(format!(r"C:\d\f{i}.pdf"), false)).collect();
    let source = FileSearchSource::new(FakeFiles::new(items), FakeShell::new(), t(), None);

    assert_eq!(10, search_files(&source, "rapor").len());
}

#[test]
fn same_term_is_not_searched_twice() {
    let files = FakeFiles::new(vec![]);
    let source = FileSearchSource::new(files.clone(), FakeShell::new(), t(), None);

    search_files(&source, "rapor");
    source.search(&q("rapor"));

    assert_eq!(1, files.terms.lock().unwrap().len());
}

#[test]
fn file_search_survives_rapid_queries_and_release_from_other_threads() {
    let source =
        Arc::new(FileSearchSource::new(FakeFiles::new(vec![IndexedItem::new(r"C:\d\f.pdf", false)]), FakeShell::new(), t(), None));
    let worker_source = Arc::clone(&source);

    let worker = std::thread::spawn(move || {
        for i in 0..300 {
            let _ = worker_source.is_busy();
            let _ = worker_source.search(&q(&format!("rapor{}", i % 7)));
            if i % 25 == 0 {
                worker_source.release();
            }
        }
    });
    for i in 0..300 {
        let _ = source.is_busy();
        let _ = source.search(&q(&format!("belge{}", i % 5)));
    }

    worker.join().expect("arka plan iş parçacığı çöktü");
    source.release();

    assert!(!source.is_busy());
}

#[test]
fn file_search_skips_short_and_calculator_queries() {
    for text in ["a", "=1+1"] {
        let source = FileSearchSource::new(FakeFiles::new(vec![]), FakeShell::new(), t(), None);

        assert!(source.search(&q(text)).is_empty(), "{text}");
        assert!(!source.is_busy(), "{text}");
    }
}

// --- Süreçler ---

fn kill(log: &Arc<Mutex<Vec<String>>>, on_list: Option<Arc<AtomicI32>>, clock: Option<ClockFn>) -> KillSearchSource {
    let close_log = Arc::clone(log);
    let kill_log = Arc::clone(log);
    KillSearchSource::new(
        Box::new(move || {
            if let Some(c) = &on_list {
                c.fetch_add(1, Ordering::SeqCst);
            }
            vec![RunningProcess::new(7, "notepad", 111), RunningProcess::new(8, "chrome", 222)]
        }),
        Arc::new(move |p| kill_log.lock().unwrap().push(format!("kill {}", p.id))),
        Arc::new(move |p| close_log.lock().unwrap().push(format!("close {}", p.id))),
        t(),
        clock,
    )
}

#[test]
fn kill_needs_trigger_word_and_two_letters() {
    let calls = Arc::new(AtomicI32::new(0));
    let source = kill(&Arc::default(), Some(calls.clone()), None);

    assert!(source.search(&q("notepad")).is_empty());
    assert!(source.search(&q("kill n")).is_empty());
    assert_eq!(0, calls.load(Ordering::SeqCst));
}

#[test]
fn kill_lists_matches_and_snapshots_once() {
    let calls = Arc::new(AtomicI32::new(0));
    let source = kill(&Arc::default(), Some(calls.clone()), None);

    single(source.search(&q("kill note")));
    source.search(&q("kill notep"));

    assert_eq!(1, calls.load(Ordering::SeqCst));
}

#[test]
fn kill_primary_action_is_graceful_close_and_terminate_needs_confirmation() {
    let log: Arc<Mutex<Vec<String>>> = Arc::default();
    let result = single(kill(&log, None, None).search(&q("kill note")));

    assert!(!result.requires_confirmation);
    assert!(result.title.contains("SearchCloseProcess"));
    assert_eq!(1, result.actions.len());
    assert!(result.actions[0].requires_confirmation);

    (result.execute)();
    (result.actions[0].execute)();

    assert_eq!(vec!["close 7", "kill 7"], *log.lock().unwrap());
}

#[test]
fn kill_terminate_asks_before_running_in_the_panel() {
    let log: Arc<Mutex<Vec<String>>> = Arc::default();
    let source: Arc<dyn ISearchSource> = Arc::new(kill(&log, None, None));
    let panel = SearchPanelController::new(vec![source], None, None, None, "", "");
    panel.open();
    panel.set_query(Some("kill note"));

    assert_eq!(SearchActivation::NeedsConfirmation, panel.activate(true).outcome);
    assert!(log.lock().unwrap().is_empty());
    assert_eq!(SearchActivation::Executed, panel.activate(true).outcome);
    assert_eq!(vec!["kill 7"], *log.lock().unwrap());
}

#[test]
fn kill_snapshot_is_refreshed_when_stale() {
    let calls = Arc::new(AtomicI32::new(0));
    let now = Arc::new(AtomicI64::new(0));
    let clock_now = Arc::clone(&now);
    let source = kill(&Arc::default(), Some(calls.clone()), Some(Box::new(move || clock_now.load(Ordering::SeqCst))));

    source.search(&q("kill note"));
    now.store(KillSearchSource::MAX_SNAPSHOT_AGE_MS, Ordering::SeqCst);
    source.search(&q("kill note"));
    assert_eq!(1, calls.load(Ordering::SeqCst));

    now.store(KillSearchSource::MAX_SNAPSHOT_AGE_MS + 1, Ordering::SeqCst);
    source.search(&q("kill note"));
    assert_eq!(2, calls.load(Ordering::SeqCst));
}

#[test]
fn kill_actions_carry_the_listed_process_identity() {
    let closed: Arc<Mutex<Option<RunningProcess>>> = Arc::default();
    let sink = Arc::clone(&closed);
    let source = KillSearchSource::new(
        Box::new(|| vec![RunningProcess::new(7, "notepad", 111)]),
        Arc::new(|_| {}),
        Arc::new(move |p| *sink.lock().unwrap() = Some(p)),
        t(),
        None,
    );

    (single(source.search(&q("kill note"))).execute)();

    assert_eq!(Some(RunningProcess::new(7, "notepad", 111)), *closed.lock().unwrap());
}

// --- Web ---

#[test]
fn web_shortcut_parses() {
    for (text, key, term) in [("gg:kedi", "gg", "kedi"), ("!yt kedi videosu", "yt", "kedi videosu")] {
        let (k, term_parsed) = WebShortcutSearchSource::try_parse(text).unwrap_or_else(|| panic!("{text}"));
        assert_eq!((key, term), (k.as_str(), term_parsed.as_str()), "{text}");
    }
}

#[test]
fn web_shortcut_ignores_other_text() {
    for text in [r"C:\klasör", "https://a.b", "düz metin", "gg:", "!gg"] {
        assert!(WebShortcutSearchSource::try_parse(text).is_none(), "{text}");
    }
}

#[test]
fn web_shortcut_opens_escaped_url() {
    let shell = FakeShell::new();

    (single(WebShortcutSearchSource::new(WebShortcutSearchSource::defaults(), shell.clone(), t(), None).search(&q("gg:kedi köpek"))).execute)();

    assert_eq!(vec!["open https://www.google.com/search?q=kedi%20k%C3%B6pek"], shell.log());
}

#[test]
fn unknown_web_key_gives_nothing() {
    assert!(
        WebShortcutSearchSource::new(WebShortcutSearchSource::defaults(), FakeShell::new(), t(), None).search(&q("zz:kedi")).is_empty()
    );
}

#[test]
fn web_fallback_searches_in_default_chromium_browser_with_its_own_engine() {
    let shell = FakeShell::new();
    let chrome = DefaultBrowserSearch::from_open_command(Some(r#""C:\Chrome\chrome.exe" --single-argument %1"#));

    let row = WebShortcutSearchSource::new(
        WebShortcutSearchSource::defaults(),
        shell.clone(),
        t(),
        Some(Box::new(move || chrome.clone())),
    )
    .fallback(&q("kedi"))
    .unwrap();
    (row.execute)();

    assert_eq!(vec![r"run C:\Chrome\chrome.exe [--single-argument|? kedi]"], shell.log());
}

#[test]
fn web_fallback_uses_duckduckgo_when_browser_is_unknown() {
    let shell = FakeShell::new();

    let source = WebShortcutSearchSource::new(WebShortcutSearchSource::defaults(), shell.clone(), t(), Some(Box::new(|| None)));
    (source.fallback(&q("kedi")).unwrap().execute)();

    assert_eq!(vec!["open https://duckduckgo.com/?q=kedi"], shell.log());
}

#[test]
fn web_fallback_uses_duckduckgo() {
    let shell = FakeShell::new();

    (WebShortcutSearchSource::new(WebShortcutSearchSource::defaults(), shell.clone(), t(), None).fallback(&q("kedi")).unwrap().execute)();

    assert_eq!(vec!["open https://duckduckgo.com/?q=kedi"], shell.log());
}

// --- Yer imleri ---

const BOOKMARKS_JSON: &str = r#"
{"roots":{"bookmark_bar":{"type":"folder","children":[
  {"type":"url","name":"Koy Sitesi","url":"https://koy.dev"},
  {"type":"folder","name":"İç","children":[{"type":"url","name":"Haber","url":"http://haber.example"}]},
  {"type":"url","name":"Yerel","url":"chrome://settings"}
]},"other":{"type":"folder","children":[]}}}
"#;

#[test]
fn chromium_bookmarks_are_collected_recursively_and_only_web_links() {
    let list = ChromiumBookmarks::parse(BOOKMARKS_JSON);

    assert_eq!(vec!["Koy Sitesi", "Haber"], list.iter().map(|b| b.title.as_str()).collect::<Vec<_>>());
}

#[test]
fn broken_bookmark_files_give_nothing() {
    for json in ["", "bozuk {", "[]", r#"{"roots":5}"#] {
        assert!(ChromiumBookmarks::parse(json).is_empty(), "{json:?}");
    }
}

#[test]
fn bookmark_source_loads_in_background_and_lists_all_with_keyword() {
    let shell = FakeShell::new();
    let source = BookmarksSearchSource::new(Arc::new(|| ChromiumBookmarks::parse(BOOKMARKS_JSON)), shell.clone(), t());
    let changed = watch(&source);

    source.prepare();
    changed.wait(5);

    assert_eq!(2, source.search(&q("yer imleri")).len());
    assert!(source.search(&q("ko")).is_empty());
    (single(source.search(&q("koy"))).execute)();
    assert_eq!(vec!["open https://koy.dev"], shell.log());
}

// --- Tarih, pano, yardım ---

fn date_time(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
}

#[test]
fn date_and_time_keywords() {
    let shell = FakeShell::new();
    let source = DateTimeSearchSource::new(shell.clone(), t(), Some(Box::new(|| date_time(2026, 11, 8, 14, 5))), tr());

    let date = single(source.search(&q("tarih")));
    (date.execute)();

    assert!(date.title.contains("2026"));
    assert_eq!(vec![format!("copy {}", date.title)], shell.log());
    assert!(single(source.search(&q("saat"))).title.contains("14:05"));
    assert!(source.search(&q("tari")).is_empty());
}

struct FakeClipboard {
    items: Vec<ClipboardItem>,
    copied: Mutex<Vec<Guid>>,
    changed: Event<()>,
}

impl FakeClipboard {
    fn new(items: Vec<ClipboardItem>) -> Arc<FakeClipboard> {
        Arc::new(FakeClipboard { items, copied: Mutex::new(Vec::new()), changed: Event::new() })
    }
}

impl IClipboardHistoryService for FakeClipboard {
    fn is_running(&self) -> bool {
        true
    }

    fn start(&self) {}

    fn stop(&self) {}

    fn changed(&self) -> &Event<()> {
        &self.changed
    }

    fn query(&self, _filter: ClipboardFilter, _search: Option<&str>) -> Vec<ClipboardItem> {
        self.items.clone()
    }

    fn set_pinned(&self, _id: Guid, _pinned: bool) -> bool {
        true
    }

    fn remove(&self, _id: Guid) -> bool {
        true
    }

    fn clear(&self) {}

    fn copy_back(&self, id: Guid) -> bool {
        self.copied.lock().unwrap().push(id);
        true
    }
}

fn clip(text: &str) -> ClipboardItem {
    ClipboardItem::new(Guid::new_guid(), text, ClipboardKind::Text, false, chrono::Local::now().fixed_offset())
}

#[test]
fn clipboard_needs_three_letters_and_is_capped() {
    let source = ClipboardSearchSource::new(FakeClipboard::new(vec![clip("a"), clip("b"), clip("c"), clip("d")]), t());

    assert!(source.search(&q("ab")).is_empty());
    assert_eq!(3, source.search(&q("abc")).len());
}

#[test]
fn clipboard_result_copies_back_and_flattens_long_text() {
    let item = clip(&format!("satır bir\nsatır iki {}", "x".repeat(200)));
    let service = FakeClipboard::new(vec![item.clone()]);

    let result = single(ClipboardSearchSource::new(service.clone(), t()).search(&q("satır")));
    (result.execute)();

    assert!(!result.title.contains('\n'));
    assert!(result.title.encode_utf16().count() <= 81);
    assert!(result.restores_focus);
    assert_eq!(vec![item.id], *service.copied.lock().unwrap());
}

/// C# testindeki `WithSyntax`: yalnız sözdizimleri olan boş kaynak.
struct WithSyntax {
    base: SearchSourceBase,
}

impl WithSyntax {
    fn new(id: &str, name: &str, syntaxes: Vec<SearchSyntax>) -> Arc<dyn ISearchSource> {
        let mut base = SearchSourceBase::new(id, name);
        base.syntaxes = syntaxes;
        Arc::new(WithSyntax { base })
    }
}

impl ISearchSource for WithSyntax {
    search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, _query: &SearchQuery) -> Vec<SearchResult> {
        Vec::new()
    }
}

/// `help = new HelpSearchSource(() => [help!, .. sources], T)`.
fn help(sources: Vec<Arc<dyn ISearchSource>>) -> Arc<HelpSearchSource> {
    let slot: Arc<Mutex<Option<Arc<HelpSearchSource>>>> = Arc::default();
    let slot_for_list = Arc::clone(&slot);
    let help = Arc::new(HelpSearchSource::new(
        Box::new(move || {
            let mut list: Vec<Arc<dyn ISearchSource>> = vec![slot_for_list.lock().unwrap().clone().unwrap() as Arc<dyn ISearchSource>];
            list.extend(sources.iter().cloned());
            list
        }),
        t(),
    ));
    *slot.lock().unwrap() = Some(Arc::clone(&help));
    help
}

#[test]
fn help_lists_documented_sources_and_row_expands_to_details() {
    let help = help(vec![
        WithSyntax::new("web", "Web", vec![SearchSyntax::new("gg:ara", "Google")]),
        WithSyntax::new("hidden", "Gizli", vec![]),
    ]);

    let row = single(help.search(&q("?")));

    assert_eq!("gg:ara", row.title);
    assert_eq!(Some("?Web"), row.replaces_query.as_deref());
    assert!(row.skip_history);
    assert!(help.search(&q("web")).is_empty());
}

#[test]
fn help_keyword_works_like_question_mark() {
    let help = help(vec![WithSyntax::new("a", "A", vec![SearchSyntax::new("x", "y")])]);

    assert_eq!(1, help.search(&q("yardım")).len());
}

#[test]
fn help_shows_every_example_for_a_named_source_in_order() {
    let help = help(vec![WithSyntax::new(
        "web",
        "Web",
        vec![SearchSyntax::new("gg:ara", "Google"), SearchSyntax::new("yt:ara", "YouTube")],
    )]);

    let rows = help.search(&q("?web"));

    assert_eq!(vec!["gg:ara", "yt:ara"], titles(&rows));
    assert!(rows[0].relevance > rows[1].relevance);
    assert_eq!(Some("yt:ara"), rows[1].replaces_query.as_deref());
}

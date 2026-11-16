//! Arama panelini ayarlara göre kurar; kapatılan kaynaklar hiç oluşturulmaz.
//! Karşılığı: `Features/Search/SearchFactory.cs` (SearchGlyphConverter: `glyph_path`).

use std::sync::Arc;

use koy_core::clipboard::IClipboardHistoryService;
use koy_core::search::{
    AppSearchSource, BookmarksSearchSource, CalculatorSearchSource, ClipboardSearchSource, ColorConverterSource, DateTimeSearchSource,
    DictionarySource, FileSearchSource, HelpSearchSource, ISearchShell, ISearchSource, InstalledApp, JumpListSearchSource,
    KillSearchSource, LocationSearchSource, MicrosoftStoreSearchSource, PlacesSearchSource, RecentDocumentsSearchSource, SearchGlyph,
    SearchHistory, SearchPanelController, SearchSyntax, SessionSearchSource, ShellCommandSearchSource, SpecialCharactersSource,
    SpellCheckSource, SystemCommand, SystemCommandEntry, Translate, UnitConverterSource, WebShortcutSearchSource, WindowSearchSource,
};
use koy_core::settings::{AppSettings, in_settings_folder};
use koy_core::shell::{IShellActionService, IWindowTracker};
use koy_platform::jump_list::JumpListService;
use koy_platform::search::{
    DefaultBrowserLocator, FileSearchService, HttpDictionaryService, InstalledAppsService, SearchBookmarks, SearchPlaces, SearchProcesses,
    SearchShell, SystemCommandService, WindowsSpellCheckerService,
};
use koy_platform::shell::{IconHelper, RecentFilesService};

use super::search_view_model::SearchViewModel;
use crate::infra::clipboard_text;
use crate::infra::dispatch;
use crate::resources::strings::{self, Strings};

const MAX_RECENT_DOCUMENTS: i32 = 100;

fn t(key: &str) -> String {
    Strings::get(key)
}

fn translate() -> Translate {
    Arc::new(|k: &str| Strings::get(k))
}

fn keywords(key: &str) -> Vec<String> {
    t(key).split(';').filter(|s| !s.is_empty()).map(str::to_string).collect()
}

fn first_keyword(key: &str) -> String {
    keywords(key).into_iter().next().unwrap_or_default()
}

fn syntax(example: impl Into<String>, description_key: &str) -> SearchSyntax {
    SearchSyntax::new(example, t(description_key))
}

/// Ayarlar sayfasındaki anahtarlar için: kimlik ve ad anahtarı.
pub const SOURCE_CATALOG: &[(&str, &str)] = &[
    ("apps", "SearchCatApps"),
    ("jumplist", "SearchCatJumpList"),
    ("windows", "SearchCatWindows"),
    ("calculator", "SearchCatCalculator"),
    ("units", "SearchCatUnits"),
    ("color", "SearchCatColor"),
    ("chars", "SearchCatSpecialChars"),
    ("spell", "SearchCatSpellCheck"),
    ("dictionary", "SearchCatDictionary"),
    ("session", "SearchCatSession"),
    ("shell", "SearchCatShell"),
    ("locations", "SearchCatLocations"),
    ("places", "SearchCatPlaces"),
    ("recent", "SearchCatRecent"),
    ("files", "SearchCatFiles"),
    ("web", "SearchCatWeb"),
    ("store", "SearchCatStore"),
    ("bookmarks", "SearchCatBookmarks"),
    ("kill", "SearchCatProcesses"),
    ("clipboard", "SearchCatClipboard"),
    ("datetime", "SearchCatDateTime"),
    ("help", "SearchCatHelp"),
];

fn app_icon(app: &InstalledApp) -> Option<Vec<u8>> {
    IconHelper::get_icon_bytes(&InstalledAppsService::shell_path(app))
}

/// Uygulama listesi bütün ekranların araması için tek kopya.
pub fn create_app_source(shell_actions: Arc<dyn IShellActionService>) -> Arc<AppSearchSource> {
    let mut source = AppSearchSource::new(
        Arc::new(InstalledAppsService),
        Arc::new(move |app: InstalledApp| {
            shell_actions.launch(&InstalledAppsService::shell_path(&app), None);
        }),
        translate(),
        Some(Arc::new(app_icon)),
    );
    source.base.syntaxes = vec![syntax(t("SearchExampleApp"), "SearchSyntaxApps")];
    Arc::new(source)
}

fn search_history() -> Arc<SearchHistory> {
    Arc::new(SearchHistory::new(in_settings_folder("search-history.txt").to_string_lossy().into_owned()))
}

/// Kayıtlı arama geçmişini siler (Ayarlar).
pub fn clear_history() {
    search_history().clear();
}

fn session_entries() -> Vec<SystemCommandEntry> {
    vec![
        SystemCommandEntry::new(SystemCommand::Lock, t("SearchCmdLock"), keywords("SearchKwLock")),
        SystemCommandEntry::new(SystemCommand::Sleep, t("SearchCmdSleep"), keywords("SearchKwSleep")),
        SystemCommandEntry::new(SystemCommand::SignOut, t("SearchCmdSignOut"), keywords("SearchKwSignOut")),
        SystemCommandEntry::new(SystemCommand::SwitchUser, t("SearchCmdSwitchUser"), keywords("SearchKwSwitchUser")),
        SystemCommandEntry::new(SystemCommand::Restart, t("SearchCmdRestart"), keywords("SearchKwRestart")),
        SystemCommandEntry::new(SystemCommand::Shutdown, t("SearchCmdShutdown"), keywords("SearchKwShutdown")),
    ]
}

pub fn create(
    settings: &AppSettings,
    apps: Arc<AppSearchSource>,
    windows: Arc<dyn IWindowTracker>,
    clipboard: Option<Arc<dyn IClipboardHistoryService>>,
    save_pinned: Box<dyn Fn(bool)>,
) -> std::rc::Rc<SearchViewModel> {
    let shell: Arc<dyn ISearchShell> = Arc::new(SearchShell::new(|text: &str| {
        // Pano o anda başka bir uygulamada kilitli olabilir: sessizce geçilir.
        let _ = clipboard_text::set_text(text);
    }));
    let culture = strings::culture();
    let disabled: Vec<String> = settings.search.disabled_sources.iter().cloned().collect();
    let mut sources: Vec<Arc<dyn ISearchSource>> = Vec::new();
    let mut add = |s: Arc<dyn ISearchSource>| {
        if !disabled.iter().any(|d| d == s.id()) {
            sources.push(s);
        }
    };

    add(apps);

    let mut s = JumpListSearchSource::new(Arc::new(InstalledAppsService), Arc::new(JumpListService::default()), translate(), Some(Arc::new(app_icon)));
    s.base.syntaxes = vec![syntax(t("SearchExampleApp"), "SearchSyntaxJumpList")];
    add(Arc::new(s));

    let w1 = Arc::clone(&windows);
    let w2 = Arc::clone(&windows);
    let mut s = WindowSearchSource::new(Box::new(move || w1.get_open_windows()), Arc::new(move |h| w2.activate_window(h)), translate());
    s.base.syntaxes = vec![syntax(t("SearchExampleWindow"), "SearchSyntaxWindows")];
    add(Arc::new(s));

    let mut s = CalculatorSearchSource::new(Arc::clone(&shell), translate(), culture.clone());
    s.base.syntaxes =
        vec![syntax("12*8", "SearchSyntaxCalc"), syntax("sqrt(16)+1", "SearchSyntaxCalcFunctions"), syntax("hex=255", "SearchSyntaxCalcBase")];
    add(Arc::new(s));

    let mut s = UnitConverterSource::new(Arc::clone(&shell), translate(), culture.clone());
    s.base.syntaxes =
        vec![syntax("10 km to mi", "SearchSyntaxUnits"), syntax("100 c in f", "SearchSyntaxUnits"), syntax("1 gb in mb", "SearchSyntaxUnitsData")];
    add(Arc::new(s));

    let mut s = ColorConverterSource::new(Arc::clone(&shell), translate());
    s.base.syntaxes =
        vec![syntax("#FF8800", "SearchSyntaxColor"), syntax("rgb(255, 136, 0)", "SearchSyntaxColor"), syntax("hsl(32, 100%, 50%)", "SearchSyntaxColor")];
    add(Arc::new(s));

    let mut s = SpecialCharactersSource::new(Arc::clone(&shell), translate(), culture.clone());
    s.base.syntaxes = vec![syntax(first_keyword("SearchKwSpecialChars") + " arrow", "SearchSyntaxSpecialChars")];
    add(Arc::new(s));

    let mut s = SpellCheckSource::new(Arc::new(WindowsSpellCheckerService::default()), Arc::clone(&shell), translate());
    s.base.syntaxes = vec![syntax(first_keyword("SearchKwSpellCheck") + " recieve", "SearchSyntaxSpellCheck")];
    add(Arc::new(s));

    let mut s = DictionarySource::new(Arc::new(HttpDictionaryService), Arc::clone(&shell), translate(), culture.clone());
    s.base.syntaxes = vec![syntax(first_keyword("SearchKwDictionary") + " serendipity", "SearchSyntaxDictionary")];
    add(Arc::new(s));

    let mut s = SessionSearchSource::new(Arc::new(SystemCommandService), session_entries(), translate());
    s.base.syntaxes = vec![syntax(first_keyword("SearchKwLock"), "SearchSyntaxSession")];
    add(Arc::new(s));

    let mut s = ShellCommandSearchSource::new(Arc::clone(&shell), translate());
    s.base.syntaxes = vec![syntax("ipconfig /all", "SearchSyntaxShell")];
    add(Arc::new(s));

    let mut s = LocationSearchSource::new(Arc::clone(&shell), translate());
    s.base.syntaxes = vec![syntax(r"C:\Windows", "SearchSyntaxLocations"), syntax("https://github.com", "SearchSyntaxLocationsUrl")];
    add(Arc::new(s));

    let mut s = PlacesSearchSource::new(Box::new(|| SearchPlaces::load(|k| Strings::get(k))), Arc::clone(&shell), translate());
    s.base.syntaxes = vec![syntax(t("SearchKwPlaces"), "SearchSyntaxPlaces")];
    add(Arc::new(s));

    let mut s = RecentDocumentsSearchSource::new(
        Box::new(|| {
            use koy_core::dock::IRecentFilesService;
            RecentFilesService.get_recent(MAX_RECENT_DOCUMENTS).into_iter().map(|f| f.open_path).collect()
        }),
        Arc::clone(&shell),
        translate(),
    );
    s.base.syntaxes = vec![syntax(t("SearchExampleDocument"), "SearchSyntaxRecent")];
    add(Arc::new(s));

    let mut s = FileSearchSource::new(Arc::new(FileSearchService), Arc::clone(&shell), translate(), None);
    s.base.syntaxes = vec![syntax(t("SearchExampleDocument"), "SearchSyntaxFiles")];
    add(Arc::new(s));

    let mut web = WebShortcutSearchSource::new(
        WebShortcutSearchSource::defaults(),
        Arc::clone(&shell),
        translate(),
        Some(Box::new(DefaultBrowserLocator::find)),
    );
    let mut web_syntaxes: Vec<SearchSyntax> = WebShortcutSearchSource::defaults()
        .iter()
        .map(|s| SearchSyntax::new(format!("{}:{}", s.key, t("SearchExampleWeb")), Strings::format("SearchSyntaxWeb", &[&s.provider])))
        .collect();
    web_syntaxes.push(SearchSyntax::new(format!("!gg {}", t("SearchExampleWeb")), t("SearchSyntaxWebBang")));
    web.base.syntaxes = web_syntaxes;
    let web = Arc::new(web);
    let web_enabled = !disabled.iter().any(|d| d == "web");
    add(Arc::clone(&web) as Arc<dyn ISearchSource>);

    let mut s = MicrosoftStoreSearchSource::new(Arc::clone(&shell), translate());
    s.base.syntaxes = vec![syntax(first_keyword("SearchKwStore") + " terminal", "SearchSyntaxStore")];
    add(Arc::new(s));

    let mut s = BookmarksSearchSource::new(Arc::new(SearchBookmarks::load), Arc::clone(&shell), translate());
    s.base.syntaxes = vec![syntax(t("SearchKwBookmarks"), "SearchSyntaxBookmarks")];
    add(Arc::new(s));

    let w3 = Arc::clone(&windows);
    let mut s = KillSearchSource::new(
        Box::new(SearchProcesses::list),
        Arc::new(move |p| SearchProcesses::terminate(&p, |id| w3.terminate_process(id))),
        Arc::new(|p| SearchProcesses::request_close(&p)),
        translate(),
        None,
    );
    s.base.syntaxes = vec![syntax(t("SearchKwKill") + " notepad", "SearchSyntaxKill")];
    add(Arc::new(s));

    if let Some(c) = clipboard {
        let mut s = ClipboardSearchSource::new(c, translate());
        s.base.syntaxes = vec![syntax(t("SearchExampleClipboard"), "SearchSyntaxClipboard")];
        add(Arc::new(s));
    }

    let mut s = DateTimeSearchSource::new(Arc::clone(&shell), translate(), None, culture);
    s.base.syntaxes = vec![syntax(first_keyword("SearchKwDate"), "SearchSyntaxDate"), syntax(first_keyword("SearchKwTime"), "SearchSyntaxTime")];
    add(Arc::new(s));

    // Yardım kaynağı bütün kaynakları listeler: kendisi de dahil, kurulduktan sonra okunur.
    let all: Arc<std::sync::Mutex<Vec<Arc<dyn ISearchSource>>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let all_for_help = Arc::clone(&all);
    add(Arc::new(HelpSearchSource::new(
        Box::new(move || all_for_help.lock().map(|v| v.clone()).unwrap_or_default()),
        translate(),
    )));
    *all.lock().unwrap_or_else(|e| e.into_inner()) = sources.clone();

    let fallback: Option<koy_core::search::FallbackFn> = web_enabled.then(|| {
        let web = Arc::clone(&web);
        Arc::new(move |q: &koy_core::search::SearchQuery| web.fallback(q)) as koy_core::search::FallbackFn
    });
    let post = dispatch::post_fn();
    let panel = SearchPanelController::new(
        sources,
        Some(Arc::new(move |action: Box<dyn FnOnce() + Send>| post(action))),
        Some(search_history()),
        fallback,
        &t("SearchRecentQueries"),
        &t("SearchActRemove"),
    );
    panel.set_retain_prior_search(settings.search.retain_prior);
    panel.set_history_mode(settings.search.history_mode);
    SearchViewModel::new(panel, settings.search.pinned, save_pinned)
}

/// Satır simgesi yoksa kullanılan çizgi ikon (24×24 yol). `None`: genel simgeler (`Icons` global'i) kullanılır.
pub fn glyph_path(glyph: SearchGlyph) -> Option<&'static str> {
    Some(match glyph {
        SearchGlyph::Calculator => "M6 2h12a2 2 0 0 1 2 2v16a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2v-16a2 2 0 0 1 2 -2zM8 6h8M16 14v4M16 10h.01M12 10h.01M8 10h.01M12 14h.01M8 14h.01M12 18h.01M8 18h.01",
        SearchGlyph::Terminal => "M4 17l6 -6l-6 -6M12 19h8",
        SearchGlyph::Folder => "M20 20a2 2 0 0 0 2 -2v-10a2 2 0 0 0 -2 -2h-8l-2 -2h-6a2 2 0 0 0 -2 2v12a2 2 0 0 0 2 2z",
        SearchGlyph::File => "M15 2h-9a2 2 0 0 0 -2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2 -2v-13zM14 2v4a2 2 0 0 0 2 2h4",
        SearchGlyph::Globe => "M22 12a10 10 0 1 1 -20 0a10 10 0 0 1 20 0zM2 12h20M12 2a15.3 15.3 0 0 1 4 10a15.3 15.3 0 0 1 -4 10a15.3 15.3 0 0 1 -4 -10a15.3 15.3 0 0 1 4 -10z",
        SearchGlyph::Bookmark => "M19 21l-7 -5l-7 5v-16a2 2 0 0 1 2 -2h10a2 2 0 0 1 2 2z",
        SearchGlyph::Clipboard => "M9 2h6a1 1 0 0 1 1 1v2a1 1 0 0 1 -1 1h-6a1 1 0 0 1 -1 -1v-2a1 1 0 0 1 1 -1zM16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2v-14a2 2 0 0 1 2 -2h2",
        SearchGlyph::Clock => "M22 12a10 10 0 1 1 -20 0a10 10 0 0 1 20 0zM12 6v6l4 2",
        SearchGlyph::Help => "M22 12a10 10 0 1 1 -20 0a10 10 0 0 1 20 0zM9.09 9a3 3 0 0 1 5.83 1c0 2 -3 3 -3 3M12 17h.01",
        SearchGlyph::History => "M3 12a9 9 0 1 0 9 -9a9.75 9.75 0 0 0 -6.74 2.74l-2.26 2.26M3 3v5h5M12 7v5l4 2",
        SearchGlyph::Symbol => "M12 2l2.4 7.4h7.6l-6 4.6l2.3 7l-6.3 -4.6l-6.3 4.6l2.3 -7l-6 -4.6h7.6z",
        SearchGlyph::SpellCheck => "M6 16l6 -12l6 12M8 12h8M16 20l2 2l4 -4",
        SearchGlyph::Dictionary => "M4 19.5v-15a2.5 2.5 0 0 1 2.5 -2.5h13.5v19h-13.5a2.5 2.5 0 0 1 0 -5h13.5",
        SearchGlyph::Store => "M6 7h12l1 13h-14zM9 7v-1a3 3 0 0 1 6 0v1",
        _ => return None,
    })
}

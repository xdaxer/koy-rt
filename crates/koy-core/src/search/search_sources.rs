use super::search_util::{take_utf16, utf16_len};
use super::{
    is_canceled, log_service_error, ExpressionCalculator, IInstalledAppsService, ISearchShell, ISearchSource,
    ISystemCommandService, InstalledApp, SearchAction, SearchCategoryPlacement, SearchDragData, SearchDragKind, SearchGlyph,
    SearchMatcher, SearchQuery, SearchRelevance, SearchResult, SearchSourceBase, SearchText, SystemCommand, Translate,
};
use crate::cancel::CancellationTokenSource;
use crate::clipboard::{ClipboardFilter, IClipboardHistoryService};
use crate::clock::{format_date_time, Culture};
use crate::event::Event;
use crate::shell::OpenWindowInfo;
use crate::text;
use chrono::NaiveDateTime;
use std::collections::HashSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

/// `Func<InstalledApp, byte[]?>`.
pub type AppIconFn = Arc<dyn Fn(&InstalledApp) -> Option<Vec<u8>> + Send + Sync>;
/// `Action<InstalledApp>`.
pub type LaunchAppFn = Arc<dyn Fn(InstalledApp) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct AppEntry {
    pub(crate) app: InstalledApp,
    pub(crate) name: String,
    pub(crate) key: String,
}

struct AppInner {
    service: Arc<dyn IInstalledAppsService>,
    changed: Arc<Event<()>>,
    entries: Mutex<Vec<AppEntry>>,
}

/// Kurulu uygulamalar. Liste ~1 sn sürdüğü için panel açılınca arka planda tazelenir, eski kopya hemen kullanılır.
pub struct AppSearchSource {
    pub base: SearchSourceBase,
    launch: LaunchAppFn,
    load_icon: Option<AppIconFn>,
    inner: Arc<AppInner>,
    /// `_loading`: panel açıkken yükleme belirteci.
    loading: Mutex<Option<CancellationTokenSource>>,
}

const SECONDARY_KEY_WEIGHT: f64 = 0.6;
const IGNORED_EXTENSIONS: [&str; 6] = [".txt", ".pdf", ".htm", ".html", ".chm", ".rtf"];

impl AppSearchSource {
    pub fn new(service: Arc<dyn IInstalledAppsService>, launch: LaunchAppFn, t: Translate, load_icon: Option<AppIconFn>) -> Self {
        let name = t("SearchCatApps");
        let base = SearchSourceBase::new("apps", name);
        let inner = Arc::new(AppInner { service, changed: base.changed_handle(), entries: Mutex::new(Vec::new()) });
        AppSearchSource { base, launch, load_icon, inner, loading: Mutex::new(None) }
    }

    /// Listeye girecek mi: belge kısayolları ve kaldırıcılar uygulama değildir.
    pub fn should_list(app: &InstalledApp) -> bool {
        if text::is_null_or_white_space(Some(&app.name)) {
            return false;
        }

        let name = SearchText::fold_str(&app.name);
        if name.contains("uninstall") || name.contains("kaldir") {
            return false;
        }

        !IGNORED_EXTENSIONS.iter().any(|e| text::ends_with_ignore_case(&app.id, e))
    }

    /// İkinci arama anahtarı: Türkçe Windows'ta ad "Hesap Makinesi" olsa da "calc" yazan bulsun. Paketli uygulamada
    /// paket adı, klasik uygulamada exe adı.
    pub fn secondary_key(id: &str) -> String {
        if let Some(bang) = id.find('!').filter(|&b| b > 0) {
            let package = &id[..bang];
            return match package.find('_').filter(|&u| u > 0) {
                Some(underscore) => SearchText::fold_str(&package[..underscore]),
                None => SearchText::fold_str(package),
            };
        }

        let start = id.rfind(['\\', '/']).map_or(0, |i| i + 1);
        let name = &id[start..];
        match name.rfind('.').filter(|&d| d > 0) {
            Some(dot) => SearchText::fold_str(&name[..dot]),
            None => SearchText::fold_str(name),
        }
    }

    /// C# `LoadAsync`.
    fn load(inner: Arc<AppInner>, cts: CancellationTokenSource) {
        let token = cts.token();
        let outcome = catch_unwind(AssertUnwindSafe(|| inner.service.load(&token)));
        match outcome {
            Ok(Ok(apps)) => {
                if cts.is_cancellation_requested() {
                    return;
                }

                *inner.entries.lock().unwrap() = Self::build_entries(apps);
                inner.changed.invoke(&());
            }
            // Panel kapandı; önceki liste kalır.
            Ok(Err(e)) if is_canceled(&e) => {}
            Ok(Err(e)) => log_service_error("Uygulama listesi", &e),
            Err(payload) => log_service_error("Uygulama listesi", &super::search_util::panic_message(payload.as_ref()).into()),
        }
    }

    /// `Where(ShouldList).DistinctBy(Fold(Name)).Select(...)`.
    pub(crate) fn build_entries(apps: Vec<InstalledApp>) -> Vec<AppEntry> {
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        for app in apps.into_iter().filter(Self::should_list) {
            let folded = SearchText::fold_str(&app.name);
            if !seen.insert(folded.clone()) {
                continue;
            }
            let key = Self::secondary_key(&app.id);
            entries.push(AppEntry { app, name: folded, key });
        }
        entries
    }
}

impl Drop for AppSearchSource {
    fn drop(&mut self) {
        ISearchSource::release(self);
    }
}

impl ISearchSource for AppSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {
        let mut loading = self.loading.lock().unwrap();
        if loading.is_some() {
            return;
        }

        let cts = CancellationTokenSource::new();
        *loading = Some(cts.clone());
        let inner = Arc::clone(&self.inner);
        std::thread::spawn(move || Self::load(inner, cts));
    }

    fn release(&self) {
        if let Some(cts) = self.loading.lock().unwrap().take() {
            cts.cancel();
        }
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let entries = self.inner.entries.lock().unwrap().clone();
        let mut results = Vec::new();
        for entry in &entries {
            let score = f64::max(SearchMatcher::score(query, &entry.name), SearchMatcher::score(query, &entry.key) * SECONDARY_KEY_WEIGHT);
            if score <= 0.0 {
                continue;
            }

            let app = entry.app.clone();
            let launch = Arc::clone(&self.launch);
            let launching = app.clone();
            let mut result = SearchResult::new(format!("app:{}", app.id), self.base.name(), app.name.clone(), move || launch(launching.clone()));
            result.glyph = SearchGlyph::App;
            result.relevance = score;
            result.placement = SearchCategoryPlacement::Top;
            result.category_relevance = if entry.name == query.folded { SearchRelevance::HIGHEST } else { SearchRelevance::MODERATE };
            result.load_icon = self.load_icon.clone().map(|f| {
                let app = app.clone();
                Arc::new(move || f(&app)) as Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>
            });
            results.push(result);
        }
        results
    }
}

#[derive(Clone)]
struct WindowEntry {
    window: OpenWindowInfo,
    title: String,
    process: String,
}

/// `Func<IReadOnlyList<OpenWindowInfo>>`.
pub type WindowsFn = Box<dyn Fn() -> Vec<OpenWindowInfo> + Send + Sync>;
/// `Action<nint>`.
pub type ActivateWindowFn = Arc<dyn Fn(isize) + Send + Sync>;

/// Açık pencereye geçiş. Pencere listesi panel açılırken bir kez alınır.
pub struct WindowSearchSource {
    pub base: SearchSourceBase,
    windows: WindowsFn,
    activate: ActivateWindowFn,
    entries: Mutex<Vec<WindowEntry>>,
}

const WINDOW_MIN_QUERY_LENGTH: usize = 2;
const PROCESS_NAME_WEIGHT: f64 = 0.6;

impl WindowSearchSource {
    pub fn new(windows: WindowsFn, activate: ActivateWindowFn, t: Translate) -> Self {
        let name = t("SearchCatWindows");
        WindowSearchSource { base: SearchSourceBase::new("windows", name), windows, activate, entries: Mutex::new(Vec::new()) }
    }

    fn describe(window: OpenWindowInfo) -> WindowEntry {
        let title = SearchText::fold_str(&window.title);
        let process = SearchText::fold_str(&Self::process_display_name(window.process_path.as_deref()));
        WindowEntry { window, title, process }
    }

    fn process_display_name(path: Option<&str>) -> String {
        let path = match path {
            Some(p) if !p.is_empty() => p,
            _ => return String::new(),
        };

        let start = path.rfind(['\\', '/']).map_or(0, |i| i + 1);
        let name = &path[start..];
        match name.rfind('.').filter(|&d| d > 0) {
            Some(dot) => name[..dot].to_string(),
            None => name.to_string(),
        }
    }
}

impl ISearchSource for WindowSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {
        *self.entries.lock().unwrap() = (self.windows)()
            .into_iter()
            .filter(|w| !text::is_null_or_white_space(Some(&w.title)))
            .map(Self::describe)
            .collect();
    }

    fn release(&self) {
        self.entries.lock().unwrap().clear();
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        if query.folded.chars().count() < WINDOW_MIN_QUERY_LENGTH {
            return Vec::new();
        }

        let entries = self.entries.lock().unwrap().clone();
        let mut results = Vec::new();
        for entry in &entries {
            let score = f64::max(SearchMatcher::score(query, &entry.title), SearchMatcher::score(query, &entry.process) * PROCESS_NAME_WEIGHT);
            if score <= 0.0 {
                continue;
            }

            let window = &entry.window;
            let activate = Arc::clone(&self.activate);
            let handle = window.handle;
            let mut result =
                SearchResult::new(format!("window:{}", window.handle), self.base.name(), window.title.clone(), move || activate(handle));
            result.subtitle = Some(Self::process_display_name(window.process_path.as_deref()));
            result.glyph = SearchGlyph::Window;
            result.relevance = score;
            result.category_relevance = SearchRelevance::MODERATE;
            result.load_icon = window.icon_data.clone().map(|icon| {
                Arc::new(move || Some(icon.clone())) as Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>
            });
            results.push(result);
        }
        results
    }
}

/// Hesap. Enter sonucu yazı kutusuna alır (devam edilebilsin), ilk eylem panoya kopyalar.
pub struct CalculatorSearchSource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    culture: Culture,
}

impl CalculatorSearchSource {
    /// C# `culture = null` → `CurrentCulture`: burada zorunlu parametre.
    pub fn new(shell: Arc<dyn ISearchShell>, t: Translate, culture: Culture) -> Self {
        let name = t("SearchCatCalculator");
        CalculatorSearchSource { base: SearchSourceBase::new("calculator", name), shell, t, culture }
    }
}

impl ISearchSource for CalculatorSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let Some((display, copy)) = ExpressionCalculator::try_calculate(&query.raw, &self.culture) else {
            return Vec::new();
        };

        let mut result = SearchResult::new(format!("calc:{}", query.raw), self.base.name(), display, || {});
        result.glyph = SearchGlyph::Calculator;
        result.category_relevance = SearchRelevance::HIGH;
        result.relevance = 1.0;
        result.replaces_query = Some(copy.clone());
        let shell = Arc::clone(&self.shell);
        let copying = copy.clone();
        result.actions = vec![SearchAction::new("copy", (self.t)("SearchActCopy"), move || shell.copy_text(&copying))];
        result.restores_focus = true;
        result.drag = Some(SearchDragData::new(SearchDragKind::Text, copy));
        vec![result]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemCommandEntry {
    pub command: SystemCommand,
    pub title: String,
    pub keywords: Vec<String>,
}

impl SystemCommandEntry {
    pub fn new(command: SystemCommand, title: impl Into<String>, keywords: Vec<String>) -> Self {
        SystemCommandEntry { command, title: title.into(), keywords }
    }
}

/// Kilitle, uyku, oturumu kapat, kullanıcı değiştir, yeniden başlat, kapat.
pub struct SessionSearchSource {
    pub base: SearchSourceBase,
    service: Arc<dyn ISystemCommandService>,
    entries: Vec<SystemCommandEntry>,
}

impl SessionSearchSource {
    pub fn new(service: Arc<dyn ISystemCommandService>, entries: Vec<SystemCommandEntry>, t: Translate) -> Self {
        let name = t("SearchCatSession");
        SessionSearchSource { base: SearchSourceBase::new("session", name), service, entries }
    }

    pub fn needs_confirmation(command: SystemCommand) -> bool {
        matches!(command, SystemCommand::Restart | SystemCommand::Shutdown | SystemCommand::SignOut)
    }

    /// Tam eşleşme 1; anahtar sözcüğün başıysa yazılan oran; başka durumda 0.
    pub fn keyword_score<S: AsRef<str>>(keywords: &[S], folded_term: &str) -> f64 {
        let mut best = 0.0;
        for keyword in keywords.iter().map(|k| SearchText::fold_str(k.as_ref())) {
            if keyword == folded_term {
                return 1.0;
            }

            if !folded_term.is_empty() && keyword.starts_with(folded_term) {
                best = f64::max(best, folded_term.chars().count() as f64 / keyword.chars().count() as f64);
            }
        }

        best
    }
}

impl ISearchSource for SessionSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let mut results = Vec::new();
        for entry in &self.entries {
            let score = Self::keyword_score(&entry.keywords, &query.folded);
            if score <= 0.0 {
                continue;
            }

            let service = Arc::clone(&self.service);
            let command = entry.command;
            let mut result = SearchResult::new(
                format!("session:{}", entry.command.name()),
                self.base.name(),
                entry.title.clone(),
                move || service.execute(command),
            );
            result.glyph = SearchGlyph::Power;
            result.category_relevance = if score >= 1.0 { SearchRelevance::HIGHEST } else { SearchRelevance::LOW };
            result.relevance = if score >= 1.0 { 0.9 } else { 0.9 * score };
            result.requires_confirmation = Self::needs_confirmation(entry.command);
            results.push(result);
        }
        results
    }
}

/// Pano geçmişi. İçerik ekrana gelir, bu yüzden tek harfte listelenmez.
pub struct ClipboardSearchSource {
    pub base: SearchSourceBase,
    service: Arc<dyn IClipboardHistoryService>,
    t: Translate,
}

const CLIPBOARD_MIN_QUERY_LENGTH: usize = 3;
const CLIPBOARD_MAX_RESULTS: usize = 3;
const MAX_TITLE_LENGTH: usize = 80;

impl ClipboardSearchSource {
    pub fn new(service: Arc<dyn IClipboardHistoryService>, t: Translate) -> Self {
        let name = t("SearchCatClipboard");
        ClipboardSearchSource { base: SearchSourceBase::new("clipboard", name), service, t }
    }

    fn title_of(text: &str) -> String {
        let flattened = replace_line_endings(text, " ");
        let line = flattened.trim();
        if utf16_len(line) <= MAX_TITLE_LENGTH {
            line.to_string()
        } else {
            format!("{}…", take_utf16(line, MAX_TITLE_LENGTH))
        }
    }
}

/// `string.ReplaceLineEndings(replacement)`: CRLF, CR, LF, FF, NEL, LS ve PS.
fn replace_line_endings(text: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str(replacement);
            }
            '\n' | '\u{000C}' | '\u{0085}' | '\u{2028}' | '\u{2029}' => out.push_str(replacement),
            _ => out.push(c),
        }
    }
    out
}

impl ISearchSource for ClipboardSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        if utf16_len(&query.raw) < CLIPBOARD_MIN_QUERY_LENGTH {
            return Vec::new();
        }

        self.service
            .query(ClipboardFilter::All, Some(&query.raw))
            .into_iter()
            .take(CLIPBOARD_MAX_RESULTS)
            .map(|item| {
                let service = Arc::clone(&self.service);
                let id = item.id;
                let mut result = SearchResult::new(format!("clip:{}", item.id), self.base.name(), Self::title_of(&item.text), move || {
                    service.copy_back(id);
                });
                result.subtitle = Some((self.t)("SearchHintCopy"));
                result.glyph = SearchGlyph::Clipboard;
                result.category_relevance = SearchRelevance::LOW;
                result.relevance = 0.4;
                result.restores_focus = true;
                result
            })
            .collect()
    }
}

/// `Func<DateTime>`.
pub type NowFn = Box<dyn Fn() -> NaiveDateTime + Send + Sync>;

/// "tarih" ve "saat": şimdiki değeri gösterir, Enter panoya kopyalar.
pub struct DateTimeSearchSource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    culture: Culture,
    now: NowFn,
}

impl DateTimeSearchSource {
    /// C# `now = null` → `DateTime.Now`; `culture = null` → `CurrentCulture` (zorunlu).
    pub fn new(shell: Arc<dyn ISearchShell>, t: Translate, now: Option<NowFn>, culture: Culture) -> Self {
        let name = t("SearchCatDateTime");
        DateTimeSearchSource {
            base: SearchSourceBase::new("datetime", name),
            shell,
            t,
            culture,
            now: now.unwrap_or_else(|| Box::new(|| chrono::Local::now().naive_local())),
        }
    }

    fn keywords(&self, key: &str) -> Vec<String> {
        (self.t)(key).split(';').filter(|k| !k.is_empty()).map(SearchText::fold_str).collect()
    }

    fn row(&self, id: &str, text: String) -> SearchResult {
        let shell = Arc::clone(&self.shell);
        let copy = text.clone();
        let mut result = SearchResult::new(format!("datetime:{id}"), self.base.name(), text, move || shell.copy_text(&copy));
        result.subtitle = Some((self.t)("SearchHintCopy"));
        result.glyph = SearchGlyph::Clock;
        result.category_relevance = SearchRelevance::HIGHEST;
        result.relevance = 1.0;
        result.restores_focus = true;
        result
    }
}

impl ISearchSource for DateTimeSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let date_words = self.keywords("SearchKwDate");
        let time_words = self.keywords("SearchKwTime");
        if date_words.contains(&query.folded) {
            let text = format_date_time((self.now)(), None, "D", &self.culture).unwrap_or_default();
            vec![self.row("date", text)]
        } else if time_words.contains(&query.folded) {
            let text = format_date_time((self.now)(), None, "T", &self.culture).unwrap_or_default();
            vec![self.row("time", text)]
        } else {
            Vec::new()
        }
    }
}

/// `Func<IReadOnlyList<ISearchSource>>`.
pub type SourcesFn = Box<dyn Fn() -> Vec<Arc<dyn ISearchSource>> + Send + Sync>;

/// "?" ya da "yardım": kaynakların listesi; "?ad" o kaynağın kullanım örnekleri.
pub struct HelpSearchSource {
    pub base: SearchSourceBase,
    sources: SourcesFn,
    t: Translate,
}

const HELP_TRIGGER: &str = "?";
const MIN_EXTENDED_LENGTH: usize = 3;

impl HelpSearchSource {
    pub fn new(sources: SourcesFn, t: Translate) -> Self {
        let name = t("SearchCatHelp");
        HelpSearchSource { base: SearchSourceBase::new("help", name), sources, t }
    }

    fn extended(source: &Arc<dyn ISearchSource>) -> Vec<SearchResult> {
        source
            .syntaxes()
            .iter()
            .enumerate()
            .map(|(index, syntax)| {
                let mut result =
                    SearchResult::new(format!("help:{}:{}", source.id(), index), source.name(), syntax.example.clone(), || {});
                result.subtitle = Some(syntax.description.clone());
                result.glyph = SearchGlyph::Help;
                result.category_relevance = SearchRelevance::MODERATE;
                result.relevance = 1.0 / (index as f64 + 1.0);
                result.replaces_query = Some(syntax.example.clone());
                result.skip_history = true;
                result
            })
            .collect()
    }

    /// Tetikleyici varsa ardındaki süzgeç metni, yoksa `None`.
    fn strip_trigger(&self, query: &SearchQuery) -> Option<String> {
        if let Some(rest) = query.folded.strip_prefix(HELP_TRIGGER) {
            return Some(rest.trim().to_string());
        }

        let word = SearchText::fold_str(&(self.t)("SearchKwHelp"));
        if query.folded == word {
            Some(String::new())
        } else if query.folded.starts_with(&format!("{word} ")) {
            Some(query.folded.chars().skip(word.chars().count() + 1).collect::<String>().trim().to_string())
        } else {
            None
        }
    }
}

impl ISearchSource for HelpSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let Some(filter) = self.strip_trigger(query) else {
            return Vec::new();
        };

        // ReferenceEquals(s, this)
        let me = self as *const Self as *const ();
        let documented: Vec<Arc<dyn ISearchSource>> = (self.sources)()
            .into_iter()
            .filter(|s| !std::ptr::eq(Arc::as_ptr(s) as *const (), me) && !s.syntaxes().is_empty())
            .collect();
        let matching: Vec<Arc<dyn ISearchSource>> = if filter.is_empty() {
            documented
        } else {
            documented
                .into_iter()
                .filter(|s| SearchText::fold_str(s.name()).contains(&filter) || s.id().contains(&filter))
                .collect()
        };

        if matching.len() == 1 && filter.chars().count() >= MIN_EXTENDED_LENGTH {
            return Self::extended(&matching[0]);
        }

        matching
            .iter()
            .map(|source| {
                let mut result = SearchResult::new(
                    format!("help:{}", source.id()),
                    self.base.name(),
                    source.syntaxes()[0].example.clone(),
                    || {},
                );
                result.subtitle = Some(source.name().to_string());
                result.glyph = SearchGlyph::Help;
                result.category_relevance = SearchRelevance::LOW;
                result.replaces_query = Some(format!("{}{}", HELP_TRIGGER, source.name()));
                result.skip_history = true;
                result
            })
            .collect()
    }
}

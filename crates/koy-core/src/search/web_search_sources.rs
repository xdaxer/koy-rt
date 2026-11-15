use super::search_util::{escape_data_string, format_template};
use super::{
    BrowserBookmark, DefaultBrowser, DefaultBrowserSearch, ISearchShell, ISearchSource, SearchDragData, SearchDragKind,
    SearchGlyph, SearchMatcher, SearchQuery, SearchRelevance, SearchResult, SearchSourceBase, SearchText, Translate,
};
use crate::diagnostics::{CrashError, CrashLog};
use crate::event::Event;
use crate::text;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebShortcut {
    pub key: String,
    pub provider: String,
    pub url_template: String,
}

impl WebShortcut {
    pub fn new(key: impl Into<String>, provider: impl Into<String>, url_template: impl Into<String>) -> Self {
        WebShortcut { key: key.into(), provider: provider.into(), url_template: url_template.into() }
    }
}

/// `Func<DefaultBrowser?>`.
pub type DefaultBrowserFn = Box<dyn Fn() -> Option<DefaultBrowser> + Send + Sync>;

/// "gg:kedi" ya da "!gg kedi": seçilen sitede arar. Hiç sonuç yoksa varsayılan tarayıcının kendi arama motoruyla aranır
/// ([`DefaultBrowserSearch`]); tarayıcı tanınmazsa DuckDuckGo.
pub struct WebShortcutSearchSource {
    pub base: SearchSourceBase,
    shortcuts: Vec<WebShortcut>,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    default_browser: Option<DefaultBrowserFn>,
}

impl WebShortcutSearchSource {
    /// `WebShortcutSearchSource.Defaults`.
    pub fn defaults() -> Vec<WebShortcut> {
        vec![
            WebShortcut::new("gg", "Google", "https://www.google.com/search?q={0}"),
            WebShortcut::new("ddg", "DuckDuckGo", "https://duckduckgo.com/?q={0}"),
            WebShortcut::new("bi", "Bing", "https://www.bing.com/search?q={0}"),
            WebShortcut::new("yt", "YouTube", "https://www.youtube.com/results?search_query={0}"),
            WebShortcut::new("wp", "Wikipedia", "https://www.wikipedia.org/search-redirect.php?search={0}"),
            WebShortcut::new("gh", "GitHub", "https://github.com/search?q={0}"),
            WebShortcut::new("so", "Stack Overflow", "https://stackoverflow.com/search?q={0}"),
        ]
    }

    pub fn new(
        shortcuts: Vec<WebShortcut>,
        shell: Arc<dyn ISearchShell>,
        t: Translate,
        default_browser: Option<DefaultBrowserFn>,
    ) -> Self {
        let name = t("SearchCatWeb");
        WebShortcutSearchSource { base: SearchSourceBase::new("web", name), shortcuts, shell, t, default_browser }
    }

    /// Hiç sonuç yokken arama satırı: varsayılan tarayıcının motoru, tanınmazsa DuckDuckGo.
    pub fn fallback(&self, query: &SearchQuery) -> Option<SearchResult> {
        if query.is_empty() {
            return None;
        }

        // Kayıt defteri okuması ucuz ve yalnız sonuçsuz arama bitince yapılır; kullanıcı tarayıcıyı değiştirmiş olabilir.
        if let Some(browser) = self.default_browser.as_ref().and_then(|f| f()) {
            if let Some(command) = DefaultBrowserSearch::search_command(&browser, &query.raw) {
                let shell = Arc::clone(&self.shell);
                return Some(self.row_with("browser", &browser.name, &query.raw, move || {
                    shell.run_with_arguments(&command.executable, &command.arguments)
                }));
            }
        }

        let shortcut = self
            .shortcuts
            .iter()
            .find(|s| s.key == "ddg")
            .or_else(|| self.shortcuts.first());
        shortcut.map(|s| self.row(s, &query.raw))
    }

    /// `TryParse(text, out key, out term)`: başarılıysa `Some((key, term))`.
    pub fn try_parse(text: &str) -> Option<(String, String)> {
        if text.starts_with('!') {
            let space = text.find(' ')?;
            if space <= 1 {
                return None;
            }

            let key = &text[1..space];
            let term = text[space + 1..].trim();
            return if term.is_empty() { None } else { Some((key.to_string(), term.to_string())) };
        }

        let colon = text.find(':')?;
        if colon == 0 || text[..colon].contains(' ') {
            return None;
        }

        let key = &text[..colon];
        let term = text[colon + 1..].trim();

        // "C:\klasör" ya da "http://..." kısayol değildir.
        if !term.is_empty() && !term.starts_with('\\') && !term.starts_with('/') {
            Some((key.to_string(), term.to_string()))
        } else {
            None
        }
    }

    fn row(&self, shortcut: &WebShortcut, term: &str) -> SearchResult {
        let url = format_template(&shortcut.url_template, &[&escape_data_string(term)]);
        let shell = Arc::clone(&self.shell);
        self.row_with(&shortcut.key, &shortcut.provider, term, move || shell.open(&url))
    }

    fn row_with(&self, key: &str, provider: &str, term: &str, execute: impl Fn() + Send + Sync + 'static) -> SearchResult {
        let title = format_template(&(self.t)("SearchWebFor"), &[provider, term]);
        let mut result = SearchResult::new(format!("web:{key}:{term}"), self.base.name(), title, execute);
        result.glyph = SearchGlyph::Globe;
        result.category_relevance = SearchRelevance::HIGHEST;
        result.relevance = 0.9;
        result
    }
}

impl ISearchSource for WebShortcutSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let Some((key, term)) = Self::try_parse(&query.raw) else {
            return Vec::new();
        };

        match self.shortcuts.iter().find(|s| text::eq_ignore_case(&s.key, &key)) {
            Some(shortcut) => vec![self.row(shortcut, &term)],
            None => Vec::new(),
        }
    }
}

/// `Func<IReadOnlyList<BrowserBookmark>>`.
pub type LoadBookmarksFn = Arc<dyn Fn() -> Vec<BrowserBookmark> + Send + Sync>;

struct BookmarksInner {
    changed: Arc<Event<()>>,
    bookmarks: Mutex<Vec<BrowserBookmark>>,
}

/// Tarayıcı yer imleri (Chrome ailesi). Panel açılınca arka planda okunur.
pub struct BookmarksSearchSource {
    pub base: SearchSourceBase,
    load: LoadBookmarksFn,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    inner: Arc<BookmarksInner>,
}

const BOOKMARKS_MIN_QUERY_LENGTH: usize = 3;

impl BookmarksSearchSource {
    pub fn new(load: LoadBookmarksFn, shell: Arc<dyn ISearchShell>, t: Translate) -> Self {
        let name = t("SearchCatBookmarks");
        let base = SearchSourceBase::new("bookmarks", name);
        let inner = Arc::new(BookmarksInner { changed: base.changed_handle(), bookmarks: Mutex::new(Vec::new()) });
        BookmarksSearchSource { base, load, shell, t, inner }
    }
}

impl ISearchSource for BookmarksSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {
        // `_ = LoadAsync()`: arka planda okunur, bitince Changed gelir.
        let inner = Arc::clone(&self.inner);
        let load = Arc::clone(&self.load);
        std::thread::spawn(move || match catch_unwind(AssertUnwindSafe(|| load())) {
            Ok(list) => {
                *inner.bookmarks.lock().unwrap() = list;
                inner.changed.invoke(&());
            }
            Err(payload) => {
                let message = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_default();
                CrashLog::write("Yer imleri", &CrashError::exception("Panic", &message, ""));
            }
        });
    }

    fn release(&self) {
        self.inner.bookmarks.lock().unwrap().clear();
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let all = query.folded == SearchText::fold_str(&(self.t)("SearchKwBookmarks"));
        if !all && query.folded.chars().count() < BOOKMARKS_MIN_QUERY_LENGTH {
            return Vec::new();
        }

        let bookmarks = self.inner.bookmarks.lock().unwrap().clone();
        let mut results = Vec::new();
        for bookmark in &bookmarks {
            let title = SearchText::fold_str(&bookmark.title);
            let score = if all {
                0.5
            } else {
                f64::max(
                    SearchMatcher::score(query, &title),
                    SearchMatcher::score(query, &SearchText::fold_str(&bookmark.url)) * 0.6,
                )
            };
            if score <= 0.0 {
                continue;
            }

            let url = bookmark.url.clone();
            let shell = Arc::clone(&self.shell);
            let open_url = url.clone();
            let mut result = SearchResult::new(format!("url:{url}"), self.base.name(), bookmark.title.clone(), move || {
                shell.open(&open_url)
            });
            result.subtitle = Some(url.clone());
            result.glyph = SearchGlyph::Bookmark;
            result.category_relevance = if title == query.folded { SearchRelevance::HIGH } else { SearchRelevance::MODERATE };
            result.relevance = score;
            result.drag = Some(SearchDragData::new(SearchDragKind::Url, url));
            results.push(result);
        }
        results
    }
}

/// Chrome, Edge ve Brave'in `Bookmarks` dosyasını okur. Bozuk dosyada boş döner.
pub struct ChromiumBookmarks;

const BOOKMARKS_MAX_DEPTH: i32 = 64;
const MAX_BOOKMARKS: usize = 5000;

impl ChromiumBookmarks {
    pub fn parse(json: &str) -> Vec<BrowserBookmark> {
        let mut found = Vec::new();
        let Ok(document) = serde_json::from_str::<serde_json::Value>(json) else {
            return Vec::new();
        };

        if let Some(roots) = document.as_object().and_then(|o| o.get("roots")).and_then(|r| r.as_object()) {
            for (_, root) in roots {
                Self::collect(root, &mut found, 0);
            }
        }

        found
    }

    fn collect(node: &serde_json::Value, found: &mut Vec<BrowserBookmark>, depth: i32) {
        if depth > BOOKMARKS_MAX_DEPTH || found.len() >= MAX_BOOKMARKS || !node.is_object() {
            return;
        }

        if Self::text(node, "type") == Some("url") {
            if let Some(url) = Self::text(node, "url") {
                if !url.is_empty() && (text::starts_with_ignore_case(url, "http://") || text::starts_with_ignore_case(url, "https://")) {
                    found.push(BrowserBookmark::new(Self::text(node, "name").unwrap_or(url), url));
                    return;
                }
            }
        }

        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                Self::collect(child, found, depth + 1);
            }
        }
    }

    fn text<'a>(node: &'a serde_json::Value, name: &str) -> Option<&'a str> {
        node.get(name).and_then(|v| v.as_str())
    }
}

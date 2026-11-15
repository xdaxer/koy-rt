use super::search_util::{format_template, panic_message, sleep_unless_canceled, utf16_len};
use super::uri_util::try_create_absolute;
use super::{
    is_canceled, log_service_error, IFileSearchService, ISearchShell, ISearchSource, IndexedItem, RunningProcess, SearchAction,
    SearchCategoryPlacement, SearchCommandSafety, SearchDragData, SearchDragKind, SearchGlyph, SearchPlace, SearchQuery,
    SearchRelevance, SearchResult, SearchSourceBase, SearchText, ServiceError, Translate,
};
use crate::cancel::{CancellationToken, CancellationTokenSource};
use crate::event::Event;
use crate::text;
use crate::win_path;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

/// Yazılanın ilk sözcüğü PATH'te bir programsa "Çalıştır" önerir.
pub struct ShellCommandSearchSource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    /// Her tuşta PATH × PATHEXT taranmasın: çözülen (ya da bulunamayan) komutlar panel oturumu boyunca saklanır.
    /// Anahtar `OrdinalIgnoreCase`.
    resolved: Mutex<HashMap<String, Option<String>>>,
}

const DEFAULT_EXTENSIONS: [&str; 4] = [".com", ".exe", ".bat", ".cmd"];

impl ShellCommandSearchSource {
    pub fn new(shell: Arc<dyn ISearchShell>, t: Translate) -> Self {
        let name = t("SearchCatShell");
        ShellCommandSearchSource { base: SearchSourceBase::new("shell", name), shell, t, resolved: Mutex::new(HashMap::new()) }
    }

    /// İlk sözcük (çift tırnaklı olabilir) ve kalan metin.
    pub fn split_command(text: &str) -> (String, Option<String>) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return (String::new(), None);
        }

        let end;
        let command;
        if trimmed.starts_with('"') {
            let Some(close) = trimmed[1..].find('"').map(|i| i + 1) else {
                return (String::new(), None);
            };

            command = &trimmed[1..close];
            end = close + 1;
        } else {
            end = trimmed.find(' ').unwrap_or(trimmed.len());
            command = &trimmed[..end];
        }

        let rest = trimmed[end..].trim();
        (command.to_string(), if rest.is_empty() { None } else { Some(rest.to_string()) })
    }

    fn find_executable(&self, command: &str) -> Option<String> {
        let key = text::to_upper_invariant(command);
        if let Some(known) = self.resolved.lock().unwrap().get(&key) {
            return known.clone();
        }

        let found = self.resolve(command);
        self.resolved.lock().unwrap().insert(key, found.clone());
        found
    }

    /// Ağ yolları (\\sunucu\paylaşım) yoklanmaz: yanıt vermeyen paylaşım arayüz iş parçacığını bekletir.
    pub fn is_network_path(path: &str) -> bool {
        let trimmed = path.trim_start();
        trimmed.starts_with("\\\\") || trimmed.starts_with("//")
    }

    fn resolve(&self, command: &str) -> Option<String> {
        // Yolu verilmiş komutu Konumlar değil, burası çalıştırır; ama yalnız program uzantılarında.
        if command.contains('\\') || command.contains('/') {
            return if !Self::is_network_path(command) && self.has_executable_extension(command) && self.shell.file_exists(command) {
                Some(command.to_string())
            } else {
                None
            };
        }

        let extensions = self.extensions();
        let has_extension = self.has_executable_extension(command);
        let path_value = self.shell.get_environment_variable("PATH").unwrap_or_default();
        for directory in path_value.split(';').filter(|d| !d.is_empty()) {
            if Self::is_network_path(directory) {
                continue;
            }

            if has_extension {
                let direct = win_path::combine(directory.trim(), command);
                if self.shell.file_exists(&direct) {
                    return Some(direct);
                }
                continue;
            }

            for extension in &extensions {
                let candidate = win_path::combine(directory.trim(), &format!("{command}{extension}"));
                if self.shell.file_exists(&candidate) {
                    return Some(candidate);
                }
            }
        }

        None
    }

    fn has_executable_extension(&self, command: &str) -> bool {
        self.extensions().iter().any(|e| text::ends_with_ignore_case(command, e))
    }

    fn extensions(&self) -> Vec<String> {
        let value = self.shell.get_environment_variable("PATHEXT");
        match value {
            Some(v) if !text::is_null_or_white_space(Some(&v)) => v.split(';').filter(|e| !e.is_empty()).map(str::to_string).collect(),
            _ => DEFAULT_EXTENSIONS.iter().map(|e| e.to_string()).collect(),
        }
    }
}

impl ISearchSource for ShellCommandSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {
        self.resolved.lock().unwrap().clear();
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        // Kabuğa satır sonu hiç gitmez; pano ve çok satırlı metin ayrıca onay ister.
        let line = SearchCommandSafety::to_single_line(&query.raw);
        let (command, arguments) = Self::split_command(&line);
        if command.is_empty() {
            return Vec::new();
        }
        let Some(executable) = self.find_executable(&command) else {
            return Vec::new();
        };

        let confirm = query.needs_command_confirmation();
        let shown = SearchCommandSafety::for_display(&query.raw);
        let shell = Arc::clone(&self.shell);
        let exe = executable.clone();
        let args = arguments.clone();
        let mut result = SearchResult::new(
            format!("exec:{} {}", executable, arguments.as_deref().unwrap_or("")),
            self.base.name(),
            format_template(&(self.t)("SearchRunCommand"), &[&shown]),
            move || shell.run(&exe, args.as_deref()),
        );
        result.glyph = SearchGlyph::Terminal;
        result.category_relevance = SearchRelevance::HIGHEST;
        result.relevance = 0.7;
        result.requires_confirmation = confirm;
        result.confirmation_detail = if confirm { Some(shown) } else { None };
        let shell = Arc::clone(&self.shell);
        let mut terminal = SearchAction::new("terminal", (self.t)("SearchActRunInTerminal"), move || shell.run_in_terminal(&line));
        terminal.requires_confirmation = confirm;
        result.actions = vec![terminal];
        vec![result]
    }
}

/// Yazılan yol varsa açar, adresse gider. `%USERPROFILE%` gibi değişkenler çözülür.
pub struct LocationSearchSource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
}

const WEB_SCHEMES: [&str; 3] = ["http", "https", "ftp"];

impl LocationSearchSource {
    pub fn new(shell: Arc<dyn ISearchShell>, t: Translate) -> Self {
        let name = t("SearchCatLocations");
        LocationSearchSource { base: SearchSourceBase::new("locations", name), shell, t }
    }

    fn local(&self, text: &str, path: &str, glyph: SearchGlyph) -> SearchResult {
        let shell = Arc::clone(&self.shell);
        let open = path.to_string();
        let mut result = SearchResult::new(
            format!("path:{path}"),
            self.base.name(),
            format_template(&(self.t)("SearchOpen"), &[text]),
            move || shell.open(&open),
        );
        result.glyph = glyph;
        result.category_relevance = SearchRelevance::HIGHEST;
        result.relevance = 1.0;
        result.drag = Some(SearchDragData::new(SearchDragKind::File, path));
        result
    }

    fn is_program(path: &str) -> bool {
        text::ends_with_ignore_case(path, ".exe") || text::ends_with_ignore_case(path, ".bat") || text::ends_with_ignore_case(path, ".cmd")
    }

    /// "C:\..." ya da değişkenle başlayan yol; ağ yolu denetimi UI'yi bekletebilir, bu yüzden yerel diskle sınırlı.
    fn is_local_path_like(text: &str) -> bool {
        let c: Vec<char> = text.chars().take(3).collect();
        c.len() >= 3 && c[0].is_ascii_alphabetic() && c[1] == ':' && (c[2] == '\\' || c[2] == '/')
    }

    fn expand_environment(&self, text: &str) -> Option<String> {
        if !text.contains('%') {
            return Some(text.to_string());
        }

        let mut result = text.to_string();
        let mut start = result.find('%');
        while let Some(s) = start {
            let Some(end) = result[s + 1..].find('%').map(|i| i + s + 1) else { break };

            let value = self.shell.get_environment_variable(&result[s + 1..end])?;

            result = format!("{}{}{}", &result[..s], value, &result[end + 1..]);
            let from = s + value.len();
            start = if from <= result.len() { result[from..].find('%').map(|i| i + from) } else { None };
        }

        Some(result)
    }

    /// `NormalizeUrl`: `Some((şema, AbsoluteUri))`.
    fn normalize_url(text: &str) -> Option<(String, String)> {
        if text.contains(' ') {
            return None;
        }

        let text = if text::starts_with_ignore_case(text, "www.") { format!("https://{text}") } else { text.to_string() };

        let uri = try_create_absolute(&text)?;
        if WEB_SCHEMES.contains(&uri.scheme.as_str()) || uri.scheme == "mailto" { Some((uri.scheme, uri.absolute_uri)) } else { None }
    }
}

impl ISearchSource for LocationSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let text = query.raw.as_str();
        if text.is_empty() {
            return Vec::new();
        }

        if let Some(expanded) = self.expand_environment(text) {
            if Self::is_local_path_like(&expanded) {
                if self.shell.directory_exists(&expanded) {
                    return vec![self.local(text, &expanded, SearchGlyph::Folder)];
                }

                if self.shell.file_exists(&expanded) && !Self::is_program(&expanded) {
                    return vec![self.local(text, &expanded, SearchGlyph::File)];
                }

                return Vec::new();
            }
        }

        let Some((scheme, target)) = Self::normalize_url(text) else {
            return Vec::new();
        };

        let title = if scheme == "mailto" {
            let after: String = text.chars().skip(text.chars().position(|c| c == ':').map_or(0, |i| i + 1)).collect();
            format_template(&(self.t)("SearchSendMail"), &[&after])
        } else {
            format_template(&(self.t)("SearchGoTo"), &[text])
        };
        let shell = Arc::clone(&self.shell);
        let open = target.clone();
        let mut result = SearchResult::new(format!("url:{target}"), self.base.name(), title, move || shell.open(&open));
        result.glyph = SearchGlyph::Globe;
        result.category_relevance = SearchRelevance::LOW;
        result.relevance = 1.0;
        result.drag = Some(SearchDragData::new(SearchDragKind::Url, target));
        vec![result]
    }
}

/// `Func<IReadOnlyList<SearchPlace>>`.
pub type PlacesFn = Box<dyn Fn() -> Vec<SearchPlace> + Send + Sync>;

/// Masaüstü, Belgeler gibi bilinen klasörler ve sürücüler. Anahtar sözcük hepsini listeler.
pub struct PlacesSearchSource {
    pub base: SearchSourceBase,
    places: PlacesFn,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    current: Mutex<Vec<SearchPlace>>,
}

impl PlacesSearchSource {
    pub fn new(places: PlacesFn, shell: Arc<dyn ISearchShell>, t: Translate) -> Self {
        let name = t("SearchCatPlaces");
        PlacesSearchSource { base: SearchSourceBase::new("places", name), places, shell, t, current: Mutex::new(Vec::new()) }
    }
}

impl ISearchSource for PlacesSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {
        *self.current.lock().unwrap() = (self.places)();
    }

    fn release(&self) {
        self.current.lock().unwrap().clear();
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let all = query.folded == SearchText::fold_str(&(self.t)("SearchKwPlaces"));
        let places = self.current.lock().unwrap().clone();
        let mut results = Vec::new();
        for place in &places {
            let folded = SearchText::fold_str(&place.name);
            let category;
            let relevance;
            if all || folded == query.folded {
                category = SearchRelevance::HIGHEST;
                relevance = if all { 0.9 } else { 1.0 };
            } else if folded.contains(&query.folded) {
                category = SearchRelevance::HIGH;
                relevance = 0.7;
            } else {
                continue;
            }

            let path = place.path.clone();
            let shell = Arc::clone(&self.shell);
            let open = path.clone();
            let mut result = SearchResult::new(format!("path:{path}"), self.base.name(), place.name.clone(), move || shell.open(&open));
            result.subtitle = place.group.clone();
            result.glyph = SearchGlyph::Folder;
            result.category_relevance = category;
            result.relevance = relevance;
            result.drag = Some(SearchDragData::new(SearchDragKind::File, path));
            results.push(result);
        }
        results
    }
}

/// `Func<IReadOnlyList<string>>`.
pub type PathsFn = Box<dyn Fn() -> Vec<String> + Send + Sync>;

/// Son kullanılan belgeler; panel açılırken bir kez okunur.
pub struct RecentDocumentsSearchSource {
    pub base: SearchSourceBase,
    paths: PathsFn,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    current: Mutex<Vec<String>>,
}

const MIN_EXACT_LENGTH: usize = 5;

impl RecentDocumentsSearchSource {
    pub fn new(paths: PathsFn, shell: Arc<dyn ISearchShell>, t: Translate) -> Self {
        let name = t("SearchCatRecent");
        RecentDocumentsSearchSource { base: SearchSourceBase::new("recent", name), paths, shell, t, current: Mutex::new(Vec::new()) }
    }
}

impl ISearchSource for RecentDocumentsSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {
        *self.current.lock().unwrap() = (self.paths)();
    }

    fn release(&self) {
        self.current.lock().unwrap().clear();
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let paths = self.current.lock().unwrap().clone();
        let mut relevance = 0.75;
        let mut results = Vec::new();
        for path in &paths {
            let name = win_path::get_file_name(path);
            let folded = SearchText::fold_str(name);
            let Some(index) = folded.find(&query.folded) else { continue };

            let query_len = query.folded.chars().count();
            let exact = query_len >= MIN_EXACT_LENGTH
                && index == 0
                && (folded.chars().count() == query_len
                    || SearchText::fold_str(win_path::get_file_name_without_extension(name)).chars().count() == query_len);

            let captured = path.clone();
            let shell = Arc::clone(&self.shell);
            let open = captured.clone();
            let mut result = SearchResult::new(format!("path:{path}"), self.base.name(), name, move || shell.open(&open));
            result.subtitle = win_path::get_directory_name(path);
            result.glyph = SearchGlyph::File;
            result.placement = SearchCategoryPlacement::Bottom;
            result.category_relevance = if exact {
                SearchRelevance::HIGHEST
            } else if index == 0 {
                SearchRelevance::HIGH
            } else {
                SearchRelevance::LOW
            };
            result.relevance = if index == 0 { relevance + 0.1 } else { relevance };
            let shell = Arc::clone(&self.shell);
            let folder = captured.clone();
            result.actions = vec![SearchAction::new("folder", (self.t)("SearchActOpenFolder"), move || shell.open_containing_folder(&folder))];
            result.drag = Some(SearchDragData::new(SearchDragKind::File, captured));
            results.push(result);

            relevance -= 0.05;
        }
        results
    }
}

/// Arka plan görevi yazar, arayüz iş parçacığı okur: bütün durum bu kilitle korunur.
struct FileState {
    running: Option<(u64, CancellationTokenSource)>,
    next_id: u64,
    pending: String,
    ready: String,
    items: Vec<IndexedItem>,
}

struct FileInner {
    service: Arc<dyn IFileSearchService>,
    changed: Arc<Event<()>>,
    state: Mutex<FileState>,
}

/// `Func<byte[]?>`.
pub type NoIconFn = Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>;

/// Windows Arama dizininde dosya adı araması; türe göre gruplanır, yazma durunca çalışır.
pub struct FileSearchSource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    no_icon: Option<NoIconFn>,
    inner: Arc<FileInner>,
}

const FILE_MIN_QUERY_LENGTH: usize = 2;
const FETCH_LIMIT: i32 = 60;
const PER_CATEGORY: usize = 10;
const FILE_DEBOUNCE_MS: u64 = 150;

impl FileSearchSource {
    pub fn new(service: Arc<dyn IFileSearchService>, shell: Arc<dyn ISearchShell>, t: Translate, no_icon: Option<NoIconFn>) -> Self {
        let name = t("SearchCatFiles");
        let base = SearchSourceBase::new("files", name);
        let inner = Arc::new(FileInner {
            service,
            changed: base.changed_handle(),
            state: Mutex::new(FileState { running: None, next_id: 0, pending: String::new(), ready: String::new(), items: Vec::new() }),
        });
        FileSearchSource { base, shell, t, no_icon, inner }
    }

    /// Uzantıya göre sonuç grubu; tanınmayan "Dosyalar".
    pub fn category_key(path: &str, is_folder: bool) -> &'static str {
        if is_folder {
            return "SearchCatFolders";
        }

        match text::to_lower_invariant(win_path::get_extension(path)).as_str() {
            ".mp3" | ".flac" | ".wav" | ".aac" | ".ogg" | ".m4a" | ".wma" => "SearchCatAudio",
            ".jpg" | ".jpeg" | ".png" | ".gif" | ".bmp" | ".webp" | ".svg" | ".heic" | ".tiff" => "SearchCatImages",
            ".mp4" | ".mkv" | ".avi" | ".mov" | ".webm" | ".wmv" | ".m4v" => "SearchCatVideos",
            ".xls" | ".xlsx" | ".csv" | ".ods" => "SearchCatSpreadsheets",
            ".ppt" | ".pptx" | ".odp" => "SearchCatPresentations",
            ".doc" | ".docx" | ".pdf" | ".odt" | ".rtf" => "SearchCatDocuments",
            ".zip" | ".rar" | ".7z" | ".tar" | ".gz" => "SearchCatArchives",
            ".txt" | ".md" | ".log" | ".json" | ".xml" | ".yml" | ".yaml" | ".ini" => "SearchCatTexts",
            _ => "SearchCatFiles",
        }
    }

    fn build(&self, query: &SearchQuery, items: &[IndexedItem]) -> Vec<SearchResult> {
        // GroupBy: ilk görülme sırası.
        let mut groups: Vec<(&'static str, Vec<&IndexedItem>)> = Vec::new();
        for item in items {
            let key = Self::category_key(&item.path, item.is_folder);
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, g)) => g.push(item),
                None => groups.push((key, vec![item])),
            }
        }

        groups
            .into_iter()
            .flat_map(|(key, group)| {
                let category = (self.t)(key);
                group.into_iter().take(PER_CATEGORY).map(|item| self.to_result(query, item, &category)).collect::<Vec<_>>()
            })
            .collect()
    }

    fn to_result(&self, query: &SearchQuery, item: &IndexedItem, category: &str) -> SearchResult {
        let name = win_path::get_file_name(item.path.trim_end_matches('\\'));
        let folded = SearchText::fold_str(name);
        let exact = folded == query.folded
            || SearchText::fold_str(win_path::get_file_name_without_extension(name)) == query.folded;
        let path = item.path.clone();
        let shell = Arc::clone(&self.shell);
        let open = path.clone();
        let mut result = SearchResult::new(format!("path:{path}"), category, name, move || shell.open(&open));
        result.subtitle = win_path::get_directory_name(&path);
        result.glyph = if item.is_folder { SearchGlyph::Folder } else { SearchGlyph::File };
        result.placement = SearchCategoryPlacement::Bottom;
        result.category_relevance = if exact {
            SearchRelevance::HIGHEST
        } else if folded.contains(&query.folded) {
            SearchRelevance::HIGH
        } else {
            SearchRelevance::LOW
        };
        result.relevance = 0.75;
        let shell = Arc::clone(&self.shell);
        let folder = path.clone();
        result.actions = vec![SearchAction::new("folder", (self.t)("SearchActOpenFolder"), move || shell.open_containing_folder(&folder))];
        result.load_icon = self.no_icon.clone();
        result.drag = Some(SearchDragData::new(SearchDragKind::File, path));
        result
    }

    fn cancel_locked(state: &mut FileState) {
        // Kaynağı görev kendi sonunda bırakır; burada yalnız iptal edilir.
        if let Some((_, cts)) = state.running.take() {
            cts.cancel();
        }
        state.pending = String::new();
    }

    fn start_locked(&self, state: &mut FileState, term: &str) {
        if let Some((_, cts)) = state.running.take() {
            cts.cancel();
        }
        let cts = CancellationTokenSource::new();
        state.next_id += 1;
        let id = state.next_id;
        // Kaynak başka iş parçacığında bırakılabilir; belirteç baştan alınır.
        let token = cts.token();
        state.running = Some((id, cts));
        state.pending = term.to_string();

        let inner = Arc::clone(&self.inner);
        let term = term.to_string();
        std::thread::spawn(move || Self::run(inner, term, id, token));
    }

    /// C# `RunAsync`.
    fn run(inner: Arc<FileInner>, term: String, id: u64, token: CancellationToken) {
        if !sleep_unless_canceled(FILE_DEBOUNCE_MS, &token) {
            return;
        }

        let outcome = catch_unwind(AssertUnwindSafe(|| inner.service.search(&term, FETCH_LIMIT, &token)));
        let failure: ServiceError = match outcome {
            Ok(Ok(items)) => {
                {
                    let mut state = inner.state.lock().unwrap();
                    if token.is_cancellation_requested() || state.running.as_ref().map(|(i, _)| *i) != Some(id) {
                        return;
                    }

                    state.items = items;
                    state.ready = term;
                    state.pending = String::new();
                    state.running = None;
                }

                inner.changed.invoke(&());
                return;
            }
            Ok(Err(e)) => e,
            Err(payload) => panic_message(payload.as_ref()).into(),
        };

        // Yeni arama geldi ya da panel kapandı.
        if is_canceled(&failure) {
            return;
        }

        log_service_error("Dosya arama", &failure);
        let cleared = {
            let mut state = inner.state.lock().unwrap();
            let cleared = state.running.as_ref().map(|(i, _)| *i) == Some(id);
            if cleared {
                state.running = None;
                state.pending = String::new();
            }
            cleared
        };

        if cleared {
            inner.changed.invoke(&());
        }
    }
}

impl ISearchSource for FileSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        self.inner.state.lock().unwrap().running.is_some()
    }

    fn prepare(&self) {}

    fn release(&self) {
        let mut state = self.inner.state.lock().unwrap();
        Self::cancel_locked(&mut state);
        state.ready = String::new();
        state.items = Vec::new();
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        // "=" hesap makinesinindir; dizin sorgusunda işleç sayılır.
        if utf16_len(&query.raw) < FILE_MIN_QUERY_LENGTH || query.raw.starts_with('=') {
            Self::cancel_locked(&mut self.inner.state.lock().unwrap());
            return Vec::new();
        }

        let (items, ready) = {
            let mut state = self.inner.state.lock().unwrap();
            if state.ready != query.raw && state.pending != query.raw {
                self.start_locked(&mut state, &query.raw);
            }

            (state.items.clone(), state.ready == query.raw)
        };

        if ready { self.build(query, &items) } else { Vec::new() }
    }
}

/// `Func<IReadOnlyList<RunningProcess>>`.
pub type ProcessesFn = Box<dyn Fn() -> Vec<RunningProcess> + Send + Sync>;
/// `Action<RunningProcess>`.
pub type ProcessActionFn = Arc<dyn Fn(RunningProcess) + Send + Sync>;
/// `Func<long>`.
pub type ClockFn = Box<dyn Fn() -> i64 + Send + Sync>;

fn tick_count_64() -> i64 {
    // `Environment.TickCount64`: sistem açılışından beri milisaniye; burada süreç başlangıcından beri.
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_millis() as i64
}

struct KillSnapshot {
    list: Option<Vec<RunningProcess>>,
    at: i64,
}

/// "kill ad": eşleşen süreçleri listeler. Tetik sözcüğü olmadan hiç çalışmaz.
/// Ana eylem nazik kapatma; zorla sonlandırma ayrı eylemdir ve onay ister.
pub struct KillSearchSource {
    pub base: SearchSourceBase,
    processes: ProcessesFn,
    terminate: ProcessActionFn,
    request_close: ProcessActionFn,
    t: Translate,
    clock: ClockFn,
    snapshot: Mutex<KillSnapshot>,
}

const KILL_MIN_NAME_LENGTH: usize = 2;

impl KillSearchSource {
    /// Liste bundan eskiyse yeniden alınır (panel iğnelenip açık kalırsa kimlikler eskir).
    pub const MAX_SNAPSHOT_AGE_MS: i64 = 10_000;

    /// C# `clock = null` → `Environment.TickCount64`.
    pub fn new(
        processes: ProcessesFn,
        terminate: ProcessActionFn,
        request_close: ProcessActionFn,
        t: Translate,
        clock: Option<ClockFn>,
    ) -> Self {
        let name = t("SearchCatProcesses");
        KillSearchSource {
            base: SearchSourceBase::new("kill", name),
            processes,
            terminate,
            request_close,
            t,
            clock: clock.unwrap_or_else(|| Box::new(tick_count_64)),
            snapshot: Mutex::new(KillSnapshot { list: None, at: 0 }),
        }
    }
}

impl ISearchSource for KillSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {
        self.snapshot.lock().unwrap().list = None;
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let word = format!("{} ", SearchText::fold_str(&(self.t)("SearchKwKill")));
        if !query.folded.starts_with(&word) {
            return Vec::new();
        }

        let term: String = query.folded.chars().skip(word.chars().count()).collect::<String>().trim().to_string();
        if term.chars().count() < KILL_MIN_NAME_LENGTH {
            return Vec::new();
        }

        // Süreç listesi pahalı: yalnız bu kaynak gerçekten kullanılınca, kısa süreliğine saklanır.
        let now = (self.clock)();
        let list = {
            let mut snapshot = self.snapshot.lock().unwrap();
            if snapshot.list.is_none() || now - snapshot.at > Self::MAX_SNAPSHOT_AGE_MS {
                snapshot.list = Some((self.processes)());
                snapshot.at = now;
            }
            snapshot.list.clone().unwrap()
        };

        list.into_iter()
            .filter(|p| SearchText::fold_str(&p.name).contains(&term))
            .map(|p| {
                let close = Arc::clone(&self.request_close);
                let terminate = Arc::clone(&self.terminate);
                let closing = p.clone();
                let terminating = p.clone();
                let mut result = SearchResult::new(
                    format!("kill:{}", p.id),
                    self.base.name(),
                    format_template(&(self.t)("SearchCloseProcess"), &[&p.name]),
                    move || close(closing.clone()),
                );
                result.subtitle = Some(format_template(&(self.t)("SearchProcessId"), &[&p.id.to_string()]));
                result.glyph = SearchGlyph::Process;
                result.category_relevance = SearchRelevance::MODERATE;
                result.relevance = if SearchText::fold_str(&p.name) == term { 1.0 } else { 0.9 };
                let mut action = SearchAction::new("terminate", (self.t)("SearchActTerminate"), move || terminate(terminating.clone()));
                action.requires_confirmation = true;
                result.actions = vec![action];
                result
            })
            .collect()
    }
}

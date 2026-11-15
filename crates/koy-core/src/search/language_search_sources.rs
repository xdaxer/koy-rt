use super::search_util::{escape_data_string, format_template, panic_message, sleep_unless_canceled, two_letter_language_name};
use super::{
    is_canceled, log_service_error, ISearchShell, ISearchSource, SearchAction, SearchDragData, SearchDragKind, SearchGlyph, SearchQuery, SearchRelevance,
    SearchResult, SearchSourceBase, ServiceError, Translate,
};
use crate::cancel::{CancellationToken, CancellationTokenSource};
use crate::clock::Culture;
use crate::event::Event;
use crate::text;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

/// Yazım denetimi sonucu.
/// `is_supported`: Sistemde uygun sözlük bulunabildi mi. `is_correct`: Metin yazım hatası içermiyor mu.
/// `suggestions`: Önerilen düzeltmeler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCheckResult {
    pub is_supported: bool,
    pub is_correct: bool,
    pub suggestions: Vec<String>,
}

impl SpellCheckResult {
    pub fn new(is_supported: bool, is_correct: bool, suggestions: Vec<String>) -> Self {
        SpellCheckResult { is_supported, is_correct, suggestions }
    }
}

pub trait ISpellCheckerService: Send + Sync {
    fn check(&self, text: &str) -> SpellCheckResult;

    fn release(&self);
}

/// Sözlük anlamı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryMeaning {
    pub part_of_speech: String,
    pub definition: String,
    pub example: Option<String>,
}

impl DictionaryMeaning {
    /// C# `DictionaryMeaning(PartOfSpeech, Definition, Example = null)`: `example` zorunlu parametre.
    pub fn new(part_of_speech: impl Into<String>, definition: impl Into<String>, example: Option<String>) -> Self {
        DictionaryMeaning { part_of_speech: part_of_speech.into(), definition: definition.into(), example }
    }
}

/// Sözlük kaydı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryEntry {
    pub word: String,
    pub phonetic: Option<String>,
    pub meanings: Vec<DictionaryMeaning>,
    pub source_url: Option<String>,
}

impl DictionaryEntry {
    /// C# `DictionaryEntry(Word, Phonetic, Meanings, SourceUrl = null)`: `source_url` zorunlu parametre.
    pub fn new(
        word: impl Into<String>,
        phonetic: Option<String>,
        meanings: Vec<DictionaryMeaning>,
        source_url: Option<String>,
    ) -> Self {
        DictionaryEntry { word: word.into(), phonetic, meanings, source_url }
    }
}

pub trait IDictionaryService: Send + Sync {
    /// C# `LookupAsync`: arka plan iş parçacığında çağrılır; iptal edilirse `Err(OperationCanceled)`.
    fn lookup(&self, word: &str, cancel: &CancellationToken) -> Result<Option<DictionaryEntry>, ServiceError>;
}

/// "Anahtar sözcük + boşluk/iki nokta + terim" önekini ayıklar (C# `internal static class SearchPrefix`).
pub(crate) struct SearchPrefix;

impl SearchPrefix {
    pub(crate) fn try_extract(raw: &str, semicolon_separated_keywords: &str) -> Option<String> {
        let trimmed = raw.trim();
        for kw in semicolon_separated_keywords.split(';').filter(|k| !k.is_empty()) {
            if text::starts_with_ignore_case(trimmed, &format!("{kw} ")) || text::starts_with_ignore_case(trimmed, &format!("{kw}:")) {
                let term: String = trimmed.chars().skip(kw.chars().count() + 1).collect::<String>().trim().to_string();
                return if term.is_empty() { None } else { Some(term) };
            }
        }

        None
    }
}

/// Yazım denetimi: "spell <kelime>" veya "yazım <kelime>". Enter öneriyi panoya kopyalar.
pub struct SpellCheckSource {
    pub base: SearchSourceBase,
    service: Arc<dyn ISpellCheckerService>,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
}

const MAX_SUGGESTIONS: usize = 8;

impl SpellCheckSource {
    pub fn new(service: Arc<dyn ISpellCheckerService>, shell: Arc<dyn ISearchShell>, t: Translate) -> Self {
        let name = t("SearchCatSpellCheck");
        SpellCheckSource { base: SearchSourceBase::new("spell", name), service, shell, t }
    }
}

impl ISearchSource for SpellCheckSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {
        self.service.release();
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let Some(term) = SearchPrefix::try_extract(&query.raw, &(self.t)("SearchKwSpellCheck")) else {
            return Vec::new();
        };

        let check = self.service.check(&term);
        if !check.is_supported {
            return Vec::new();
        }

        let copy_to = |text: String| {
            let shell = Arc::clone(&self.shell);
            move || shell.copy_text(&text)
        };

        if check.is_correct {
            let mut result =
                SearchResult::new(format!("spell:ok:{term}"), self.base.name(), format!("✓ {term}"), copy_to(term.clone()));
            result.subtitle = Some((self.t)("SearchSpellCorrect"));
            result.glyph = SearchGlyph::SpellCheck;
            result.category_relevance = SearchRelevance::HIGHEST;
            result.relevance = 1.0;
            result.restores_focus = true;
            result.drag = Some(SearchDragData::new(SearchDragKind::Text, term));
            return vec![result];
        }

        if check.suggestions.is_empty() {
            let mut result = SearchResult::new(format!("spell:none:{term}"), self.base.name(), format!("✗ {term}"), || {});
            result.subtitle = Some((self.t)("SearchSpellNoSuggestions"));
            result.glyph = SearchGlyph::SpellCheck;
            result.category_relevance = SearchRelevance::HIGHEST;
            result.relevance = 0.8;
            return vec![result];
        }

        check
            .suggestions
            .iter()
            .take(MAX_SUGGESTIONS)
            .enumerate()
            .map(|(i, suggestion)| {
                let mut result = SearchResult::new(
                    format!("spell:{suggestion}"),
                    self.base.name(),
                    suggestion.clone(),
                    copy_to(suggestion.clone()),
                );
                result.subtitle = Some(format!(
                    "{} · {}",
                    format_template(&(self.t)("SearchSpellSuggestionFor"), &[&term]),
                    (self.t)("SearchHintCopy")
                ));
                result.glyph = SearchGlyph::SpellCheck;
                result.category_relevance = SearchRelevance::HIGHEST;
                result.relevance = 1.0 - (i as f64 * 0.05);
                result.restores_focus = true;
                result.drag = Some(SearchDragData::new(SearchDragKind::Text, suggestion.clone()));
                result
            })
            .collect()
    }
}

const MAX_MEANINGS: usize = 6;
const DEBOUNCE_MS: u64 = 220;

/// Arka plan görevi ile arayüzün paylaştığı durum (C#'taki `_gate` ile korunan alanlar).
struct DictState {
    /// Çalışan görevin kimliği ve iptal kaynağı (`_running`).
    running: Option<(u64, CancellationTokenSource)>,
    next_id: u64,
    pending: String,
    ready: String,
    entry: Option<DictionaryEntry>,
}

struct DictInner {
    service: Arc<dyn IDictionaryService>,
    changed: Arc<Event<()>>,
    state: Mutex<DictState>,
}

/// Sözlük araması: "dict <kelime>" veya "sözlük <kelime>". Boşta ağ isteği yapmaz, yazma durunca sorgular.
pub struct DictionarySource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    turkish: bool,
    inner: Arc<DictInner>,
}

impl DictionarySource {
    /// C# `culture = null` → `CurrentUICulture`: burada zorunlu parametre.
    pub fn new(service: Arc<dyn IDictionaryService>, shell: Arc<dyn ISearchShell>, t: Translate, culture: Culture) -> Self {
        let name = t("SearchCatDictionary");
        let base = SearchSourceBase::new("dictionary", name);
        let inner = Arc::new(DictInner {
            service,
            changed: base.changed_handle(),
            state: Mutex::new(DictState {
                running: None,
                next_id: 0,
                pending: String::new(),
                ready: String::new(),
                entry: None,
            }),
        });
        DictionarySource {
            base,
            shell,
            t,
            turkish: text::eq_ignore_case(&two_letter_language_name(&culture), "tr"),
            inner,
        }
    }

    fn build_results(&self, entry: &DictionaryEntry) -> Vec<SearchResult> {
        let open_url = entry.source_url.clone().unwrap_or_else(|| self.online_url(&entry.word));
        let count = entry.meanings.len().min(MAX_MEANINGS);
        let mut results = Vec::new();
        for i in 0..count {
            let meaning = &entry.meanings[i];
            let header = match entry.phonetic.as_deref() {
                Some(p) if !text::is_null_or_white_space(Some(p)) => format!("{} {} ({})", entry.word, p, meaning.part_of_speech),
                _ => format!("{} ({})", entry.word, meaning.part_of_speech),
            };
            let copy_text = format!("{}: {}", entry.word, meaning.definition);

            let shell = Arc::clone(&self.shell);
            let copy = copy_text.clone();
            let mut result = SearchResult::new(
                format!("dict:{}:{}", entry.word, i),
                self.base.name(),
                meaning.definition.clone(),
                move || shell.copy_text(&copy),
            );
            result.subtitle = Some(match meaning.example.as_deref() {
                Some(e) if !text::is_null_or_white_space(Some(e)) => format!("{header} · \"{e}\""),
                _ => header,
            });
            result.glyph = SearchGlyph::Dictionary;
            result.category_relevance = SearchRelevance::HIGHEST;
            result.relevance = 1.0 - (i as f64 * 0.05);
            result.restores_focus = true;
            result.drag = Some(SearchDragData::new(SearchDragKind::Text, copy_text));
            let shell = Arc::clone(&self.shell);
            let url = open_url.clone();
            result.actions = vec![SearchAction::new("open", (self.t)("SearchActOpenWeb"), move || shell.open(&url))];
            results.push(result);
        }
        results
    }

    fn online_fallback(&self, word: &str) -> SearchResult {
        let url = self.online_url(word);
        let shell = Arc::clone(&self.shell);
        let open = url.clone();
        let mut result = SearchResult::new(
            format!("dict:web:{word}"),
            self.base.name(),
            format_template(&(self.t)("SearchDictWebFor"), &[word]),
            move || shell.open(&open),
        );
        result.subtitle = Some(url.clone());
        result.glyph = SearchGlyph::Dictionary;
        result.category_relevance = SearchRelevance::HIGH;
        result.relevance = 0.7;
        result.drag = Some(SearchDragData::new(SearchDragKind::Url, url));
        result
    }

    fn online_url(&self, word: &str) -> String {
        let escaped = escape_data_string(word);
        if self.turkish {
            format!("https://sozluk.gov.tr/?ara={escaped}")
        } else {
            format!("https://www.merriam-webster.com/dictionary/{escaped}")
        }
    }

    fn cancel_locked(state: &mut DictState) {
        if let Some((_, cts)) = state.running.take() {
            cts.cancel();
        }
        state.pending = String::new();
    }

    fn start_locked(&self, state: &mut DictState, word: &str) {
        if let Some((_, cts)) = state.running.take() {
            cts.cancel();
        }
        let cts = CancellationTokenSource::new();
        state.next_id += 1;
        let id = state.next_id;
        let token = cts.token();
        state.running = Some((id, cts));
        state.pending = word.to_string();

        let inner = Arc::clone(&self.inner);
        let word = word.to_string();
        std::thread::spawn(move || Self::run(inner, word, id, token));
    }

    /// C# `RunAsync`.
    fn run(inner: Arc<DictInner>, word: String, id: u64, token: CancellationToken) {
        if !sleep_unless_canceled(DEBOUNCE_MS, &token) {
            return;
        }

        let outcome = catch_unwind(AssertUnwindSafe(|| inner.service.lookup(&word, &token)));
        let failure: ServiceError = match outcome {
            Ok(Ok(entry)) => {
                if token.is_cancellation_requested() {
                    return;
                }

                {
                    let mut state = inner.state.lock().unwrap();
                    if state.running.as_ref().map(|(i, _)| *i) != Some(id) {
                        return;
                    }
                    state.ready = word;
                    state.entry = entry;
                    state.pending = String::new();
                    state.running = None;
                }

                inner.changed.invoke(&());
                return;
            }
            Ok(Err(e)) => e,
            Err(payload) => panic_message(payload.as_ref()).into(),
        };

        // Yeni sorgu geldi veya panel kapandı.
        if is_canceled(&failure) {
            return;
        }

        log_service_error("Sözlük araması", &failure);
        {
            let mut state = inner.state.lock().unwrap();
            if state.running.as_ref().map(|(i, _)| *i) == Some(id) {
                state.ready = word;
                state.entry = None;
                state.pending = String::new();
                state.running = None;
            }
        }

        inner.changed.invoke(&());
    }
}

impl ISearchSource for DictionarySource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        self.inner.state.lock().unwrap().running.is_some()
    }

    fn prepare(&self) {}

    fn release(&self) {
        let mut state = self.inner.state.lock().unwrap();
        Self::cancel_locked(&mut state);
        state.ready = String::new();
        state.entry = None;
    }

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let Some(word) = SearchPrefix::try_extract(&query.raw, &(self.t)("SearchKwDictionary")) else {
            Self::cancel_locked(&mut self.inner.state.lock().unwrap());
            return Vec::new();
        };

        let (entry, ready) = {
            let mut state = self.inner.state.lock().unwrap();
            if !text::eq_ignore_case(&state.ready, &word) && !text::eq_ignore_case(&state.pending, &word) {
                self.start_locked(&mut state, &word);
            }

            (state.entry.clone(), text::eq_ignore_case(&state.ready, &word))
        };

        if ready {
            if let Some(entry) = entry.filter(|e| !e.meanings.is_empty()) {
                return self.build_results(&entry);
            }
            return vec![self.online_fallback(&word)];
        }

        Vec::new()
    }
}

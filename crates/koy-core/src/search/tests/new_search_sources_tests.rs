use super::support::{q, translate, FakeShell, Signal};
use crate::cancel::CancellationToken;
use crate::clock::Culture;
use crate::jump_list::{IJumpListService, JumpListEntry, JumpListLaunch, JumpListSection, JumpListSectionKind};
use crate::search::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn en() -> Culture {
    Culture::en_us()
}

fn tr() -> Culture {
    Culture::tr_tr()
}

fn t() -> Translate {
    translate(|key| {
        let text = match key {
            "SearchKwSpecialChars" => "char;sym;sembol",
            "SearchKwSpellCheck" => "spell;yazım",
            "SearchKwDictionary" => "dict;def;sözlük",
            "SearchKwStore" => "store;mağaza",
            "SearchSpellSuggestionFor" => "Suggestion for \"{0}\"",
            "SearchDictWebFor" => "Look up \"{0}\" online",
            "SearchStoreFor" => "Search Microsoft Store for \"{0}\"",
            _ => key,
        };
        text.to_string()
    })
}

struct FakeSpellChecker {
    check: Box<dyn Fn(&str) -> SpellCheckResult + Send + Sync>,
    released: Mutex<bool>,
}

impl FakeSpellChecker {
    fn new(check: impl Fn(&str) -> SpellCheckResult + Send + Sync + 'static) -> Arc<FakeSpellChecker> {
        Arc::new(FakeSpellChecker { check: Box::new(check), released: Mutex::new(false) })
    }
}

impl ISpellCheckerService for FakeSpellChecker {
    fn check(&self, text: &str) -> SpellCheckResult {
        (self.check)(text)
    }

    fn release(&self) {
        *self.released.lock().unwrap() = true;
    }
}

struct FakeDictionary {
    entry: Option<DictionaryEntry>,
    queries: Mutex<Vec<String>>,
}

impl FakeDictionary {
    fn new(entry: Option<DictionaryEntry>) -> Arc<FakeDictionary> {
        Arc::new(FakeDictionary { entry, queries: Mutex::new(Vec::new()) })
    }
}

impl IDictionaryService for FakeDictionary {
    fn lookup(&self, word: &str, _cancel: &CancellationToken) -> Result<Option<DictionaryEntry>, ServiceError> {
        self.queries.lock().unwrap().push(word.to_string());
        Ok(self.entry.clone())
    }
}

struct FakeApps(Vec<InstalledApp>);

impl IInstalledAppsService for FakeApps {
    fn load(&self, _cancel: &CancellationToken) -> Result<Vec<InstalledApp>, ServiceError> {
        Ok(self.0.clone())
    }
}

struct FakeJumpList {
    data: HashMap<String, Vec<JumpListSection>>,
    launched: Mutex<Vec<String>>,
}

impl IJumpListService for FakeJumpList {
    fn get(&self, app_user_model_id: &str) -> Vec<JumpListSection> {
        self.data.get(app_user_model_id).cloned().unwrap_or_default()
    }

    fn resolve_app_id(&self, _exe_path: &str) -> Option<String> {
        None
    }

    fn launch(&self, entry: &JumpListEntry, _app_exe_path: Option<&str>) {
        self.launched.lock().unwrap().push(entry.title.clone());
    }

    fn clear(&self) {}
}

fn watch(source: &dyn ISearchSource) -> Signal {
    let signal = Signal::new();
    let handle = signal.handle();
    source.changed().subscribe(move |_| handle.set());
    signal
}

fn query(text: &str) -> SearchQuery {
    q(text)
}

// --- Special characters ---

#[test]
fn special_characters_keyword_alone_lists_catalog_and_copies_on_execute() {
    let shell = FakeShell::new();
    let source = SpecialCharactersSource::new(shell.clone(), t(), en());

    let results = source.search(&query("char"));

    assert!(!results.is_empty());
    let first = &results[0];
    assert_eq!(Some(SearchDragKind::Text), first.drag.as_ref().map(|d| d.kind));
    assert!(first.restores_focus);
    (first.execute)();
    assert_eq!(vec![format!("copy {}", first.drag.as_ref().unwrap().value)], shell.log());
}

#[test]
fn special_characters_keyword_filter_and_direct_lookup_find_expected_symbols() {
    let shell = FakeShell::new();
    let source = SpecialCharactersSource::new(shell, t(), tr());

    let arrow_results = source.search(&query("sembol:ok"));
    assert!(arrow_results
        .iter()
        .any(|r| r.drag.as_ref().map(|d| d.value.as_str()) == Some("→") && r.category_relevance == SearchRelevance::HIGHEST));

    let euro_results = source.search(&query("euro"));
    assert!(euro_results.iter().any(|r| r.drag.as_ref().map(|d| d.value.as_str()) == Some("€")));

    let code_point_results = source.search(&query("U+2192"));
    assert!(code_point_results.iter().any(|r| r.drag.as_ref().map(|d| d.value.as_str()) == Some("→")));
}

// --- Spell check ---

#[test]
fn spell_check_ignores_queries_without_prefix() {
    let spell = FakeSpellChecker::new(|_| SpellCheckResult::new(true, false, vec!["receive".to_string()]));
    let source = SpellCheckSource::new(spell, FakeShell::new(), t());

    assert!(source.search(&query("recieve")).is_empty());
}

#[test]
fn spell_check_returns_suggestions_and_copies_on_execute() {
    let shell = FakeShell::new();
    let spell = FakeSpellChecker::new(|_| SpellCheckResult::new(true, false, vec!["receive".to_string(), "relieve".to_string()]));
    let source = SpellCheckSource::new(spell.clone(), shell.clone(), t());

    let results = source.search(&query("spell recieve"));

    assert_eq!(2, results.len());
    assert_eq!("receive", results[0].title);
    assert_eq!(Some(SearchDragData::new(SearchDragKind::Text, "receive")), results[0].drag);
    (results[0].execute)();
    assert_eq!(vec!["copy receive"], shell.log());

    source.release();
    assert!(*spell.released.lock().unwrap());
}

#[test]
fn spell_check_correct_word_shows_checkmark_and_copies() {
    let shell = FakeShell::new();
    let spell = FakeSpellChecker::new(|_| SpellCheckResult::new(true, true, vec![]));
    let source = SpellCheckSource::new(spell, shell.clone(), t());

    let mut results = source.search(&query("yazım:merhaba"));
    assert_eq!(1, results.len());
    let result = results.remove(0);

    assert_eq!("✓ merhaba", result.title);
    (result.execute)();
    assert_eq!(vec!["copy merhaba"], shell.log());
}

// --- Dictionary ---

#[test]
fn dictionary_debounced_lookup_returns_meanings_and_web_action() {
    let shell = FakeShell::new();
    let entry = DictionaryEntry::new(
        "koy",
        Some("/koj/".to_string()),
        vec![DictionaryMeaning::new("isim", "Küçük körfez", Some("Sakin bir koy".to_string()))],
        Some("https://sozluk.gov.tr/?ara=koy".to_string()),
    );
    let service = FakeDictionary::new(Some(entry));
    let source = DictionarySource::new(service, shell.clone(), t(), tr());
    let changed = watch(&source);

    assert!(source.search(&query("sözlük koy")).is_empty());
    changed.wait(3);

    let results = source.search(&query("sözlük koy"));
    assert_eq!(1, results.len());
    let first = &results[0];
    assert_eq!("Küçük körfez", first.title);
    assert_eq!(Some(SearchDragData::new(SearchDragKind::Text, "koy: Küçük körfez")), first.drag);

    (first.execute)();
    assert!(shell.log().contains(&"copy koy: Küçük körfez".to_string()));
    (first.actions[0].execute)();
    assert!(shell.log().contains(&"open https://sozluk.gov.tr/?ara=koy".to_string()));
}

#[test]
fn dictionary_falls_back_to_online_link_when_not_found() {
    let shell = FakeShell::new();
    let source = DictionarySource::new(FakeDictionary::new(None), shell.clone(), t(), en());
    let changed = watch(&source);

    assert!(source.search(&query("dict xyzzy")).is_empty());
    changed.wait(3);

    let mut results = source.search(&query("dict xyzzy"));
    assert_eq!(1, results.len());
    let result = results.remove(0);
    assert_eq!(Some(SearchDragKind::Url), result.drag.as_ref().map(|d| d.kind));
    (result.execute)();
    assert_eq!(vec!["open https://www.merriam-webster.com/dictionary/xyzzy"], shell.log());
}

// --- Jump List ---

#[test]
fn jump_list_lazily_fetches_matching_app_and_launches_entry() {
    let app_id = "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App";
    let apps = Arc::new(FakeApps(vec![InstalledApp::new(app_id, "Windows Terminal")]));
    let mut data = HashMap::new();
    data.insert(
        app_id.to_string(),
        vec![JumpListSection::new(
            JumpListSectionKind::Tasks,
            None,
            vec![
                JumpListEntry::separator(),
                JumpListEntry::new("PowerShell", Some(JumpListLaunch::new("pwsh.exe"))),
                JumpListEntry::new("Docs", Some(JumpListLaunch::new("https://learn.microsoft.com"))),
            ],
        )],
    );
    let jump = Arc::new(FakeJumpList { data, launched: Mutex::new(Vec::new()) });

    let source = JumpListSearchSource::new(apps, jump.clone(), t(), None);
    source.prepare();
    std::thread::sleep(std::time::Duration::from_millis(50));

    // C# testinde `changed` uygulama listesi olayıyla zaten tamamlanmış olabilir ve geçerli yarış orada daha seyrek
    // (Task.FromResult eşzamanlı); Rust'ta tembel yükleme olayı için dinleyici uygulama listesi yüklendikten sonra kurulur.
    let changed = watch(&source);

    // İlk aramada tembel yükleme tetiklenir.
    let _ = source.search(&query("terminal"));
    changed.wait(3);

    let results = source.search(&query("terminal powershell"));
    let ps: Vec<&SearchResult> = results.iter().filter(|r| r.title == "PowerShell").collect();
    assert_eq!(1, ps.len());
    (ps[0].execute)();
    assert_eq!(vec!["PowerShell"], *jump.launched.lock().unwrap());

    let all_for_terminal = source.search(&query("terminal"));
    let docs: Vec<&SearchResult> = all_for_terminal.iter().filter(|r| r.title == "Docs").collect();
    assert_eq!(1, docs.len());
    assert_eq!(Some(SearchDragData::new(SearchDragKind::Url, "https://learn.microsoft.com")), docs[0].drag);
}

// --- Microsoft Store ---

#[test]
fn microsoft_store_builds_protocol_uri_and_web_drag_url() {
    let shell = FakeShell::new();
    let source = MicrosoftStoreSearchSource::new(shell.clone(), t());

    assert!(source.search(&query("terminal")).is_empty());

    let mut results = source.search(&query("store windows terminal"));
    assert_eq!(1, results.len());
    let result = results.remove(0);
    assert_eq!(
        Some(SearchDragData::new(SearchDragKind::Url, "https://apps.microsoft.com/search?q=windows%20terminal")),
        result.drag
    );

    (result.execute)();
    assert!(shell.log().contains(&"open ms-windows-store://search/?query=windows%20terminal".to_string()));
    (result.actions[0].execute)();
    assert!(shell.log().contains(&"open https://apps.microsoft.com/search?q=windows%20terminal".to_string()));
}

// --- Drag payloads on existing sources ---

#[test]
fn existing_sources_expose_drag_payloads() {
    let shell = FakeShell::new();
    shell.add_file(r"C:\test\notes.txt");

    let calc = CalculatorSearchSource::new(shell.clone(), t(), en()).search(&query("2+2")).remove(0);
    assert_eq!(Some(SearchDragData::new(SearchDragKind::Text, "4")), calc.drag);

    let file = LocationSearchSource::new(shell.clone(), t()).search(&query(r"C:\test\notes.txt")).remove(0);
    assert_eq!(Some(SearchDragData::new(SearchDragKind::File, r"C:\test\notes.txt")), file.drag);

    let url = LocationSearchSource::new(shell, t()).search(&query("https://example.com")).remove(0);
    assert_eq!(Some(SearchDragData::new(SearchDragKind::Url, "https://example.com/")), url.drag);
}

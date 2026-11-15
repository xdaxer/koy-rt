use crate::cancel::CancellationToken;
use crate::event::Event;
use std::sync::Arc;

use super::SearchQuery;

/// `Func<string, string> t`: yerelleştirme anahtarından metin.
pub type Translate = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// Bir iş parçacığına geçirilen hata (C# `Exception`).
pub type ServiceError = Box<dyn std::error::Error + Send + Sync>;

/// Kategorinin önemi; listedeki grup sırasını belirler. Grup içi sıra [`SearchResult::relevance`] ile.
pub struct SearchRelevance;

impl SearchRelevance {
    pub const LOWEST: i32 = 0;
    pub const LOW: i32 = 30;
    pub const MODERATE: i32 = 50;
    pub const HIGH: i32 = 70;
    pub const HIGHEST: i32 = 100;
}

/// Kategorinin listedeki yeri; önemden önce gelir: uygulamalar hep üstte, dosyalar hep altta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum SearchCategoryPlacement {
    Top,
    #[default]
    Normal,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SearchGlyph {
    #[default]
    App,
    Window,
    Calculator,
    Terminal,
    Power,
    Folder,
    File,
    Globe,
    Bookmark,
    Process,
    Clipboard,
    Clock,
    Help,
    History,
    Symbol,
    SpellCheck,
    Dictionary,
    Store,
}

/// Panelden dışarı sürüklenebilen veri türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SearchDragKind {
    #[default]
    None,
    File,
    Url,
    Text,
}

/// Sürükle-bırak yükü.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDragData {
    pub kind: SearchDragKind,
    pub value: String,
}

impl SearchDragData {
    pub fn new(kind: SearchDragKind, value: impl Into<String>) -> Self {
        SearchDragData { kind, value: value.into() }
    }
}

/// C#'taki `Action` temsilcisi.
pub type ActionFn = Arc<dyn Fn() + Send + Sync>;

/// Satırın ek eylemi (ör. "Kopyala", "Bulunduğu klasörü aç").
#[derive(Clone)]
pub struct SearchAction {
    pub id: String,
    pub label: String,
    pub execute: ActionFn,
    /// İlk Enter yalnız sorar, ikinci Enter eylemi çalıştırır.
    pub requires_confirmation: bool,
}

impl SearchAction {
    pub fn new(id: impl Into<String>, label: impl Into<String>, execute: impl Fn() + Send + Sync + 'static) -> Self {
        SearchAction { id: id.into(), label: label.into(), execute: Arc::new(execute), requires_confirmation: false }
    }
}

impl std::fmt::Debug for SearchAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SearchAction({})", self.id)
    }
}

/// Listede bir satır. C#'taki `init` özellikleri herkese açık alandır; varsayılanlar [`SearchResult::new`]'de.
#[derive(Clone)]
pub struct SearchResult {
    pub id: String,
    pub category: String,
    pub title: String,
    pub execute: ActionFn,
    pub subtitle: Option<String>,
    pub placement: SearchCategoryPlacement,
    pub category_relevance: i32,
    pub relevance: f64,
    pub glyph: SearchGlyph,
    pub actions: Vec<SearchAction>,
    /// İlk Enter yalnız sorar, ikinci Enter çalıştırır (kapat, yeniden başlat).
    pub requires_confirmation: bool,
    /// Onay satırında gösterilecek tam metin (ör. çalıştırılacak komut, satır sonları ⏎ ile).
    pub confirmation_detail: Option<String>,
    /// Panel kapanınca odak önceki pencereye dönsün: kopyalanan sonucu oraya yapıştırmak için.
    pub restores_focus: bool,
    /// Doluysa Enter panel kapatmaz, sorguyu bu metinle değiştirir (hesap sonucu, yardım).
    pub replaces_query: Option<String>,
    /// Çalıştırılınca arama geçmişine yazılmasın.
    pub skip_history: bool,
    /// Simge yalnız görünen satırlar için, arka planda çözülür.
    pub load_icon: Option<Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>>,
    /// Doluysa satır panelden dışarı sürüklenebilir (dosya, adres ya da metin).
    pub drag: Option<SearchDragData>,
}

impl SearchResult {
    /// C# kurucusu `SearchResult(id, category, title, execute)`; öteki alanlar C# varsayılanlarıyla:
    /// `Placement = Normal`, `CategoryRelevance = Moderate`, `Relevance = 0.5`, `Glyph = App`.
    pub fn new(
        id: impl Into<String>,
        category: impl Into<String>,
        title: impl Into<String>,
        execute: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        SearchResult {
            id: id.into(),
            category: category.into(),
            title: title.into(),
            execute: Arc::new(execute),
            subtitle: None,
            placement: SearchCategoryPlacement::Normal,
            category_relevance: SearchRelevance::MODERATE,
            relevance: 0.5,
            glyph: SearchGlyph::App,
            actions: Vec::new(),
            requires_confirmation: false,
            confirmation_detail: None,
            restores_focus: false,
            replaces_query: None,
            skip_history: false,
            load_icon: None,
            drag: None,
        }
    }
}

impl std::fmt::Debug for SearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SearchResult({}, {})", self.id, self.title)
    }
}

/// Yardımda gösterilen kullanım örneği.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSyntax {
    pub example: String,
    pub description: String,
}

impl SearchSyntax {
    pub fn new(example: impl Into<String>, description: impl Into<String>) -> Self {
        SearchSyntax { example: example.into(), description: description.into() }
    }
}

/// Sonuç kaynağı. Boşta iş yapmaz: [`ISearchSource::prepare`] panel açılınca, [`ISearchSource::release`] kapanınca
/// çağrılır.
///
/// C# `IEnumerable<SearchResult>` dönüşü burada hevesli `Vec`'tir. İş parçacığı güvenli olmalıdır: `changed`
/// arka plan iş parçacığından gelebilir.
pub trait ISearchSource: Send + Sync {
    /// Ayar anahtarı; değişmez.
    fn id(&self) -> &str;

    /// Kullanıcıya görünen ad.
    fn name(&self) -> &str;

    fn syntaxes(&self) -> &[SearchSyntax];

    /// Sonuç arka planda aranıyor; biraz sonra [`ISearchSource::changed`] gelecek.
    fn is_busy(&self) -> bool;

    /// Sonuçlar değişti (ör. uygulama listesi hazır). Arka plan iş parçacığından gelebilir.
    fn changed(&self) -> &Event<()>;

    fn prepare(&self);

    fn release(&self);

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult>;
}

/// C# `abstract class SearchSource` (kimlik, ad, sözdizimleri, `Changed`): kaynaklar bunu alan olarak taşır ve
/// [`search_source_base_methods`] makrosuyla trait'in ortak üyelerini uygular.
pub struct SearchSourceBase {
    id: String,
    name: String,
    /// C#'taki `Syntaxes { get; init; }`.
    pub syntaxes: Vec<SearchSyntax>,
    changed: Arc<Event<()>>,
}

impl SearchSourceBase {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        SearchSourceBase { id: id.into(), name: name.into(), syntaxes: Vec::new(), changed: Arc::new(Event::new()) }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn syntaxes(&self) -> &[SearchSyntax] {
        &self.syntaxes
    }

    pub fn changed(&self) -> &Event<()> {
        &self.changed
    }

    /// Arka plan iş parçacığının `RaiseChanged()` çağırabilmesi için olay tutacağı.
    pub fn changed_handle(&self) -> Arc<Event<()>> {
        Arc::clone(&self.changed)
    }

    /// C# `RaiseChanged()`.
    pub fn raise_changed(&self) {
        self.changed.invoke(&());
    }
}

/// `ISearchSource` içinde `id/name/syntaxes/changed` üyelerini `self.<alan>` taban nesnesine yönlendirir.
#[macro_export]
macro_rules! search_source_base_methods {
    ($field:ident) => {
        fn id(&self) -> &str {
            self.$field.id()
        }

        fn name(&self) -> &str {
            self.$field.name()
        }

        fn syntaxes(&self) -> &[$crate::search::SearchSyntax] {
            self.$field.syntaxes()
        }

        fn changed(&self) -> &$crate::event::Event<()> {
            self.$field.changed()
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SystemCommand {
    #[default]
    Lock,
    Sleep,
    SignOut,
    SwitchUser,
    Restart,
    Shutdown,
}

impl SystemCommand {
    /// `entry.Command.ToString()` (kimlikte kullanılır).
    pub fn name(self) -> &'static str {
        match self {
            SystemCommand::Lock => "Lock",
            SystemCommand::Sleep => "Sleep",
            SystemCommand::SignOut => "SignOut",
            SystemCommand::SwitchUser => "SwitchUser",
            SystemCommand::Restart => "Restart",
            SystemCommand::Shutdown => "Shutdown",
        }
    }
}

pub trait ISystemCommandService: Send + Sync {
    fn execute(&self, command: SystemCommand);
}

/// Başlat menüsündeki bir uygulama. `id`: `shell:AppsFolder` altındaki kimlik.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
}

impl InstalledApp {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        InstalledApp { id: id.into(), name: name.into() }
    }
}

pub trait IInstalledAppsService: Send + Sync {
    /// Yavaş olabilir (yaklaşık 1 sn); arka plan iş parçacığında çağrılır, UI iş parçacığını bekletmez.
    /// C# `LoadAsync`: eşzamanlı; iptal edilirse `Err(OperationCanceled)`.
    fn load(&self, cancel: &CancellationToken) -> Result<Vec<InstalledApp>, ServiceError>;
}

/// Windows Arama dizininden bir dosya ya da klasör.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedItem {
    pub path: String,
    pub is_folder: bool,
}

impl IndexedItem {
    pub fn new(path: impl Into<String>, is_folder: bool) -> Self {
        IndexedItem { path: path.into(), is_folder }
    }
}

pub trait IFileSearchService: Send + Sync {
    /// Dizin kapalıysa boş döner, hata vermez. C# `SearchAsync`: arka plan iş parçacığında çağrılır.
    fn search(&self, term: &str, limit: i32, cancel: &CancellationToken) -> Result<Vec<IndexedItem>, ServiceError>;
}

/// Bilinen klasör ya da sürücü.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPlace {
    pub name: String,
    pub path: String,
    pub group: Option<String>,
}

impl SearchPlace {
    /// C# `SearchPlace(Name, Path, Group = null)`: `group` zorunlu parametre.
    pub fn new(name: impl Into<String>, path: impl Into<String>, group: Option<String>) -> Self {
        SearchPlace { name: name.into(), path: path.into(), group }
    }
}

/// `start_ticks`: Başlangıç zamanı (UTC tik); 0 bilinmiyor. Kimlik yeniden kullanılırsa başka süreç öldürülmesin diye.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningProcess {
    pub id: i32,
    pub name: String,
    pub start_ticks: i64,
}

impl RunningProcess {
    /// C# `RunningProcess(Id, Name, StartTicks = 0)`: `start_ticks` zorunlu parametre.
    pub fn new(id: i32, name: impl Into<String>, start_ticks: i64) -> Self {
        RunningProcess { id, name: name.into(), start_ticks }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserBookmark {
    pub title: String,
    pub url: String,
}

impl BrowserBookmark {
    pub fn new(title: impl Into<String>, url: impl Into<String>) -> Self {
        BrowserBookmark { title: title.into(), url: url.into() }
    }
}

/// Sonuçların dışarıya dokunan işleri; Core'daki kaynaklar Windows'a doğrudan bağlanmaz.
pub trait ISearchShell: Send + Sync {
    fn file_exists(&self, path: &str) -> bool;

    fn directory_exists(&self, path: &str) -> bool;

    fn get_environment_variable(&self, name: &str) -> Option<String>;

    /// Dosyayı, klasörü ya da adresi varsayılan uygulamayla açar.
    fn open(&self, target: &str);

    fn run(&self, executable: &str, arguments: Option<&str>);

    /// Kabuksuz çalıştırır; her argüman olduğu gibi tek parça gider (tırnak kaçışı gerekmez).
    fn run_with_arguments(&self, executable: &str, arguments: &[String]);

    fn run_in_terminal(&self, command_line: &str);

    fn open_containing_folder(&self, path: &str);

    fn copy_text(&self, text: &str);
}

/// Arka plan iş parçacığında hata yazma ortak kısayolu (`CrashLog.Write(source, ex)`).
pub(crate) fn log_service_error(source: &str, error: &ServiceError) {
    use crate::diagnostics::{CrashError, CrashLog};
    CrashLog::write(source, &CrashError::from_error(error.as_ref(), "Exception", ""));
}

/// `catch (OperationCanceledException)`.
pub(crate) fn is_canceled(error: &ServiceError) -> bool {
    error.downcast_ref::<crate::cancel::OperationCanceled>().is_some()
}

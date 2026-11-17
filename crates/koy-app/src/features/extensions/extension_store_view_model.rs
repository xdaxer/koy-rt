//! Karşılığı: `koy/src/App/Features/Extensions/ExtensionStoreViewModel.cs`, `StoreCardViewModel.cs`,
//! `BuiltinBackgroundCardViewModel.cs`.
//!
//! Eklentiler sayfası: mağaza indeksini ve kurulu eklentileri listeler, kurar, günceller, uygular ve kaldırır. Mantığın
//! hepsi Core'dadır (`StoreListState`, `ExtensionApplyPlanner`, kurulum denetimleri); ağ ve dosya işi
//! `ExtensionStoreClient` ile `StorePreviewLoader` içindedir. Burası durumu, onay adımını ve metinleri bağlar. Sayfa
//! görünürken indeksi bir kez çeker; arka planda yoklama yoktur. Ağ ve kurulum işleri arka plan iş parçacığında, sonuç
//! arayüz turunda yazılır.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use koy_core::cancel::{CancellationToken, CancellationTokenSource};
use koy_core::extensions::store::{
    ExtensionApplyPlan, ExtensionApplyPlanner, ExtensionFileActionKind, ExtensionInstallError, ExtensionKind, ExtensionPaths,
    IExtensionStoreClient, InstallResult, InstalledEntry, PackageEntryPolicy, PackageManifest, PackageManifestParser, StoreEntryView,
    StoreFilter, StoreIndexError, StoreItem, StoreItemStatus, StoreListState, StoreQuery, StoreSort,
};
use koy_core::notch::{LiveBackgroundPresets, LiveBackgroundSourceKind, LiveBackgroundTarget};
use koy_core::settings::AppSettings;
use koy_core::system_stats::SystemFormat;
use koy_platform::extensions::{ExtensionStoreClient, StorePreviewLoader};

use crate::features::settings::profile_service::ProfileService;
use crate::infra::observable::{LocalEvent, Notifier};
use crate::infra::{dispatch, ui_ref::UiRef};
use crate::resources::strings::{self, Strings};

use super::extension_file_action_runner::{ExtensionFileActionRunner, ExtensionFileActionResult, UserStyleSnapshot};

/// Listede bir seferde gösterilen kart sayısı ("Daha fazla göster" bu kadar ekler).
pub const PAGE_SIZE: usize = 20;
const PREVIEW_DECODE_PIXELS: u32 = 320;
const MAX_PARALLEL_PREVIEWS: usize = 3;

/// Kartın kalıcı (yeniden kurulmada korunan) durumu.
#[derive(Default, Clone)]
struct CardExtra {
    message: String,
    message_is_error: bool,
    progress: f64,
    is_expanded: bool,
    is_uninstall_confirming: bool,
    preview: Option<slint::Image>,
    preview_requested: bool,
}

/// Mağaza kartı (görünüm verisi).
#[derive(Clone)]
pub struct StoreCard {
    pub id: String,
    pub kind: ExtensionKind,
    pub name: String,
    pub meta_text: String,
    pub kind_label: String,
    pub summary: String,
    pub description: String,
    pub details_text: String,
    pub has_homepage: bool,
    pub is_busy: bool,
    pub show_install: bool,
    pub show_update: bool,
    pub show_apply: bool,
    pub is_applied: bool,
    pub show_uninstall: bool,
    pub apply_label: String,
    pub progress: f64,
    pub is_uninstall_confirming: bool,
    pub is_expanded: bool,
    pub message: String,
    pub message_is_error: bool,
    pub preview: Option<slint::Image>,
}

/// Yerleşik canlı arka plan kartı (mağaza indirmesi gerektirmez).
#[derive(Clone)]
pub struct BuiltinCard {
    pub id: String,
    pub name: String,
    pub kind_label: String,
    pub colors: Vec<String>,
    pub is_applied: bool,
}

#[derive(Clone)]
pub struct ConfirmFact {
    pub label: String,
    pub value: String,
}

enum PendingInstall {
    Store { id: String, item: StoreItem, update: bool },
    File { path: String },
}

#[derive(Default)]
struct View {
    search_text: String,
    category_key: String,
    filter_key: String,
    sort_key: String,
    is_loading: bool,
    index_error_text: String,
    status_text: String,
    status_is_error: bool,
    updates_label: String,
    count_text: String,
    show_more_text: String,
    is_installing_file: bool,
    is_confirming: bool,
    confirm_title: String,
    confirm_button: String,
    confirm_file_note: String,
    confirm_facts: Vec<ConfirmFact>,
}

pub struct ExtensionStoreViewModel {
    client: Arc<ExtensionStoreClient>,
    previews: Arc<StorePreviewLoader>,
    profiles: Rc<ProfileService>,
    runner: ExtensionFileActionRunner,
    state: RefCell<StoreListState>,
    entries: RefCell<Vec<StoreEntryView>>,
    cards: RefCell<Vec<StoreCard>>,
    extras: RefCell<HashMap<String, CardExtra>>,
    builtin: RefCell<Vec<BuiltinCard>>,
    busy: RefCell<HashSet<String>>,
    settings: RefCell<AppSettings>,
    view: RefCell<View>,
    active: Cell<bool>,
    display_count: Cell<usize>,
    skipped_items: Cell<i32>,
    index_cts: RefCell<Option<CancellationTokenSource>>,
    preview_cts: RefCell<Option<CancellationTokenSource>>,
    previews_running: Arc<AtomicUsize>,
    pending: RefCell<Option<PendingInstall>>,
    pub changed: Notifier,
    /// Profil kitaplığına bir profil eklendi (Genel sayfasındaki liste tazelensin).
    pub profiles_changed: LocalEvent<()>,
}

fn kind_label(kind: ExtensionKind) -> String {
    Strings::get(&format!("ExtKind{}", kind.name()))
}

fn current_language() -> String {
    if Strings::is_turkish() { "tr".into() } else { "en".into() }
}

impl ExtensionStoreViewModel {
    pub fn new(profiles: Rc<ProfileService>) -> Rc<Self> {
        let runner = ExtensionFileActionRunner::new(None, Rc::clone(&profiles));
        let me = Rc::new(ExtensionStoreViewModel {
            client: Arc::new(ExtensionStoreClient::new(None, None, None, None)),
            previews: Arc::new(StorePreviewLoader::new(None, None)),
            profiles,
            runner,
            state: RefCell::new(StoreListState::initial()),
            entries: RefCell::new(Vec::new()),
            cards: RefCell::new(Vec::new()),
            extras: RefCell::new(HashMap::new()),
            builtin: RefCell::new(Vec::new()),
            busy: RefCell::new(HashSet::new()),
            settings: RefCell::new(AppSettings::default()),
            view: RefCell::new(View {
                category_key: "All".into(),
                filter_key: "All".into(),
                sort_key: "Newest".into(),
                updates_label: Strings::get("ExtFilterUpdates"),
                ..Default::default()
            }),
            active: Cell::new(false),
            display_count: Cell::new(PAGE_SIZE),
            skipped_items: Cell::new(0),
            index_cts: RefCell::new(None),
            preview_cts: RefCell::new(None),
            previews_running: Arc::new(AtomicUsize::new(0)),
            pending: RefCell::new(None),
            changed: Notifier::default(),
            profiles_changed: LocalEvent::default(),
        });
        me
    }

    // ------------------------------------------------------------------ okuma

    pub fn cards(&self) -> Vec<StoreCard> {
        self.cards.borrow().clone()
    }

    pub fn builtin_cards(&self) -> Vec<BuiltinCard> {
        if self.show_builtin_backgrounds() { self.builtin.borrow().clone() } else { Vec::new() }
    }

    pub fn search_text(&self) -> String {
        self.view.borrow().search_text.clone()
    }

    pub fn category_index(&self) -> i32 {
        ["All", "Theme", "IconPack", "Background", "Profile"].iter().position(|k| *k == self.view.borrow().category_key).unwrap_or(0) as i32
    }

    pub fn filter_index(&self) -> i32 {
        ["All", "Installed", "Updates"].iter().position(|k| *k == self.view.borrow().filter_key).unwrap_or(0) as i32
    }

    pub fn sort_index(&self) -> i32 {
        ["Newest", "Alphabetical", "MostDownloads"].iter().position(|k| *k == self.view.borrow().sort_key).unwrap_or(0) as i32
    }

    /// (yükleniyor, indeks hatası, durum, durum hatası, güncellemeler etiketi, sayaç, daha fazla metni, daha fazla var mı,
    /// boş, dosyadan kurulabilir)
    pub fn texts(&self) -> (bool, String, String, bool, String, String, String, bool, bool, bool) {
        let v = self.view.borrow();
        let cards = self.cards.borrow().len();
        let has_more = self.entries.borrow().len() > cards;
        let show_builtin = self.show_builtin_backgrounds();
        let show_empty = !v.is_loading && cards == 0 && !show_builtin && v.index_error_text.is_empty();
        (
            v.is_loading && cards == 0,
            v.index_error_text.clone(),
            v.status_text.clone(),
            v.status_is_error,
            v.updates_label.clone(),
            v.count_text.clone(),
            v.show_more_text.clone(),
            has_more,
            show_empty,
            !v.is_installing_file,
        )
    }

    /// (onay açık, başlık, düğme, dosya notu, satırlar)
    pub fn confirm(&self) -> (bool, String, String, String, Vec<ConfirmFact>) {
        let v = self.view.borrow();
        (v.is_confirming, v.confirm_title.clone(), v.confirm_button.clone(), v.confirm_file_note.clone(), v.confirm_facts.clone())
    }

    fn show_builtin_backgrounds(&self) -> bool {
        let v = self.view.borrow();
        !self.builtin.borrow().is_empty() && (v.category_key == "All" || v.category_key == "Background") && v.filter_key != "Updates"
    }

    // ------------------------------------------------------------------ sorgu

    pub fn set_search_text(self: &Rc<Self>, t: &str) {
        self.view.borrow_mut().search_text = t.to_string();
        self.query_changed();
    }

    pub fn set_category(self: &Rc<Self>, index: usize) {
        let key = ["All", "Theme", "IconPack", "Background", "Profile"].get(index).copied().unwrap_or("All");
        self.view.borrow_mut().category_key = key.to_string();
        self.query_changed();
    }

    pub fn set_filter(self: &Rc<Self>, index: usize) {
        let key = ["All", "Installed", "Updates"].get(index).copied().unwrap_or("All");
        self.view.borrow_mut().filter_key = key.to_string();
        self.query_changed();
    }

    pub fn set_sort(self: &Rc<Self>, index: usize) {
        let key = ["Newest", "Alphabetical", "MostDownloads"].get(index).copied().unwrap_or("Newest");
        self.view.borrow_mut().sort_key = key.to_string();
        self.query_changed();
    }

    fn query_changed(self: &Rc<Self>) {
        self.display_count.set(PAGE_SIZE);
        self.rebuild();
        self.load_previews();
    }

    /// Başka bir sayfadaki "Mağazadan daha fazla al…" bağlantısı: sayfayı bu kategoriyle açar.
    pub fn show_category(self: &Rc<Self>, category: &str) {
        {
            let mut v = self.view.borrow_mut();
            v.search_text.clear();
            v.filter_key = "All".into();
            v.category_key = match kind_from_name(category) {
                Some(k) => k.name().to_string(),
                None => "All".into(),
            };
        }
        self.query_changed();
    }

    // ------------------------------------------------------------------ yaşam döngüsü

    /// Sayfa görünürken true: kayıt ve ayarlar okunur, indeks henüz çekilmediyse bir kez çekilir, görünen kartların
    /// önizlemesi istenir. false: bekleyen indirmeler iptal edilir, görseller bırakılır.
    pub fn set_active(self: &Rc<Self>, active: bool) {
        if active == self.active.get() {
            return;
        }
        self.active.set(active);
        if active {
            *self.settings.borrow_mut() = self.profiles.current_settings();
            self.state.borrow_mut().registry = self.client.registry().load();
            self.rebuild();
            if self.state.borrow().index.is_none() && !self.view.borrow().is_loading {
                self.load_index();
            } else {
                self.load_previews();
            }
        } else {
            if let Some(c) = self.index_cts.borrow().as_ref() {
                c.cancel();
            }
            if let Some(c) = self.preview_cts.borrow_mut().take() {
                c.cancel();
            }
            for e in self.extras.borrow_mut().values_mut() {
                e.preview = None;
                e.preview_requested = false;
            }
            self.rebuild_cards_only();
        }
    }

    pub fn refresh(self: &Rc<Self>) {
        if !self.view.borrow().is_loading {
            self.load_index();
        }
    }

    fn load_index(self: &Rc<Self>) {
        let cts = CancellationTokenSource::new();
        let token = cts.token();
        *self.index_cts.borrow_mut() = Some(cts);
        {
            let mut v = self.view.borrow_mut();
            v.is_loading = true;
            v.index_error_text.clear();
        }
        self.changed.notify();
        let client = Arc::clone(&self.client);
        let ui = UiRef::new(self);
        std::thread::spawn(move || {
            let result = client.fetch_index(&token);
            let registry = client.registry().load();
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                if result.is_success() {
                    me.skipped_items.set(result.skipped_items);
                    let mut st = me.state.borrow_mut();
                    st.index = result.index;
                    st.registry = registry;
                } else if result.error != StoreIndexError::Cancelled {
                    me.view.borrow_mut().index_error_text = Strings::get(&format!("ExtIndexError{:?}", result.error));
                }
                me.view.borrow_mut().is_loading = false;
                me.rebuild();
                me.load_previews();
            });
        });
    }

    // ------------------------------------------------------------------ liste

    fn build_query(&self) -> StoreQuery {
        let v = self.view.borrow();
        StoreQuery {
            filter: match v.filter_key.as_str() {
                "Installed" => StoreFilter::Installed,
                "Updates" => StoreFilter::Updates,
                _ => StoreFilter::All,
            },
            category: kind_from_name(&v.category_key),
            search: Some(v.search_text.clone()),
            sort: match v.sort_key.as_str() {
                "Alphabetical" => StoreSort::Alphabetical,
                "MostDownloads" => StoreSort::MostDownloads,
                _ => StoreSort::Newest,
            },
        }
    }

    fn rebuild(&self) {
        {
            let mut st = self.state.borrow_mut();
            st.query = self.build_query();
            st.language = current_language();
        }
        let all = self.state.borrow().visible();
        let updates = self.state.borrow().update_count();
        let has_index = self.state.borrow().index.is_some();
        *self.entries.borrow_mut() = all;
        self.rebuild_cards_only();
        let total = self.entries.borrow().len();
        let shown = self.cards.borrow().len();
        {
            let mut v = self.view.borrow_mut();
            v.updates_label = if updates > 0 {
                Strings::format("ExtFilterUpdatesCount", &[&updates.to_string()])
            } else {
                Strings::get("ExtFilterUpdates")
            };
            v.count_text = if !has_index && total == 0 { String::new() } else { self.build_count_text(total) };
            v.show_more_text = Strings::format("ExtShowMore", &[&(total - shown).min(PAGE_SIZE).to_string()]);
        }
        self.rebuild_builtin_backgrounds();
        self.changed.notify();
    }

    fn rebuild_cards_only(&self) {
        let settings = self.settings.borrow().clone();
        let mut extras = self.extras.borrow_mut();
        let cards: Vec<StoreCard> = self
            .entries
            .borrow()
            .iter()
            .take(self.display_count.get())
            .map(|view| {
                let active = self.is_active(&settings, view);
                let extra = extras.entry(view.id.clone()).or_default();
                let busy = view.status == StoreItemStatus::Installing;
                if !busy && !view.is_installed {
                    extra.is_uninstall_confirming = false;
                }
                let is_applied = active && !busy;
                StoreCard {
                    id: view.id.clone(),
                    kind: view.kind,
                    name: view.name.clone(),
                    meta_text: [view.author.clone(), version_of(view), view.license.clone()]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · "),
                    kind_label: kind_label(view.kind),
                    summary: if !view.summary.is_empty() || view.item.is_some() {
                        view.summary.clone()
                    } else {
                        Strings::get("ExtInstalledFromFile")
                    },
                    description: view.description.clone(),
                    details_text: details_of(view),
                    has_homepage: view.item.as_ref().and_then(|i| i.homepage.as_ref()).is_some_and(|h| !h.is_empty()),
                    is_busy: busy,
                    show_install: !busy && !view.is_installed && view.item.is_some(),
                    show_update: !busy && view.has_update && view.item.is_some(),
                    show_apply: !busy && view.is_installed && !is_applied,
                    is_applied,
                    show_uninstall: !busy && view.is_installed,
                    apply_label: Strings::get(if view.kind == ExtensionKind::Profile { "ExtAddToProfiles" } else { "ExtApply" }),
                    progress: extra.progress,
                    is_uninstall_confirming: extra.is_uninstall_confirming,
                    is_expanded: extra.is_expanded,
                    message: extra.message.clone(),
                    message_is_error: extra.message_is_error,
                    preview: extra.preview.clone(),
                }
            })
            .collect();
        // Listeden düşenlerin görseli bırakılır; kartlar (ileti, ilerleme) saklanır.
        let wanted: HashSet<String> = cards.iter().map(|c| c.id.clone()).collect();
        for (id, e) in extras.iter_mut() {
            if !wanted.contains(id) {
                e.preview = None;
                e.preview_requested = false;
            }
        }
        drop(extras);
        *self.cards.borrow_mut() = cards;
    }

    fn rebuild_builtin_backgrounds(&self) {
        let s = self.settings.borrow();
        let active_id = if s.notch.live_background_target != LiveBackgroundTarget::None
            && s.notch.live_background_source == LiveBackgroundSourceKind::BuiltInAmbient
        {
            Some(LiveBackgroundPresets::find(Some(&s.notch.live_background_preset)).id.clone())
        } else {
            None
        };
        let v = self.view.borrow();
        let show_category = v.category_key == "All" || v.category_key == "Background";
        let show_filter = v.filter_key != "Updates";
        let only_applied = v.filter_key == "Installed";
        let query = v.search_text.trim().to_lowercase();
        let mut out = Vec::new();
        for preset in LiveBackgroundPresets::all() {
            let is_applied = active_id.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(&preset.id));
            if !show_category || !show_filter || (only_applied && !is_applied) {
                continue;
            }
            let name = Strings::get(&preset.name_key);
            let kind = Strings::get(if preset.video_file.is_some() { "LiveBgKindVideo" } else { "LiveBgKindAmbient" });
            if !query.is_empty()
                && !name.to_lowercase().contains(&query)
                && !preset.id.to_lowercase().contains(&query)
                && !kind.to_lowercase().contains(&query)
            {
                continue;
            }
            out.push(BuiltinCard { id: preset.id.clone(), name, kind_label: kind, colors: preset.colors.clone(), is_applied });
        }
        *self.builtin.borrow_mut() = out;
    }

    fn build_count_text(&self, visible: usize) -> String {
        let text = Strings::format("ExtCountFormat", &[&visible.to_string()]);
        let skipped = self.skipped_items.get();
        if skipped > 0 { format!("{text} · {}", Strings::format("ExtSkippedFormat", &[&skipped.to_string()])) } else { text }
    }

    pub fn show_more(self: &Rc<Self>) {
        self.display_count.set(self.display_count.get() + PAGE_SIZE);
        self.rebuild();
        self.load_previews();
    }

    fn is_active(&self, settings: &AppSettings, view: &StoreEntryView) -> bool {
        let Some(entry) = &view.installed else { return false };
        let Some(folder) = ExtensionPaths::folder_for(Some(&view.id), Some(self.client.extensions_folder())) else { return false };
        ExtensionApplyPlanner::is_active(settings, entry, &folder.to_string_lossy())
    }

    fn reload_registry(&self) {
        *self.settings.borrow_mut() = self.profiles.current_settings();
        {
            let mut st = self.state.borrow_mut();
            st.registry = self.client.registry().load();
            st.busy_ids = self.busy.borrow().clone();
        }
        self.rebuild();
    }

    fn set_busy(&self, id: &str, busy: bool) {
        if busy {
            self.busy.borrow_mut().insert(id.to_string());
        } else {
            self.busy.borrow_mut().remove(id);
        }
        self.reload_registry();
    }

    /// Dil değişti: metinler baştan kurulur.
    pub fn refresh_language(&self) {
        self.rebuild();
    }

    // ------------------------------------------------------------------ önizleme

    fn load_previews(self: &Rc<Self>) {
        if !self.active.get() {
            return;
        }
        let token = self.preview_cts.borrow_mut().get_or_insert_with(CancellationTokenSource::new).token();
        let needed: Vec<(String, koy_core::extensions::store::StorePreview)> = {
            let entries = self.entries.borrow();
            let cards = self.cards.borrow();
            let mut extras = self.extras.borrow_mut();
            cards
                .iter()
                .filter_map(|c| {
                    let e = extras.entry(c.id.clone()).or_default();
                    if e.preview.is_some() || e.preview_requested {
                        return None;
                    }
                    let preview = entries.iter().find(|v| v.id == c.id)?.item.as_ref()?.preview.clone()?;
                    e.preview_requested = true;
                    Some((c.id.clone(), preview))
                })
                .collect()
        };
        if needed.is_empty() {
            return;
        }
        // En fazla üç paralel indirme: kuyruk tek iş parçacığı grubuyla tüketilir.
        let queue = Arc::new(std::sync::Mutex::new(needed));
        for _ in 0..MAX_PARALLEL_PREVIEWS {
            let queue = Arc::clone(&queue);
            let loader = Arc::clone(&self.previews);
            let token = token.clone();
            let running = Arc::clone(&self.previews_running);
            let ui = UiRef::new(self);
            running.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                loop {
                    let next = queue.lock().unwrap().pop();
                    let Some((id, preview)) = next else { break };
                    if token.is_cancellation_requested() {
                        break;
                    }
                    let bytes = loader.load(&preview, &token);
                    // Görünür boyutta çözülür; çözülemeyen görsel kartı bozmaz.
                    let decoded = bytes.and_then(|b| crate::infra::image_decode::decode(&b, Some(PREVIEW_DECODE_PIXELS)));
                    let cancelled = token.is_cancellation_requested();
                    dispatch::post(move || {
                        let Some(me) = ui.upgrade() else { return };
                        let mut extras = me.extras.borrow_mut();
                        let e = extras.entry(id).or_default();
                        if cancelled {
                            e.preview_requested = false;
                            return;
                        }
                        e.preview = decoded.map(|d| d.to_image());
                        drop(extras);
                        me.rebuild_cards_only();
                        me.changed.notify();
                    });
                }
                running.fetch_sub(1, Ordering::SeqCst);
            });
        }
    }

    // ------------------------------------------------------------------ kurulum onayı

    fn open_confirm(&self, title: String, button: String, note: String, facts: Vec<ConfirmFact>, pending: PendingInstall) {
        {
            let mut v = self.view.borrow_mut();
            v.confirm_title = title;
            v.confirm_button = button;
            v.confirm_file_note = note;
            v.confirm_facts = facts;
            v.is_confirming = true;
        }
        *self.pending.borrow_mut() = Some(pending);
        self.changed.notify();
    }

    fn facts_of(kind: ExtensionKind, version: &str, author: &str, license: &str, size: i64, from_file: bool) -> Vec<ConfirmFact> {
        let f = |label: &str, value: String| ConfirmFact { label: Strings::get(label), value };
        vec![
            f("ExtConfirmKind", kind_label(kind)),
            f("ExtConfirmVersion", version.to_string()),
            f("ExtConfirmAuthor", author.to_string()),
            f("ExtConfirmLicense", license.to_string()),
            f(if from_file { "ExtConfirmFileSize" } else { "ExtConfirmSize" }, SystemFormat::size(size, &strings::culture())),
            // Veri paketleri yetki istemez (yetki isteyen paket reddedilir); liste her zaman boştur ve yine de gösterilir.
            f("ExtConfirmPermissions", Strings::get("ExtConfirmNoPermissions")),
        ]
    }

    pub fn confirm_install(self: &Rc<Self>) {
        self.view.borrow_mut().is_confirming = false;
        let pending = self.pending.borrow_mut().take();
        match pending {
            Some(PendingInstall::Store { id, item, update }) => self.install_from_store(&id, item, update),
            Some(PendingInstall::File { path }) => self.install_file(path),
            None => self.changed.notify(),
        }
    }

    pub fn cancel_confirm(&self) {
        self.view.borrow_mut().is_confirming = false;
        *self.pending.borrow_mut() = None;
        self.changed.notify();
    }

    // ------------------------------------------------------------------ kart eylemleri

    fn entry_view(&self, id: &str) -> Option<StoreEntryView> {
        self.entries.borrow().iter().find(|v| v.id == id).cloned()
    }

    fn say(&self, id: &str, text: String, error: bool) {
        {
            let mut extras = self.extras.borrow_mut();
            let e = extras.entry(id.to_string()).or_default();
            e.message = text;
            e.message_is_error = error;
        }
        self.rebuild_cards_only();
        self.changed.notify();
    }

    fn report(&self, card: Option<&str>, text: String, error: bool) {
        match card {
            Some(id) => self.say(id, text, error),
            None => {
                {
                    let mut v = self.view.borrow_mut();
                    v.status_text = text;
                    v.status_is_error = error;
                }
                self.changed.notify();
            }
        }
    }

    pub fn request_install(&self, id: &str, update: bool) {
        let Some(item) = self.entry_view(id).and_then(|v| v.item) else { return };
        self.say(id, String::new(), false);
        let title = Strings::format(if update { "ExtConfirmUpdateTitle" } else { "ExtConfirmInstallTitle" }, &[&item.name]);
        let facts = Self::facts_of(item.kind, &item.version, &item.author, &item.license, item.package.size, false);
        self.open_confirm(
            title,
            Strings::get(if update { "ExtUpdate" } else { "ExtInstall" }),
            String::new(),
            facts,
            PendingInstall::Store { id: id.to_string(), item, update },
        );
    }

    fn install_from_store(self: &Rc<Self>, id: &str, item: StoreItem, update: bool) {
        self.extras.borrow_mut().entry(id.to_string()).or_default().progress = 0.0;
        self.set_busy(id, true);
        let client = Arc::clone(&self.client);
        let ui = UiRef::new(self);
        let id = id.to_string();
        std::thread::spawn(move || {
            let progress_id = id.clone();
            let progress = move |value: f64| {
                let pid = progress_id.clone();
                dispatch::post(move || {
                    if let Some(me) = ui.upgrade() {
                        me.extras.borrow_mut().entry(pid).or_default().progress = value;
                        me.rebuild_cards_only();
                        me.changed.notify();
                    }
                });
            };
            let result = client.install_from_store(&item, Some(&progress), &CancellationToken::none());
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                me.set_busy(&id, false);
                if !result.is_success() {
                    me.say(&id, Strings::get(&format!("ExtInstallError{:?}", result.error)), true);
                    return;
                }
                me.finish_install(Some(&id), &result, update);
            });
        });
    }

    // Kurulum bitti: güncellenen eklenti uygulanmışsa yeni dosyalar ayara/kullanıcı dosyasına yansısın diye Uygula yeniden çalışır.
    fn finish_install(&self, card: Option<&str>, result: &InstallResult, update: bool) {
        let Some(entry) = &result.entry else { return };
        let name = entry.name.clone();
        if result.previously_applied {
            let applied = self.apply_core(&entry.id);
            if let Some(error) = applied.0 {
                self.report(card, Strings::get(&error), true);
                return;
            }
            self.report(card, Strings::format("ExtUpdatedAndAppliedOk", &[&name]), false);
            return;
        }
        self.reload_registry();
        self.report(card, Strings::format(if update { "ExtUpdatedOk" } else { "ExtInstalledOk" }, &[&name]), false);
    }

    // ------------------------------------------------------------------ dosyadan kur

    pub fn install_from_file(&self) {
        let Some(path) =
            crate::infra::file_dialog::pick_file(None, &Strings::get("ExtFileDialogTitle"), &Strings::get("ExtFileFilter"), true)
        else {
            return;
        };
        let (error, manifest, size) = peek_package(Path::new(&path));
        if error != ExtensionInstallError::None {
            self.report(None, Strings::get(&format!("ExtInstallError{error:?}")), true);
            return;
        }
        let Some(manifest) = manifest else { return };
        self.view.borrow_mut().status_text.clear();
        let facts = Self::facts_of(
            manifest.kind,
            manifest.manifest.version(),
            manifest.manifest.author(),
            &manifest.license,
            size,
            true,
        );
        self.open_confirm(
            Strings::format("ExtConfirmInstallTitle", &[manifest.manifest.name()]),
            Strings::get("ExtInstall"),
            Strings::get("ExtConfirmFileNote"),
            facts,
            PendingInstall::File { path },
        );
    }

    fn install_file(self: &Rc<Self>, path: String) {
        {
            let mut v = self.view.borrow_mut();
            v.is_installing_file = true;
            v.status_text = Strings::get("ExtInstalling");
            v.status_is_error = false;
        }
        self.changed.notify();
        let client = Arc::clone(&self.client);
        let ui = UiRef::new(self);
        std::thread::spawn(move || {
            let result = client.install_from_file(Path::new(&path), &CancellationToken::none());
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                me.view.borrow_mut().is_installing_file = false;
                if !result.is_success() {
                    me.report(None, Strings::get(&format!("ExtInstallError{:?}", result.error)), true);
                    return;
                }
                me.finish_install(None, &result, false);
            });
        });
    }

    // ------------------------------------------------------------------ uygula

    pub fn apply_builtin_background(&self, preset_id: &str) {
        let preset = LiveBackgroundPresets::find(Some(preset_id));
        let mut updated = self.profiles.current_settings();
        // Hedef kapalıysa çentik: yoksa arka planı seçmek hiçbir şey göstermezdi.
        if updated.notch.live_background_target == LiveBackgroundTarget::None {
            updated.notch.live_background_target = LiveBackgroundTarget::Notch;
        }
        updated.notch.live_background_source = LiveBackgroundSourceKind::BuiltInAmbient;
        updated.notch.live_background_preset = preset.id.clone();
        if !self.profiles.commit_settings(&updated) {
            self.report(None, Strings::get("ExtApplySaveFailed"), true);
            return;
        }
        self.reload_registry();
        self.report(None, Strings::format("ExtAppliedOk", &[&Strings::get(&preset.name_key)]), false);
    }

    pub fn disable_builtin_background(&self, preset_id: &str) {
        let preset = LiveBackgroundPresets::find(Some(preset_id));
        let mut updated = self.profiles.current_settings();
        if updated.notch.live_background_target == LiveBackgroundTarget::None {
            return;
        }
        updated.notch.live_background_target = LiveBackgroundTarget::None;
        if !self.profiles.commit_settings(&updated) {
            self.report(None, Strings::get("ExtApplySaveFailed"), true);
            return;
        }
        self.reload_registry();
        self.report(None, Strings::format("ExtDisabledOk", &[&Strings::get(&preset.name_key)]), false);
    }

    pub fn apply(&self, id: &str) {
        self.say(id, String::new(), false);
        let (error, entry, files) = self.apply_core(id);
        if let Some(e) = error {
            self.report(Some(id), Strings::get(&e), true);
            return;
        }
        let name = entry.map(|e| e.name).unwrap_or_default();
        if let Some(files) = files.filter(|f| f.imported_profile.is_some()) {
            let key = if files.profile_already_present { "ExtProfileAlready" } else { "ExtProfileAdded" };
            self.report(Some(id), Strings::format(key, &[&name]), false);
            return;
        }
        self.report(Some(id), Strings::format("ExtAppliedOk", &[&name]), false);
    }

    pub fn disable(&self, id: &str) {
        self.say(id, String::new(), false);
        let entry = self.client.registry().load().find(Some(id)).cloned();
        let folder = ExtensionPaths::folder_for(Some(id), Some(self.client.extensions_folder()));
        let (Some(entry), Some(folder)) = (entry, folder) else {
            self.reload_registry();
            return;
        };
        let before = self.profiles.current_settings();
        let plan = ExtensionApplyPlanner::revert_on_uninstall(&before, &entry, &folder.to_string_lossy(), self.runner.read_backup());
        if !plan.is_success() {
            self.report(Some(id), Strings::get(&format!("ExtApplyError{:?}", plan.error)), true);
            return;
        }
        let snapshot = self.take_snapshot_for(&plan);
        let files = self.runner.run(&plan.file_actions);
        if !files.is_success() {
            self.restore_snapshot(&snapshot);
            self.report(Some(id), Strings::get(files.error_key.as_deref().unwrap_or("ExtApplyFileError")), true);
            return;
        }
        if !plan.settings.as_ref().is_some_and(|s| self.profiles.commit_settings(s)) {
            self.restore_snapshot(&snapshot);
            self.report(Some(id), Strings::get("ExtApplySaveFailed"), true);
            return;
        }
        if entry.kind != ExtensionKind::Profile && !self.client.registry().update(|r| Some(r.without_applied(id))) {
            self.profiles.commit_settings(&before);
            self.restore_snapshot(&snapshot);
            self.reload_registry();
            self.report(Some(id), Strings::get("ExtRegistrySaveFailed"), true);
            return;
        }
        self.reload_registry();
        self.report(Some(id), Strings::format("ExtDisabledOk", &[&entry.name]), false);
    }

    // Sıra: plan → dosya işleri → ayarı kaydet ve canlı uygula → kayıtta "uygulandı" işareti. Bir adım başarısız olursa
    // önceki adımlar geri alınır; yarım durum kalmaz.
    fn apply_core(&self, id: &str) -> (Option<String>, Option<InstalledEntry>, Option<ExtensionFileActionResult>) {
        let registry = self.client.registry().load();
        let entry = registry.find(Some(id)).cloned();
        let folder = ExtensionPaths::folder_for(Some(id), Some(self.client.extensions_folder()));
        let (Some(entry), Some(folder)) = (entry, folder) else {
            self.reload_registry();
            return (Some("ExtApplyNotInstalled".into()), None, None);
        };
        let before = self.profiles.current_settings();
        let plan = ExtensionApplyPlanner::apply(&before, &entry, &folder.to_string_lossy(), registry.has_applied(ExtensionKind::Theme));
        if !plan.is_success() {
            return (Some(format!("ExtApplyError{:?}", plan.error)), Some(entry), None);
        }
        let snapshot = self.take_snapshot_for(&plan);
        let files = self.runner.run(&plan.file_actions);
        if !files.is_success() {
            self.restore_snapshot(&snapshot);
            return (files.error_key.clone(), Some(entry), Some(files));
        }
        if !plan.settings.as_ref().is_some_and(|s| self.profiles.commit_settings(s)) {
            self.restore_snapshot(&snapshot);
            return (Some("ExtApplySaveFailed".into()), Some(entry), Some(files));
        }
        if entry.kind != ExtensionKind::Profile && !self.client.registry().update(|r| Some(r.with_applied(id))) {
            // Kayıt yazılamadı: "uygulandı" işareti olmadan ayar açık kalırsa kaldırma geri almayı atlardı.
            self.profiles.commit_settings(&before);
            self.restore_snapshot(&snapshot);
            self.reload_registry();
            return (Some("ExtRegistrySaveFailed".into()), Some(entry), Some(files));
        }
        if files.imported_profile.is_some() && !files.profile_already_present {
            self.profiles_changed.invoke(&());
        }
        self.reload_registry();
        (None, Some(entry), Some(files))
    }

    // Kullanıcı stil dosyalarına dokunan plan için geri alma kopyası (diğer planlar dosyaya yazmaz).
    fn take_snapshot_for(&self, plan: &ExtensionApplyPlan) -> Option<UserStyleSnapshot> {
        if plan.file_actions.iter().any(|a| a.kind != ExtensionFileActionKind::ImportProfile) { self.runner.take_snapshot() } else { None }
    }

    fn restore_snapshot(&self, snapshot: &Option<UserStyleSnapshot>) {
        if let Some(s) = snapshot {
            self.runner.restore_snapshot(s);
        }
    }

    // ------------------------------------------------------------------ kaldır

    pub fn request_uninstall(&self, id: &str, on: bool) {
        self.extras.borrow_mut().entry(id.to_string()).or_default().is_uninstall_confirming = on;
        self.rebuild_cards_only();
        self.changed.notify();
    }

    pub fn toggle_expanded(&self, id: &str) {
        {
            let mut extras = self.extras.borrow_mut();
            let e = extras.entry(id.to_string()).or_default();
            e.is_expanded = !e.is_expanded;
        }
        self.rebuild_cards_only();
        self.changed.notify();
    }

    pub fn uninstall(self: &Rc<Self>, id: &str) {
        self.extras.borrow_mut().entry(id.to_string()).or_default().is_uninstall_confirming = false;
        let entry = self.client.registry().load().find(Some(id)).cloned();
        let folder = ExtensionPaths::folder_for(Some(id), Some(self.client.extensions_folder()));
        let (Some(entry), Some(folder)) = (entry, folder) else {
            self.reload_registry();
            return;
        };
        self.say(id, String::new(), false);
        self.set_busy(id, true);
        // Önce ayar geri alınır, sonra klasör silinir. Klasör silinemezse geri alma geri alınır.
        let before = self.profiles.current_settings();
        let plan = ExtensionApplyPlanner::revert_on_uninstall(&before, &entry, &folder.to_string_lossy(), self.runner.read_backup());
        let fail = |me: &Self, text: String| {
            me.set_busy(id, false);
            me.say(id, text, true);
        };
        if !plan.is_success() {
            fail(self, Strings::get(&format!("ExtApplyError{:?}", plan.error)));
            return;
        }
        let snapshot = self.take_snapshot_for(&plan);
        let files = self.runner.run(&plan.file_actions);
        if !files.is_success() {
            self.restore_snapshot(&snapshot);
            fail(self, Strings::get(files.error_key.as_deref().unwrap_or("ExtApplyFileError")));
            return;
        }
        if !plan.settings.as_ref().is_some_and(|s| self.profiles.commit_settings(s)) {
            self.restore_snapshot(&snapshot);
            fail(self, Strings::get("ExtApplySaveFailed"));
            return;
        }
        if !self.client.uninstall(id) {
            // Klasör hâlâ duruyorsa eklenti kurulu kalmıştır: ayarı ve dosyaları eski haline döndür.
            if folder.exists() {
                self.profiles.commit_settings(&before);
                self.restore_snapshot(&snapshot);
            }
            fail(self, Strings::format("ExtUninstallFailed", &[&entry.name]));
            return;
        }
        self.set_busy(id, false);
        self.report(None, Strings::format("ExtUninstalledOk", &[&entry.name]), false);
    }

    /// Ayrıntı: yalnız https bağlantı kabuğa verilir.
    pub fn open_homepage(&self, id: &str) {
        let url = self.entry_view(id).and_then(|v| v.item).and_then(|i| i.homepage);
        match url {
            Some(u) if u.starts_with("https://") => crate::features::settings::settings_view_model::open_external(&u),
            _ => {}
        }
    }
}

fn version_of(view: &StoreEntryView) -> String {
    if view.is_installed && view.has_update {
        if let (Some(installed), Some(available)) = (&view.installed_version, &view.available_version) {
            return Strings::format("ExtVersionUpdateFormat", &[installed, available]);
        }
    }
    match view.installed_version.as_ref().or(view.available_version.as_ref()) {
        Some(v) => Strings::format("ExtVersionFormat", &[v]),
        None => String::new(),
    }
}

fn details_of(view: &StoreEntryView) -> String {
    let mut lines = Vec::new();
    if view.is_installed {
        if let Some(v) = &view.installed_version {
            lines.push(Strings::format("ExtInstalledVersion", &[v]));
        }
    }
    if let Some(item) = &view.item {
        lines.push(Strings::format("ExtUpdatedFormat", &[&item.updated.format("%d.%m.%Y").to_string()]));
        if item.downloads > 0 {
            lines.push(Strings::format("ExtDownloadsFormat", &[&item.downloads.to_string()]));
        }
    }
    lines.join(" · ")
}

/// Onay penceresi gerçek bilgiyi göstersin diye yalnız manifest okunur; asıl denetim kurulumda yapılır.
fn peek_package(path: &Path) -> (ExtensionInstallError, Option<PackageManifest>, i64) {
    use koy_core::extensions::store::zip_reader::ZipArchive;
    use std::io::Read;
    let Ok(meta) = std::fs::metadata(path) else { return (ExtensionInstallError::FileNotFound, None, 0) };
    let size = meta.len() as i64;
    if size > PackageEntryPolicy::MAX_DOWNLOAD_BYTES {
        return (ExtensionInstallError::DownloadTooLarge, None, size);
    }
    let Ok(file) = std::fs::File::open(path) else { return (ExtensionInstallError::IoError, None, 0) };
    let Ok(mut archive) = ZipArchive::new(std::io::BufReader::new(file)) else {
        return (ExtensionInstallError::InvalidArchive, None, 0);
    };
    let Some(index) = archive.get_entry(PackageEntryPolicy::MANIFEST_NAME) else {
        return (ExtensionInstallError::MissingManifest, None, size);
    };
    if archive.entries()[index].length > PackageEntryPolicy::MAX_MANIFEST_BYTES {
        return (ExtensionInstallError::FileTooLarge, None, size);
    }
    let mut bytes = Vec::new();
    match archive.open(index) {
        Ok(mut reader) => {
            if reader.read_to_end(&mut bytes).is_err() {
                return (ExtensionInstallError::InvalidArchive, None, 0);
            }
        }
        Err(_) => return (ExtensionInstallError::InvalidArchive, None, 0),
    }
    let parsed = PackageManifestParser::parse_bytes(&bytes);
    if parsed.is_valid() { (ExtensionInstallError::None, parsed.manifest, size) } else { (parsed.error, None, size) }
}

/// C# `Enum.TryParse<ExtensionKind>`: tanımlı ad ("Theme", "IconPack", "Background", "Profile").
fn kind_from_name(name: &str) -> Option<ExtensionKind> {
    [ExtensionKind::Theme, ExtensionKind::IconPack, ExtensionKind::Background, ExtensionKind::Profile]
        .into_iter()
        .find(|k| k.name().eq_ignore_ascii_case(name))
}

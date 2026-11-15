//! Karşılığı: `Extensions/Store/StoreCatalog.cs`.

use std::cmp::Ordering;
use std::collections::HashSet;

use chrono::{NaiveDate, Utc};

use super::{ExtensionKind, ExtensionVersion, InstalledEntry, InstalledRegistry, LocalizedText, StoreIndex, StoreItem};
use crate::search::{SearchQuery, SearchText};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StoreItemStatus {
    #[default]
    NotInstalled,
    Installed,
    UpdateAvailable,

    /// Kurulum ya da güncelleme sürüyor (kimlik `StoreListState::busy_ids` içinde).
    Installing,
}

/// Süzgeç: Tümü / Kurulu / Güncellemeler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StoreFilter {
    #[default]
    All,
    Installed,
    Updates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StoreSort {
    #[default]
    Newest,
    Alphabetical,
    MostDownloads,
}

/// `category`: `None` = bütün türler.
/// `search`: ad, özet ve yazarda; büyük/küçük harf ve Türkçe harf duyarsız; her sözcük bulunmalı.
/// (C# varsayılanları: `All`, `null`, `null`, `Newest` → `Default`.)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoreQuery {
    pub filter: StoreFilter,
    pub category: Option<ExtensionKind>,
    pub search: Option<String>,
    pub sort: StoreSort,
}

/// Listede bir satır: indeks öğesi, kurulu kayıt ya da ikisinin birleşimi. `item` yoksa eklenti dosyadan
/// kurulmuştur (ya da indeksten kalkmıştır) ve yalnız kurulu süzgecinde görünür.
#[derive(Debug, Clone, PartialEq)]
pub struct StoreEntryView {
    pub id: String,
    pub name: String,
    pub kind: ExtensionKind,
    pub author: String,
    pub summary: String,
    pub description: String,
    pub license: String,
    pub available_version: Option<String>,
    pub installed_version: Option<String>,
    pub status: StoreItemStatus,
    pub is_installed: bool,
    pub has_update: bool,
    pub applied: bool,
    pub updated: NaiveDate,
    pub downloads: i64,
    pub item: Option<StoreItem>,
    pub installed: Option<InstalledEntry>,
}

/// Mağaza sayfasının girdileri: indeks (henüz çekilmediyse `None`), kurulu kayıt, işlemi süren kimlikler ve sorgu.
/// Değişmez; [`StoreListState::visible`] her çağrıda hesaplar (en çok 500 + 64 satır).
/// `language`: özet ve açıklamanın gösterileceği iki harfli dil kodu (arayüz dili); metin yoksa "en".
#[derive(Debug, Clone, PartialEq)]
pub struct StoreListState {
    pub index: Option<StoreIndex>,
    pub registry: InstalledRegistry,
    pub busy_ids: HashSet<String>,
    pub query: StoreQuery,
    pub language: String,
}

impl StoreListState {
    /// `new StoreListState(index, registry, busyIds, query)` (dil C# varsayılanı `LocalizedText.FallbackLanguage`).
    pub fn new(index: Option<StoreIndex>, registry: InstalledRegistry, busy_ids: HashSet<String>, query: StoreQuery) -> Self {
        StoreListState { index, registry, busy_ids, query, language: LocalizedText::FALLBACK_LANGUAGE.to_string() }
    }

    /// `StoreListState.Initial`.
    pub fn initial() -> StoreListState {
        Self::new(None, InstalledRegistry::empty(), HashSet::new(), StoreQuery::default())
    }

    /// Sorguya göre süzülmüş, sıralanmış satırlar.
    pub fn visible(&self) -> Vec<StoreEntryView> {
        StoreCatalog::query(self)
    }

    /// Süzgeçsiz bütün satırlar (sayaçlar için).
    pub fn all(&self) -> Vec<StoreEntryView> {
        StoreCatalog::merge(self)
    }

    /// Güncellemesi olan kurulu eklenti sayısı ("Güncellemeler" sekmesindeki rozet).
    pub fn update_count(&self) -> i32 {
        self.all().iter().filter(|e| e.has_update).count() as i32
    }
}

/// İndeks ile kurulu kaydı birleştirir; süzgeç, kategori, arama ve sıralama uygular (saf).
pub struct StoreCatalog;

impl StoreCatalog {
    pub fn merge(state: &StoreListState) -> Vec<StoreEntryView> {
        let mut rows: Vec<StoreEntryView> = Vec::new();
        let mut indexed: HashSet<&str> = HashSet::new();

        if let Some(index) = &state.index {
            for item in &index.items {
                indexed.insert(item.id.as_str());
                let installed = state.registry.find(Some(&item.id));
                rows.push(Self::from_item(item, installed, state.busy_ids.contains(&item.id), &state.language));
            }
        }

        // Dosyadan kurulan (ya da indeksten kalkmış) kurulu eklentiler.
        for entry in state.registry.entries() {
            if !indexed.contains(entry.id.as_str()) {
                rows.push(Self::from_installed_only(entry, state.busy_ids.contains(&entry.id)));
            }
        }

        rows
    }

    pub fn query(state: &StoreListState) -> Vec<StoreEntryView> {
        let query = &state.query;
        let mut rows = Self::merge(state);

        match query.filter {
            StoreFilter::Installed => rows.retain(|r| r.is_installed),
            StoreFilter::Updates => rows.retain(|r| r.has_update),
            StoreFilter::All => {}
        }

        if let Some(category) = query.category {
            rows.retain(|r| r.kind == category);
        }

        let words = SearchQuery::new(query.search.as_deref(), false).words;
        if !words.is_empty() {
            rows.retain(|r| Self::matches_all(r, &words));
        }

        // Eşit anahtarda ada, sonra kimliğe göre: sıra her çağrıda aynı. `OrderBy` kararlıdır: `sort_by`.
        rows.sort_by(|a, b| {
            let by_name = || ordinal_cmp(&SearchText::fold_str(&a.name), &SearchText::fold_str(&b.name));
            let primary = match query.sort {
                StoreSort::Alphabetical => by_name(),
                StoreSort::MostDownloads => b.downloads.cmp(&a.downloads).then_with(by_name),
                StoreSort::Newest => b.updated.cmp(&a.updated).then_with(by_name),
            };
            primary.then_with(|| ordinal_cmp(&a.id, &b.id))
        });

        rows
    }

    fn from_item(item: &StoreItem, installed: Option<&InstalledEntry>, busy: bool, language: &str) -> StoreEntryView {
        // Aynı kimlik farklı türde ise güncelleme sayılmaz (kurulu olan başka bir şeydir).
        let has_update = installed.is_some_and(|i| i.kind == item.kind && ExtensionVersion::is_newer(Some(&item.version), Some(&i.version)));

        StoreEntryView {
            id: item.id.clone(),
            name: item.name.clone(),
            kind: item.kind,
            author: item.author.clone(),
            summary: item.summary.resolve(Some(language)),
            description: item.description.resolve(Some(language)),
            license: item.license.clone(),
            available_version: Some(item.version.clone()),
            installed_version: installed.map(|i| i.version.clone()),
            status: Self::status_of(installed.is_some(), has_update, busy),
            is_installed: installed.is_some(),
            has_update,
            applied: installed.is_some_and(|i| i.applied),
            updated: item.updated,
            downloads: item.downloads,
            item: Some(item.clone()),
            installed: installed.cloned(),
        }
    }

    fn from_installed_only(entry: &InstalledEntry, busy: bool) -> StoreEntryView {
        StoreEntryView {
            id: entry.id.clone(),
            name: entry.name.clone(),
            kind: entry.kind,
            author: entry.author.clone(),
            summary: String::new(),
            description: String::new(),
            license: entry.license.clone(),
            available_version: None,
            installed_version: Some(entry.version.clone()),
            status: Self::status_of(true, false, busy),
            is_installed: true,
            has_update: false,
            applied: entry.applied,
            updated: entry.installed_at.with_timezone(&Utc).date_naive(),
            downloads: 0,
            item: None,
            installed: Some(entry.clone()),
        }
    }

    fn status_of(installed: bool, has_update: bool, busy: bool) -> StoreItemStatus {
        if busy {
            StoreItemStatus::Installing
        } else if has_update {
            StoreItemStatus::UpdateAvailable
        } else if installed {
            StoreItemStatus::Installed
        } else {
            StoreItemStatus::NotInstalled
        }
    }

    fn matches_all(row: &StoreEntryView, words: &[String]) -> bool {
        let haystack = format!("{}\n{}\n{}", SearchText::fold_str(&row.name), SearchText::fold_str(&row.summary), SearchText::fold_str(&row.author));
        words.iter().all(|word| haystack.contains(word.as_str()))
    }
}

/// `StringComparer.Ordinal`: UTF-16 kod birimi sırası (Rust `str::cmp` kod noktası sırasıdır; BMP üstü karakterlerde ayrışır).
fn ordinal_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

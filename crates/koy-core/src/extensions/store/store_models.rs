//! Karşılığı: `Extensions/Store/StoreModels.cs`.

use chrono::NaiveDate;

use super::{ExtensionKind, LocalizedText};
use crate::time::DateTimeOffset;

/// İndeksteki paket dosyası: https adresi, SHA256 (küçük harf hex) ve bildirilen boyut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePackage {
    pub url: String,
    pub sha256: String,
    pub size: i64,
}

/// İsteğe bağlı önizleme görseli.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePreview {
    pub url: String,
    pub sha256: String,
}

/// İndeksteki bir eklenti. Alanlar `StoreIndexParser` tarafından doğrulanmıştır.
#[derive(Debug, Clone, PartialEq)]
pub struct StoreItem {
    pub id: String,
    pub name: String,
    pub kind: ExtensionKind,
    pub version: String,
    pub author: String,
    pub license: String,
    pub summary: LocalizedText,
    pub description: LocalizedText,
    pub homepage: Option<String>,
    /// C# `DateOnly`.
    pub updated: NaiveDate,
    pub downloads: i64,
    pub package: StorePackage,
    pub preview: Option<StorePreview>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoreIndex {
    pub generated: Option<DateTimeOffset>,
    pub items: Vec<StoreItem>,
}

impl StoreIndex {
    pub fn new(generated: Option<DateTimeOffset>, items: Vec<StoreItem>) -> Self {
        StoreIndex { generated, items }
    }

    /// `StoreIndex.Empty`.
    pub fn empty() -> StoreIndex {
        StoreIndex { generated: None, items: Vec::new() }
    }
}

/// İndeks alma/ayrıştırma hatası. Hata yoksa `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StoreIndexError {
    #[default]
    None,
    /// Ağ hatası ya da zaman aşımı (yalnız istemci üretir).
    Network,
    TooLarge,
    MalformedJson,
    NotAStoreIndex,
    UnsupportedFormatVersion,
    TooManyItems,
    Cancelled,
}

/// `skipped_items`: bozuk ya da tekrar eden olduğu için atlanan öğe sayısı.
#[derive(Debug, Clone, PartialEq)]
pub struct StoreIndexResult {
    pub error: StoreIndexError,
    pub index: Option<StoreIndex>,
    pub skipped_items: i32,
}

impl StoreIndexResult {
    /// `new StoreIndexResult(error, index, skippedItems = 0)`.
    pub fn new(error: StoreIndexError, index: Option<StoreIndex>, skipped_items: i32) -> Self {
        StoreIndexResult { error, index, skipped_items }
    }

    pub fn is_success(&self) -> bool {
        self.error == StoreIndexError::None && self.index.is_some()
    }
}

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::text;

/// `Guid.NewGuid().ToString("N")[..hex_len]` karşılığı: rastgele onaltılık kimlik (std'den, ek crate yok).
pub(crate) fn new_short_id(hex_len: usize) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut out = String::new();
    while out.len() < hex_len {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        if let Ok(t) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            h.write_u128(t.as_nanos());
        }
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out.truncate(hex_len);
    out
}

/// Dock üzerinde sabitlenmiş uygulamaları gruplayan klasör veri modeli (saf, bağımsız).
///
/// settings.json'da C# ile aynı özellik adları (`Id`, `Name`, `Items`) PascalCase yazılır; eksik alan varsayılan
/// yapıcıdaki değeri alır (rastgele 8 karakterlik `Id`, "Folder", boş liste).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct DockFolder {
    pub id: String,
    pub name: String,
    pub items: Vec<String>,
}

impl Default for DockFolder {
    /// Varsayılan yapıcı: boş öğe listesiyle yeni bir klasör oluşturur.
    fn default() -> Self {
        DockFolder { id: new_short_id(8), name: "Folder".to_string(), items: Vec::new() }
    }
}

impl DockFolder {
    pub fn new(id: impl Into<String>, name: impl Into<String>, items: Vec<String>) -> Self {
        DockFolder { id: id.into(), name: name.into(), items }
    }
}

/// Değer tabanlı eşitlik: Id (büyük/küçük harf duyarsız), Name (Ordinal) ve Items sırasını (büyük/küçük harf duyarsız) doğrular.
impl PartialEq for DockFolder {
    fn eq(&self, other: &Self) -> bool {
        text::eq_ignore_case(&self.id, &other.id)
            && self.name == other.name
            && self.items.len() == other.items.len()
            && self.items.iter().zip(other.items.iter()).all(|(a, b)| text::eq_ignore_case(a, b))
    }
}

impl Eq for DockFolder {}

impl std::hash::Hash for DockFolder {
    /// Eşitlikle uyumlu: Id ve öğeler büyük harfe çevrilerek karıştırılır (OrdinalIgnoreCase karması).
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        text::to_upper_invariant(&self.id).hash(state);
        self.name.hash(state);
        for item in &self.items {
            text::to_upper_invariant(item).hash(state);
        }
    }
}

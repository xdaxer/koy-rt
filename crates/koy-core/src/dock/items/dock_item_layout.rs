use std::collections::HashSet;

use super::dock_item_entry::DockItemEntry;
use super::dock_item_kind::DockItemKind;
use super::dock_item_rules::DockItemRules;
use super::dock_item_spec::DockItemSpec;
use crate::dock::dock_folder::new_short_id;
use crate::dock::pinned_order::PinnedOrder;
use crate::text;

/// Dock şeridinin düzeni: uygulama/klasör sırası (PinnedOrder) ile ayırıcı, boşluk, saat, çöp kutusu ve özel düğmelerin
/// birleşimi. Saf kurallar; çizim ve çalıştırma App'te. Hiç ek öğe yoksa düzen bugünkü dock'la aynıdır.
pub struct DockItemLayout;

impl DockItemLayout {
    /// Uygulama ve klasör dışındaki (ek) öğe mi?
    pub fn is_extra(kind: DockItemKind) -> bool {
        !matches!(kind, DockItemKind::App | DockItemKind::Folder)
    }

    /// Düzende en az bir ek öğe var mı? Yoksa dock hiç değişmez.
    pub fn has_extras(items: &[DockItemSpec]) -> bool {
        items.iter().any(|i| Self::is_extra(i.kind))
    }

    /// Uygulama ya da klasörün PinnedOrder belirteci. Ek öğeler için `None`.
    pub fn token_of(item: &DockItemSpec) -> Option<String> {
        match item.kind {
            DockItemKind::App => Some(item.id.clone()),
            DockItemKind::Folder => Some(PinnedOrder::to_folder_token(&item.id)),
            _ => None,
        }
    }

    /// Kayıtlı öğe listesini şu an dock'ta bulunan uygulama/klasör sırasıyla uzlaştırır (`effective_order`
    /// PinnedOrder uzlaştırmasının sonucudur). Listede olup dock'ta artık bulunmayan uygulama ya da klasör düşer; ek
    /// öğeler yerinde kalır. Listede olmayan yeni uygulama/klasör, son uygulama/klasörün hemen arkasına (hiç yoksa
    /// başa) kendi sırasıyla eklenir.
    pub fn merge(items: &[DockItemSpec], effective_order: &[String]) -> Vec<DockItemSpec> {
        let present: HashSet<String> = effective_order.iter().map(|s| text::to_upper_invariant(s)).collect();
        let mut result: Vec<DockItemSpec> = Vec::new();
        let mut placed: HashSet<String> = HashSet::new();

        for item in items {
            let Some(token) = Self::token_of(item) else {
                result.push(item.clone());
                continue;
            };

            let upper = text::to_upper_invariant(&token);
            if present.contains(&upper) && placed.insert(upper) {
                result.push(item.clone());
            }
        }

        let mut missing: Vec<DockItemSpec> = Vec::new();
        for token in effective_order {
            if text::is_null_or_white_space(Some(token)) || placed.contains(&text::to_upper_invariant(token)) {
                continue;
            }

            placed.insert(text::to_upper_invariant(token));
            missing.push(if PinnedOrder::is_folder_id(token) {
                DockItemSpec::new(DockItemKind::Folder, PinnedOrder::extract_folder_id(token), "")
            } else {
                DockItemSpec::new(DockItemKind::App, token.clone(), "")
            });
        }

        if !missing.is_empty() {
            // FindLastIndex(...) + 1: yoksa -1 + 1 = 0 (başa).
            let insert_at = result.iter().rposition(|i| !Self::is_extra(i.kind)).map_or(0, |i| i + 1);
            for (offset, m) in missing.into_iter().enumerate() {
                result.insert(insert_at + offset, m);
            }
        }

        result
    }

    /// Öğeyi sona ekler. Sınır aşılırsa ya da tekil bir öğe (saat, çöp kutusu) zaten varsa liste değişmeden döner
    /// (doğrulama `DockItemRules::normalize` ile aynıdır).
    pub fn add(items: &[DockItemSpec], item: DockItemSpec) -> Vec<DockItemSpec> {
        let mut appended = items.to_vec();
        appended.push(item);
        let next = DockItemRules::normalize(&Self::to_entries(&appended), DockItemRules::MAX_ITEM_COUNT);
        if next.len() == items.len() + 1 { next } else { items.to_vec() }
    }

    /// İndeksteki öğeyi çıkarır. Geçersiz indekste liste değişmeden kopyalanır.
    pub fn remove_at(items: &[DockItemSpec], index: i32) -> Vec<DockItemSpec> {
        let mut copy = items.to_vec();
        if index >= 0 && (index as usize) < copy.len() {
            copy.remove(index as usize);
        }
        copy
    }

    /// Doğrulanmış öğeleri ayar kaydına çevirir. Hiç ek öğe kalmadıysa boş döner: dock PinnedOrder'dan türetilir.
    pub fn to_stored_entries(items: &[DockItemSpec]) -> Vec<DockItemEntry> {
        if Self::has_extras(items) { Self::to_entries(items) } else { Vec::new() }
    }

    /// Özel düğme için yeni kimlik (kısa, dosya adı olmayan, tahmin edilemez).
    pub fn new_custom_id() -> String {
        new_short_id(8)
    }

    fn to_entries(items: &[DockItemSpec]) -> Vec<DockItemEntry> {
        items.iter().map(|i| DockItemEntry::new(i.kind.to_string(), i.id.clone(), i.command.clone())).collect()
    }
}

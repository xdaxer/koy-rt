//! Karşılığı: `JumpList/JumpListLayout.cs`.

use std::cmp::Reverse;

use super::custom_destinations::{CustomDestinationsFile, JumpListKnownCategory};
use super::jump_list_entry_factory::JumpListText;
use super::jump_list_limits::JumpListLimits;
use super::jump_list_models::{JumpListEntry, JumpListSection, JumpListSectionKind};

/// Atlama listesi bölümlerinin sırası ve sınırları (saf mantık). Menü sırası: uygulamanın kendi kategorileri, sık
/// kullanılanlar, son kullanılanlar, görevler. (Koy'un kendi öğeleri bunların altına App'te eklenir.)
pub struct JumpListLayout;

/// `Arrange` içindeki çalışma kaydı (C#'taki özel `Working` sınıfı).
struct Working {
    kind: JumpListSectionKind,
    name: Option<String>,
    entries: Vec<JumpListEntry>,
}

impl JumpListLayout {
    /// Son kullanılanlar bölümü gösterilsin mi. Uygulamanın kendi listesi yoksa (dosya yok ya da okunamadı) Windows
    /// varsayılanı (son kullanılanlar) geçerlidir; kendi listesi varsa yalnız `AppendKnownCategory(RECENT)` demişse.
    pub fn shows_recent(file: Option<&CustomDestinationsFile>) -> bool {
        file.is_none_or(|f| !f.is_valid || f.declares(JumpListKnownCategory::Recent))
    }

    /// Sık kullanılanlar yalnız uygulamanın listesi isterse gösterilir (Explorer gibi).
    pub fn shows_frequent(file: Option<&CustomDestinationsFile>) -> bool {
        file.is_some_and(|f| f.declares(JumpListKnownCategory::Frequent))
    }

    /// Bir bölüm türünün girdi sınırı (ayırıcılar sayılmaz).
    pub fn cap_for(kind: JumpListSectionKind) -> i32 {
        match kind {
            JumpListSectionKind::Custom => JumpListLimits::MAX_PER_CUSTOM_SECTION,
            JumpListSectionKind::Frequent => JumpListLimits::MAX_FREQUENT,
            JumpListSectionKind::Recent => JumpListLimits::MAX_RECENT,
            _ => JumpListLimits::MAX_TASKS,
        }
    }

    /// Ham bölümleri menü sırasına koyar ve sınırlar. Boş ve adsız özel kategoriler atılır; baştaki, sondaki ve art arda
    /// ayırıcılar temizlenir; bölüm başına ve toplam girdi sınırı uygulanır. Aynı türden (özel dışı) birden çok bölüm gelirse
    /// ilki alınır.
    pub fn arrange<'a>(sections: impl IntoIterator<Item = &'a JumpListSection>) -> Vec<JumpListSection> {
        let mut custom: Vec<Working> = Vec::new();
        let mut frequent: Option<Working> = None;
        let mut recent: Option<Working> = None;
        let mut tasks: Option<Working> = None;

        for section in sections {
            let name = if section.kind == JumpListSectionKind::Custom {
                Some(JumpListText::sanitize(section.name.as_deref(), JumpListLimits::MAX_CATEGORY_NAME_CHARS))
            } else {
                None
            };
            if section.kind == JumpListSectionKind::Custom && name.as_deref().is_none_or(str::is_empty) {
                continue; // adsız kategori etiketlenemez
            }

            let entries = Self::cap(&Self::normalize(&section.entries), Self::cap_for(section.kind));
            if Self::count_entries(&entries) == 0 {
                continue;
            }

            let working = Working { kind: section.kind, name, entries };
            match section.kind {
                JumpListSectionKind::Custom => {
                    if (custom.len() as i32) < JumpListLimits::MAX_CUSTOM_SECTIONS {
                        custom.push(working);
                    }
                }
                JumpListSectionKind::Frequent => {
                    frequent.get_or_insert(working);
                }
                JumpListSectionKind::Recent => {
                    recent.get_or_insert(working);
                }
                _ => {
                    tasks.get_or_insert(working);
                }
            }
        }

        let mut ordered: Vec<Working> = custom;
        ordered.extend(frequent);
        ordered.extend(recent);
        ordered.extend(tasks);

        Self::trim_to_total(&mut ordered);

        ordered
            .into_iter()
            .map(|w| JumpListSection { kind: w.kind, name: w.name, entries: Self::normalize(&w.entries) })
            .filter(|s| Self::count_entries(&s.entries) > 0)
            .collect()
    }

    /// Ayırıcı olmayan girdi sayısı.
    pub fn count_entries(entries: &[JumpListEntry]) -> i32 {
        entries.iter().filter(|e| !e.is_separator()).count() as i32
    }

    /// Toplam sınırı aşılırsa önce sık, sonra son kullanılanlar, sonra özel kategoriler (sondan), en son görevler kırpılır.
    fn trim_to_total(ordered: &mut [Working]) {
        let rank = |kind: JumpListSectionKind| match kind {
            JumpListSectionKind::Frequent => 0,
            JumpListSectionKind::Recent => 1,
            JumpListSectionKind::Custom => 2,
            _ => 3,
        };
        // OrderBy(rütbe).ThenByDescending(sıra): kararlı sıralama.
        let mut trim_order: Vec<usize> = (0..ordered.len()).collect();
        trim_order.sort_by_key(|&i| (rank(ordered[i].kind), Reverse(i)));

        loop {
            let total: i32 = ordered.iter().map(|w| Self::count_entries(&w.entries)).sum();
            if total <= JumpListLimits::MAX_TOTAL_ENTRIES {
                break;
            }

            let victim = trim_order
                .iter()
                .copied()
                .find(|&i| Self::count_entries(&ordered[i].entries) > JumpListLimits::MIN_PER_SECTION_WHEN_TRIMMING);
            let Some(victim) = victim else { break };

            let count = Self::count_entries(&ordered[victim].entries);
            ordered[victim].entries = Self::cap(&ordered[victim].entries, count - 1);
        }
    }

    /// İlk `max` girdiyi (ayırıcılar dahil sırayla) tutar ve ayırıcıları temizler.
    fn cap(entries: &[JumpListEntry], max: i32) -> Vec<JumpListEntry> {
        let mut result: Vec<JumpListEntry> = Vec::new();
        let mut count = 0;
        for entry in entries {
            if !entry.is_separator() {
                if count >= max {
                    break;
                }
                count += 1;
            }

            result.push(entry.clone());
        }

        Self::normalize(&result)
    }

    /// Baştaki, sondaki ve art arda ayırıcıları atar.
    fn normalize(entries: &[JumpListEntry]) -> Vec<JumpListEntry> {
        let mut result: Vec<JumpListEntry> = Vec::with_capacity(entries.len());
        for entry in entries {
            if entry.is_separator() && result.last().is_none_or(|last| last.is_separator()) {
                continue;
            }
            result.push(entry.clone());
        }

        while result.last().is_some_and(|last| last.is_separator()) {
            result.pop();
        }

        result
    }
}

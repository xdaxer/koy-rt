use super::SearchResult;
use crate::text;
use std::cmp::Ordering;

/// Sonuçları listeye dizer: aynı kategori bir arada, önce kategorinin yeri (uygulamalar üstte, dosyalar altta),
/// sonra önemi. Aşağıdaki kategori daha az yer alır:
/// ilk kategori sınırın en çok yarısını, ikincisi üçte birini kullanır; her kategoriye en az bir satır kalır.
pub struct SearchLayout;

impl SearchLayout {
    /// Listedeki en fazla satır; çentikte görünmeyenlere kaydırılarak ulaşılır.
    pub const MAX_RESULTS: i32 = 15;

    /// C# varsayılanı: `limit = MaxResults`.
    pub fn arrange(results: Vec<SearchResult>, limit: i32) -> Vec<SearchResult> {
        // GroupBy ilk görülme sırasını korur.
        let mut groups: Vec<(String, Vec<SearchResult>)> = Vec::new();
        for r in results {
            match groups.iter_mut().find(|(c, _)| *c == r.category) {
                Some((_, g)) => g.push(r),
                None => groups.push((r.category.clone(), vec![r])),
            }
        }

        let mut groups: Vec<Vec<SearchResult>> = groups
            .into_iter()
            .map(|(_, mut g)| {
                // OrderByDescending(Relevance).ThenBy(Title, OrdinalIgnoreCase): kararlı.
                g.sort_by(|a, b| {
                    b.relevance
                        .partial_cmp(&a.relevance)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| text::cmp_ignore_case(&a.title, &b.title))
                });
                g
            })
            .collect();

        // OrderBy(Min(Placement)).ThenByDescending(Max(CategoryRelevance)): kararlı.
        groups.sort_by(|a, b| {
            let pa = a.iter().map(|r| r.placement).min().unwrap();
            let pb = b.iter().map(|r| r.placement).min().unwrap();
            let ra = a.iter().map(|r| r.category_relevance).max().unwrap();
            let rb = b.iter().map(|r| r.category_relevance).max().unwrap();
            pa.cmp(&pb).then(rb.cmp(&ra))
        });

        let limit_usize = limit.max(0) as usize;
        if groups.len() == 1 {
            return groups.remove(0).into_iter().take(limit_usize).collect();
        }

        let mut arranged: Vec<SearchResult> = Vec::new();
        let count = groups.len() as i32;
        let reserve = (limit as f64 / i32::max(1, count) as f64).ceil() as i32;
        for (i, group) in groups.into_iter().enumerate() {
            let share = (limit as f64 / (i as i32 + 2) as f64).ceil() as i32;
            let cap = i32::max(1, i32::min(limit - arranged.len() as i32 - reserve, share));
            arranged.extend(group.into_iter().take(cap as usize));
        }

        arranged
    }
}

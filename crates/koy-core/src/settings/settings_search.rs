//! Karşılığı: `koy/src/Core/Settings/SettingsSearch.cs`.

use crate::text;

/// Ayar arama satırı: anahtar, sayfa ve App'in yerelleştirdiği metinler. Core metni çevirmez, yalnız eşleştirir
/// (Core resx'e erişemez, bkz. docs/MAP.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingSearchEntry {
    pub key: String,
    pub page: String,
    pub title: String,
    pub description: String,
    pub keywords: Vec<String>,
}

/// Arama sonucu: eşleşen satır ve puanı (yüksek olan önce).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingSearchHit {
    pub entry: SettingSearchEntry,
    pub score: i32,
}

/// Ayarlar penceresindeki arama. Sorgu boşluklara göre parçalanır; her parça satırın en az bir yerinde
/// (başlık, anahtar sözcük ya da açıklama) geçmelidir. Büyük/küçük harf ve Türkçe harfler (ı/i, ş/s, ğ/g, ç/c, ö/o, ü/u)
/// farksızdır: "karanlik" sorgusu "Karanlık tema" satırını bulur.
/// Puan, parça başına en iyi eşleşmeye göre: başlık 3, anahtar sözcük 2, açıklama 1. Başlık sorgunun tamamıyla
/// (parçalar tek boşlukla birleşik) başlıyorsa +2. Eşit puanda verilen sıra korunur.
pub struct SettingsSearch;

const TITLE_WEIGHT: i32 = 3;
const KEYWORD_WEIGHT: i32 = 2;
const DESCRIPTION_WEIGHT: i32 = 1;
const PREFIX_BONUS: i32 = 2;

impl SettingsSearch {
    /// `limit`: C# varsayılanı `int.MaxValue`. Negatif sınır hatadır (panik, `ArgumentOutOfRangeException`).
    pub fn search(entries: &[SettingSearchEntry], query: Option<&str>, limit: i32) -> Vec<SettingSearchHit> {
        if limit < 0 {
            panic!("limit negatif olamaz (ArgumentOutOfRangeException)");
        }

        let tokens = tokenize(query);
        if tokens.is_empty() || limit == 0 {
            return Vec::new();
        }

        let full_query = tokens.join(" ");
        let mut hits = Vec::new();
        for entry in entries {
            let score = score_entry(entry, &tokens, &full_query);
            if score > 0 {
                hits.push(SettingSearchHit { entry: entry.clone(), score });
            }
        }

        // OrderByDescending kararlıdır: eşit puanlı satırlar verildiği sırayla kalır.
        hits.sort_by(|a, b| b.score.cmp(&a.score));
        hits.truncate(limit as usize);
        hits
    }

    /// Küçük harfe çevirir ve Türkçe harfleri ASCII karşılığına indirir. Arama için yalnız eşleştirmede kullanılır.
    pub fn fold(text_in: Option<&str>) -> String {
        let Some(text_in) = text_in else { return String::new() };
        if text_in.is_empty() {
            return String::new();
        }

        let lower = text::to_lower_invariant(text_in);
        let mut sb = String::with_capacity(lower.len());
        for c in lower.chars() {
            // İ küçük harfe çevrilince oluşan birleşik nokta: yok say, yoksa "i" parçalanır.
            if c == '\u{307}' {
                continue;
            }

            sb.push(match c {
                'ı' | 'İ' | 'î' => 'i',
                'ş' => 's',
                'ç' => 'c',
                'ğ' => 'g',
                'ö' => 'o',
                'ü' => 'u',
                'â' => 'a',
                'û' => 'u',
                _ => c,
            });
        }

        sb
    }
}

fn tokenize(query: Option<&str>) -> Vec<String> {
    if text::is_null_or_white_space(query) {
        return Vec::new();
    }

    let folded = SettingsSearch::fold(query);
    let mut tokens: Vec<String> = Vec::new();
    for token in folded.split_whitespace() {
        if !tokens.iter().any(|t| t == token) {
            tokens.push(token.to_string());
        }
    }
    tokens
}

fn score_entry(entry: &SettingSearchEntry, tokens: &[String], full_query: &str) -> i32 {
    let title = SettingsSearch::fold(Some(&entry.title));
    let description = SettingsSearch::fold(Some(&entry.description));
    let keywords: Vec<String> = entry.keywords.iter().map(|k| SettingsSearch::fold(Some(k))).collect();

    let mut score = 0;
    for token in tokens {
        let mut best = 0;
        if title.contains(token.as_str()) {
            best = best.max(TITLE_WEIGHT);
        }

        if keywords.iter().any(|k| k.contains(token.as_str())) {
            best = best.max(KEYWORD_WEIGHT);
        }

        if description.contains(token.as_str()) {
            best = best.max(DESCRIPTION_WEIGHT);
        }

        // Bir parça hiçbir yerde yoksa satır elenir (tüm parçalar gerekir).
        if best == 0 {
            return 0;
        }

        score += best;
    }

    if title.starts_with(full_query) {
        score += PREFIX_BONUS;
    }

    score
}

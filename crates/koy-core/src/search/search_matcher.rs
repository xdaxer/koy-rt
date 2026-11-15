use super::SearchQuery;

/// Sorgunun bir metne ne kadar uyduğunu 0 (uymuyor) ile 1 arasında puanlar. Yazılan her sözcük eşleşmelidir.
/// Metin [`super::SearchText::fold`] ile sadeleştirilmiş gelir. Uzunluklar karakter sayısıyla tutulur (C#'ta
/// UTF-16 birimi; BMP dışı karakterlerde küçük fark olabilir).
pub struct SearchMatcher;

/// Yazım hatası toleransı kısa sözcüklerde her şeyi eşleştirmesin diye en az bu uzunlukta uygulanır.
const MIN_TYPO_LENGTH: usize = 4;

const WORD_SEPARATORS: [char; 13] = [' ', '-', '_', '.', ',', '(', ')', '[', ']', '\\', '/', ':', '+'];

impl SearchMatcher {
    pub fn score(query: &SearchQuery, folded: &str) -> f64 {
        if query.is_empty() || folded.is_empty() {
            return 0.0;
        }

        if folded == query.folded {
            return 1.0;
        }

        let words: Vec<&str> = folded.split(WORD_SEPARATORS).filter(|w| !w.is_empty()).collect();
        let initials: String = words.iter().filter_map(|w| w.chars().next()).collect();

        let mut total = 0.0;
        for term in &query.words {
            let score = Self::score_term(term, folded, &words, &initials);
            if score == 0.0 {
                return 0.0;
            }

            total += score;
        }

        // Aynı tür eşleşmede daha kısa ad öne geçer: sorgu metnin büyük kısmını kaplıyordur.
        let coverage = f64::min(1.0, query.folded.chars().count() as f64 / folded.chars().count() as f64);
        total / query.words.len() as f64 * 0.9 + coverage * 0.09
    }

    fn score_term(term: &str, folded: &str, words: &[&str], initials: &str) -> f64 {
        if folded.starts_with(term) {
            return 0.95;
        }

        if words.contains(&term) {
            return 0.9;
        }

        if words.iter().any(|w| w.starts_with(term)) {
            return 0.8;
        }

        let term_len = term.chars().count();
        if term_len >= 2 && initials.starts_with(term) {
            return 0.7;
        }

        if folded.contains(term) {
            return 0.5;
        }

        if term_len >= MIN_TYPO_LENGTH && words.iter().any(|w| Self::is_one_edit_away(term, w)) {
            return 0.3;
        }

        0.0
    }

    /// Yazım hatası: bir harf farklı, bir harf eksik ya da fazla, ya da yan yana iki harf yer değiştirmiş.
    fn is_one_edit_away(term: &str, word: &str) -> bool {
        let term: Vec<char> = term.chars().collect();
        let mut word: Vec<char> = word.chars().collect();
        // Uzun sözcüğün başını da yoklar: "chorm" da "chrome" bulsun.
        if word.len() > term.len() + 1 {
            word.truncate(term.len() + 1);
        }

        match term.len().abs_diff(word.len()) {
            0 => Self::is_one_substitution_or_swap(&term, &word),
            1 => {
                if term.len() < word.len() {
                    Self::is_one_insertion(&term, &word)
                } else {
                    Self::is_one_insertion(&word, &term)
                }
            }
            _ => false,
        }
    }

    fn is_one_substitution_or_swap(a: &[char], b: &[char]) -> bool {
        let mut first: Option<usize> = None;
        let mut differences = 0;
        for i in 0..a.len() {
            if a[i] == b[i] {
                continue;
            }

            if first.is_none() {
                first = Some(i);
            }
            differences += 1;
        }

        if differences <= 1 {
            return true;
        }

        let first = first.unwrap();
        differences == 2 && first + 1 < a.len() && a[first] == b[first + 1] && a[first + 1] == b[first]
    }

    fn is_one_insertion(shorter: &[char], longer: &[char]) -> bool {
        let mut i = 0;
        while i < shorter.len() && shorter[i] == longer[i] {
            i += 1;
        }

        shorter[i..] == longer[i + 1..]
    }
}

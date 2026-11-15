use super::{
    ISearchShell, ISearchSource, SearchDragData, SearchDragKind, SearchGlyph, SearchMatcher, SearchQuery, SearchRelevance,
    SearchResult, SearchSourceBase, SearchText, Translate,
};
use super::search_util::two_letter_language_name;
use crate::clock::Culture;
use crate::text;
use std::cmp::Ordering;
use std::sync::Arc;

/// Özel karakter kataloğundaki bir satır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialCharEntry {
    pub symbol: &'static str,
    pub english_name: &'static str,
    pub turkish_name: &'static str,
    pub keywords: &'static str,
}

/// Özel karakter ve sembol kataloğu; Enter panoya kopyalar, panel dışına sürüklenebilir.
pub struct SpecialCharactersSource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    turkish: bool,
}

const MAX_DIRECT_RESULTS: usize = 12;
const MAX_KEYWORD_RESULTS: usize = 24;

impl SpecialCharactersSource {
    /// C# `culture = null` → `CurrentUICulture`: burada zorunlu parametre.
    pub fn new(shell: Arc<dyn ISearchShell>, t: Translate, culture: Culture) -> Self {
        let name = t("SearchCatSpecialChars");
        SpecialCharactersSource {
            base: SearchSourceBase::new("chars", name),
            shell,
            t,
            turkish: text::eq_ignore_case(&two_letter_language_name(&culture), "tr"),
        }
    }

    /// `Catalog`.
    pub fn catalog() -> &'static [SpecialCharEntry] {
        CATALOG
    }

    fn code_point(entry: &SpecialCharEntry) -> u32 {
        entry.symbol.chars().next().map(|c| c as u32).unwrap_or(0)
    }

    fn score_entry(entry: &SpecialCharEntry, query: &SearchQuery) -> f64 {
        if query.raw.trim() == entry.symbol {
            return 1.0;
        }

        let code_point = format!("u+{:04x}", Self::code_point(entry));
        if query.folded == code_point {
            return 0.95;
        }

        let en_score = SearchMatcher::score(query, &SearchText::fold_str(entry.english_name));
        let tr_score = SearchMatcher::score(query, &SearchText::fold_str(entry.turkish_name));
        let kw_score = SearchMatcher::score(query, entry.keywords) * 0.75;
        f64::max(en_score, f64::max(tr_score, kw_score))
    }

    fn to_result(&self, entry: &SpecialCharEntry, score: f64, category_relevance: i32) -> SearchResult {
        let display_name = if self.turkish { entry.turkish_name } else { entry.english_name };
        let code_point = format!("U+{:04X}", Self::code_point(entry));
        let shell = Arc::clone(&self.shell);
        let symbol = entry.symbol;
        let mut result = SearchResult::new(
            format!("char:{code_point}"),
            self.base.name(),
            format!("{}  {}", entry.symbol, display_name),
            move || shell.copy_text(symbol),
        );
        result.subtitle = Some(format!("{} · {}", code_point, (self.t)("SearchHintCopy")));
        result.glyph = SearchGlyph::Symbol;
        result.category_relevance = category_relevance;
        result.relevance = score;
        result.restores_focus = true;
        result.drag = Some(SearchDragData::new(SearchDragKind::Text, entry.symbol));
        result
    }

    /// Anahtar sözcük tek başına ya da önek olarak varsa `Some(filtre)`.
    fn try_extract_keyword_filter(&self, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        let keywords = (self.t)("SearchKwSpecialChars");
        for kw in keywords.split(';').filter(|k| !k.is_empty()) {
            if text::eq_ignore_case(trimmed, kw) {
                return Some(String::new());
            }

            if text::starts_with_ignore_case(trimmed, &format!("{kw} ")) || text::starts_with_ignore_case(trimmed, &format!("{kw}:")) {
                let rest: String = trimmed.chars().skip(kw.chars().count() + 1).collect();
                return Some(rest.trim().to_string());
            }
        }

        None
    }

    fn is_single_catalog_symbol(raw: &str) -> bool {
        let s = raw.trim();
        s.encode_utf16().count() == 1 && CATALOG.iter().any(|c| c.symbol == s)
    }
}

impl ISearchSource for SpecialCharactersSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        if query.is_empty() {
            return Vec::new();
        }

        let keyword_filter = self.try_extract_keyword_filter(&query.raw);
        let has_keyword = keyword_filter.is_some();
        let filter = keyword_filter.unwrap_or_default();
        if !has_keyword && query.folded.chars().count() < 2 && !Self::is_single_catalog_symbol(&query.raw) {
            return Vec::new();
        }

        if has_keyword && filter.is_empty() {
            return CATALOG
                .iter()
                .take(MAX_KEYWORD_RESULTS)
                .enumerate()
                .map(|(i, entry)| self.to_result(entry, 1.0 - (i as f64 * 0.01), SearchRelevance::HIGH))
                .collect();
        }

        let owned_query;
        let search_query = if has_keyword {
            owned_query = SearchQuery::from_text(&filter);
            &owned_query
        } else {
            query
        };
        let limit = if has_keyword { MAX_KEYWORD_RESULTS } else { MAX_DIRECT_RESULTS };

        let mut results: Vec<(SearchResult, f64)> = Vec::new();
        for entry in CATALOG {
            let score = Self::score_entry(entry, search_query);
            if score <= 0.0 {
                continue;
            }

            let category_relevance = if has_keyword {
                SearchRelevance::HIGHEST
            } else if score >= 0.9 {
                SearchRelevance::MODERATE
            } else {
                SearchRelevance::LOW
            };
            results.push((self.to_result(entry, score, category_relevance), score));
        }

        // OrderByDescending kararlıdır.
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        results.into_iter().take(limit).map(|r| r.0).collect()
    }
}

/// `Catalog` (C#'taki sırayla).
static CATALOG: &[SpecialCharEntry] = &[
    SpecialCharEntry { symbol: "→", english_name: "Right Arrow", turkish_name: "Sağ Ok", keywords: "arrow right ok sag next ileri" },
    SpecialCharEntry { symbol: "←", english_name: "Left Arrow", turkish_name: "Sol Ok", keywords: "arrow left ok sol back geri" },
    SpecialCharEntry { symbol: "↑", english_name: "Up Arrow", turkish_name: "Yukarı Ok", keywords: "arrow up ok yukari" },
    SpecialCharEntry { symbol: "↓", english_name: "Down Arrow", turkish_name: "Aşağı Ok", keywords: "arrow down ok asagi" },
    SpecialCharEntry { symbol: "↔", english_name: "Left Right Arrow", turkish_name: "Çift Yönlü Ok", keywords: "arrow both left right ok cift yonlu" },
    SpecialCharEntry { symbol: "↕", english_name: "Up Down Arrow", turkish_name: "Dikey Ok", keywords: "arrow up down ok dikey" },
    SpecialCharEntry { symbol: "⇒", english_name: "Right Double Arrow", turkish_name: "Sağ Çift Ok", keywords: "arrow double right implies ok cift sag" },
    SpecialCharEntry { symbol: "⇐", english_name: "Left Double Arrow", turkish_name: "Sol Çift Ok", keywords: "arrow double left ok cift sol" },
    SpecialCharEntry { symbol: "⇔", english_name: "Left Right Double Arrow", turkish_name: "Çift Ok Denk", keywords: "arrow double iff ok cift denk" },
    SpecialCharEntry { symbol: "↗", english_name: "North East Arrow", turkish_name: "Sağ Yukarı Ok", keywords: "arrow north east external ok sag yukari" },
    SpecialCharEntry { symbol: "↘", english_name: "South East Arrow", turkish_name: "Sağ Aşağı Ok", keywords: "arrow south east ok sag asagi" },
    SpecialCharEntry { symbol: "↩", english_name: "Return Arrow", turkish_name: "Geri Dönüş Oku", keywords: "arrow return reply enter ok donus" },
    SpecialCharEntry { symbol: "↪", english_name: "Forward Arrow", turkish_name: "İleri Yön Oku", keywords: "arrow forward ok ileri" },
    SpecialCharEntry { symbol: "₺", english_name: "Turkish Lira", turkish_name: "Türk Lirası", keywords: "currency money lira tl try para birimi turk lirasi" },
    SpecialCharEntry { symbol: "€", english_name: "Euro", turkish_name: "Avro", keywords: "currency money euro eur avro para birimi" },
    SpecialCharEntry { symbol: "$", english_name: "Dollar", turkish_name: "Dolar", keywords: "currency money dollar usd dolar para birimi" },
    SpecialCharEntry { symbol: "£", english_name: "Pound Sterling", turkish_name: "Sterlin", keywords: "currency money pound gbp sterlin para birimi" },
    SpecialCharEntry { symbol: "¥", english_name: "Yen / Yuan", turkish_name: "Yen / Yuan", keywords: "currency money yen yuan jpy cny para birimi" },
    SpecialCharEntry { symbol: "₹", english_name: "Indian Rupee", turkish_name: "Hint Rupisi", keywords: "currency money rupee inr rupi para birimi" },
    SpecialCharEntry { symbol: "₩", english_name: "Korean Won", turkish_name: "Kore Wonu", keywords: "currency money won krw para birimi" },
    SpecialCharEntry { symbol: "₽", english_name: "Russian Ruble", turkish_name: "Rus Rublesi", keywords: "currency money ruble rub ruble para birimi" },
    SpecialCharEntry { symbol: "₿", english_name: "Bitcoin", turkish_name: "Bitcoin", keywords: "currency crypto bitcoin btc kripto" },
    SpecialCharEntry { symbol: "¢", english_name: "Cent", turkish_name: "Sent", keywords: "currency cent sent para birimi" },
    SpecialCharEntry { symbol: "±", english_name: "Plus-Minus", turkish_name: "Artı Eksi", keywords: "math plus minus arti eksi" },
    SpecialCharEntry { symbol: "×", english_name: "Multiplication", turkish_name: "Çarpma", keywords: "math multiply times carpma x" },
    SpecialCharEntry { symbol: "÷", english_name: "Division", turkish_name: "Bölme", keywords: "math divide division bolme" },
    SpecialCharEntry { symbol: "≠", english_name: "Not Equal", turkish_name: "Eşit Değil", keywords: "math not equal esit degil" },
    SpecialCharEntry { symbol: "≈", english_name: "Approximately Equal", turkish_name: "Yaklaşık Eşit", keywords: "math approx approximately yaklasik esit" },
    SpecialCharEntry { symbol: "≤", english_name: "Less Than or Equal", turkish_name: "Küçük Eşit", keywords: "math less equal leq kucuk esit" },
    SpecialCharEntry { symbol: "≥", english_name: "Greater Than or Equal", turkish_name: "Büyük Eşit", keywords: "math greater equal geq buyuk esit" },
    SpecialCharEntry { symbol: "∞", english_name: "Infinity", turkish_name: "Sonsuzluk", keywords: "math infinity sonsuz sonsuzluk" },
    SpecialCharEntry { symbol: "√", english_name: "Square Root", turkish_name: "Karekök", keywords: "math sqrt square root karekok" },
    SpecialCharEntry { symbol: "∑", english_name: "Summation", turkish_name: "Toplam", keywords: "math sum summation sigma toplam" },
    SpecialCharEntry { symbol: "∏", english_name: "Product", turkish_name: "Çarpım", keywords: "math product pi carpim" },
    SpecialCharEntry { symbol: "∫", english_name: "Integral", turkish_name: "İntegral", keywords: "math integral" },
    SpecialCharEntry { symbol: "∂", english_name: "Partial Differential", turkish_name: "Kısmi Türev", keywords: "math partial derivative turev" },
    SpecialCharEntry { symbol: "∆", english_name: "Increment / Delta", turkish_name: "Fark / Delta", keywords: "math delta increment fark" },
    SpecialCharEntry { symbol: "°", english_name: "Degree", turkish_name: "Derece", keywords: "math degree angle temperature derece" },
    SpecialCharEntry { symbol: "‰", english_name: "Per Mille", turkish_name: "Binde", keywords: "math permille binde" },
    SpecialCharEntry { symbol: "∴", english_name: "Therefore", turkish_name: "Dolayısıyla", keywords: "math therefore logic dolayisiyla" },
    SpecialCharEntry { symbol: "∵", english_name: "Because", turkish_name: "Çünkü", keywords: "math because logic cunku" },
    SpecialCharEntry { symbol: "∈", english_name: "Element Of", turkish_name: "Elemanıdır", keywords: "math set element in eleman" },
    SpecialCharEntry { symbol: "∉", english_name: "Not an Element Of", turkish_name: "Elemanı Değildir", keywords: "math set not element eleman degil" },
    SpecialCharEntry { symbol: "∅", english_name: "Empty Set", turkish_name: "Boş Küme", keywords: "math empty set null bos kume" },
    SpecialCharEntry { symbol: "∀", english_name: "For All", turkish_name: "Her Biri İçin", keywords: "math forall logic her" },
    SpecialCharEntry { symbol: "∃", english_name: "There Exists", turkish_name: "En Az Bir", keywords: "math exists logic var" },
    SpecialCharEntry { symbol: "²", english_name: "Superscript Two", turkish_name: "Üst Simge İki (Kare)", keywords: "math squared power two kare ust" },
    SpecialCharEntry { symbol: "³", english_name: "Superscript Three", turkish_name: "Üst Simge Üç (Küp)", keywords: "math cubed power three kup ust" },
    SpecialCharEntry { symbol: "½", english_name: "One Half", turkish_name: "Yarım", keywords: "math fraction half yarim bucuk" },
    SpecialCharEntry { symbol: "¼", english_name: "One Quarter", turkish_name: "Çeyrek", keywords: "math fraction quarter ceyrek" },
    SpecialCharEntry { symbol: "¾", english_name: "Three Quarters", turkish_name: "Üç Çeyrek", keywords: "math fraction three quarters uc ceyrek" },
    SpecialCharEntry { symbol: "α", english_name: "Alpha", turkish_name: "Alfa", keywords: "greek alpha alfa" },
    SpecialCharEntry { symbol: "β", english_name: "Beta", turkish_name: "Beta", keywords: "greek beta" },
    SpecialCharEntry { symbol: "γ", english_name: "Gamma", turkish_name: "Gama", keywords: "greek gamma gama" },
    SpecialCharEntry { symbol: "δ", english_name: "Delta", turkish_name: "Delta", keywords: "greek delta" },
    SpecialCharEntry { symbol: "ε", english_name: "Epsilon", turkish_name: "Epsilon", keywords: "greek epsilon" },
    SpecialCharEntry { symbol: "θ", english_name: "Theta", turkish_name: "Teta", keywords: "greek theta teta" },
    SpecialCharEntry { symbol: "λ", english_name: "Lambda", turkish_name: "Lambda", keywords: "greek lambda" },
    SpecialCharEntry { symbol: "μ", english_name: "Mu / Micro", turkish_name: "Mü / Mikro", keywords: "greek mu micro mikron" },
    SpecialCharEntry { symbol: "π", english_name: "Pi", turkish_name: "Pi", keywords: "greek pi math" },
    SpecialCharEntry { symbol: "σ", english_name: "Sigma", turkish_name: "Sigma", keywords: "greek sigma" },
    SpecialCharEntry { symbol: "φ", english_name: "Phi", turkish_name: "Fi", keywords: "greek phi fi" },
    SpecialCharEntry { symbol: "ψ", english_name: "Psi", turkish_name: "Psi", keywords: "greek psi" },
    SpecialCharEntry { symbol: "ω", english_name: "Omega", turkish_name: "Omega", keywords: "greek omega" },
    SpecialCharEntry { symbol: "Ω", english_name: "Ohm / Capital Omega", turkish_name: "Ohm / Büyük Omega", keywords: "greek omega ohm resistance direnc" },
    SpecialCharEntry { symbol: "—", english_name: "Em Dash", turkish_name: "Uzun Tire", keywords: "punctuation dash em uzun tire" },
    SpecialCharEntry { symbol: "–", english_name: "En Dash", turkish_name: "Orta Tire", keywords: "punctuation dash en orta tire" },
    SpecialCharEntry { symbol: "…", english_name: "Ellipsis", turkish_name: "Üç Nokta", keywords: "punctuation ellipsis dots uc nokta" },
    SpecialCharEntry { symbol: "•", english_name: "Bullet", turkish_name: "Madde İşareti", keywords: "punctuation bullet dot madde isareti" },
    SpecialCharEntry { symbol: "·", english_name: "Middle Dot", turkish_name: "Orta Nokta", keywords: "punctuation middle dot orta nokta" },
    SpecialCharEntry { symbol: "§", english_name: "Section Sign", turkish_name: "Bölüm İşareti", keywords: "punctuation section bolum" },
    SpecialCharEntry { symbol: "¶", english_name: "Pilcrow / Paragraph", turkish_name: "Paragraf İşareti", keywords: "punctuation paragraph pilcrow paragraf" },
    SpecialCharEntry { symbol: "©", english_name: "Copyright", turkish_name: "Telif Hakkı", keywords: "symbol copyright telif" },
    SpecialCharEntry { symbol: "®", english_name: "Registered Trademark", turkish_name: "Tescilli Marka", keywords: "symbol registered tescilli" },
    SpecialCharEntry { symbol: "™", english_name: "Trademark", turkish_name: "Ticari Marka", keywords: "symbol trademark marka" },
    SpecialCharEntry { symbol: "№", english_name: "Numero Sign", turkish_name: "Numara İşareti", keywords: "symbol numero no numara" },
    SpecialCharEntry { symbol: "«", english_name: "Left Guillemet", turkish_name: "Sol Çift Tırnak", keywords: "punctuation quote guillemet tirnak" },
    SpecialCharEntry { symbol: "»", english_name: "Right Guillemet", turkish_name: "Sağ Çift Tırnak", keywords: "punctuation quote guillemet tirnak" },
    SpecialCharEntry { symbol: "“", english_name: "Left Double Quote", turkish_name: "Sol Tırnak", keywords: "punctuation quote tirnak" },
    SpecialCharEntry { symbol: "”", english_name: "Right Double Quote", turkish_name: "Sağ Tırnak", keywords: "punctuation quote tirnak" },
    SpecialCharEntry { symbol: "✓", english_name: "Check Mark", turkish_name: "Onay İşareti", keywords: "symbol check tick onay tik" },
    SpecialCharEntry { symbol: "✔", english_name: "Heavy Check Mark", turkish_name: "Kalın Onay İşareti", keywords: "symbol check tick onay tik" },
    SpecialCharEntry { symbol: "✗", english_name: "Ballot X", turkish_name: "Çarpı İşareti", keywords: "symbol cross x carpi" },
    SpecialCharEntry { symbol: "★", english_name: "Black Star", turkish_name: "Dolu Yıldız", keywords: "symbol star yildiz" },
    SpecialCharEntry { symbol: "☆", english_name: "White Star", turkish_name: "Boş Yıldız", keywords: "symbol star outline yildiz" },
    SpecialCharEntry { symbol: "♥", english_name: "Heart", turkish_name: "Kalp", keywords: "symbol heart love kalp" },
    SpecialCharEntry { symbol: "♪", english_name: "Eighth Note", turkish_name: "Müzik Notası", keywords: "symbol music note muzik nota" },
    SpecialCharEntry { symbol: "⚠", english_name: "Warning Sign", turkish_name: "Uyarı İşareti", keywords: "symbol warning alert uyari" },
    SpecialCharEntry { symbol: "⌘", english_name: "Command Key", turkish_name: "Command Tuşu", keywords: "symbol key command mac tus" },
    SpecialCharEntry { symbol: "⌥", english_name: "Option Key", turkish_name: "Option Tuşu", keywords: "symbol key option alt tus" },
    SpecialCharEntry { symbol: "⇧", english_name: "Shift Key", turkish_name: "Shift Tuşu", keywords: "symbol key shift tus" },
    SpecialCharEntry { symbol: "⏎", english_name: "Return Symbol", turkish_name: "Enter Sembolü", keywords: "symbol key return enter tus" },
];

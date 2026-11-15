//! Karşılığı: `Extensions/Store/LocalizedText.cs`.

use crate::text;

/// Dile göre değişen mağaza metni (özet, açıklama). İndekste düz metin (her dilde aynı, "en" sayılır) ya da
/// `{ "en": "...", "tr": "..." }` nesnesi olarak gelir. [`LocalizedText::resolve`] önce istenen dili, yoksa
/// `FALLBACK_LANGUAGE` dilini, o da yoksa ilk değeri verir.
/// Değerler ekleme sırasını korur (C# `Dictionary` numaralandırması): "ilk değer" yedeği bu sıraya bağlıdır.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LocalizedText {
    values: Vec<(String, String)>,
}

impl LocalizedText {
    pub const FALLBACK_LANGUAGE: &'static str = "en";

    /// `LocalizedText.Empty`.
    pub fn empty() -> LocalizedText {
        LocalizedText { values: Vec::new() }
    }

    /// Dil kodu → metin çiftleri (ekleme sırasıyla; yinelenen kod yok varsayılır).
    pub fn new(values: Vec<(String, String)>) -> LocalizedText {
        LocalizedText { values }
    }

    /// Her dilde aynı olan düz metin.
    pub fn plain(text: &str) -> LocalizedText {
        LocalizedText { values: vec![(Self::FALLBACK_LANGUAGE.to_string(), text.to_string())] }
    }

    /// Dil kodu → metin (küçük harfli iki harfli kodlar), ekleme sırasıyla.
    pub fn values(&self) -> &[(String, String)] {
        &self.values
    }

    fn try_get(&self, code: &str) -> Option<&str> {
        self.values.iter().find(|(k, _)| k == code).map(|(_, v)| v.as_str())
    }

    /// `language`: iki harfli ISO dil kodu (ör. `CultureInfo.TwoLetterISOLanguageName`); büyük/küçük harf duyarsız, `None` olabilir.
    pub fn resolve(&self, language: Option<&str>) -> String {
        if let Some(language) = language.filter(|l| !l.is_empty())
            && let Some(exact) = self.try_get(&text::to_lower_invariant(language))
        {
            return exact.to_string();
        }

        if let Some(fallback) = self.try_get(Self::FALLBACK_LANGUAGE) {
            return fallback.to_string();
        }

        self.values.first().map(|(_, v)| v.clone()).unwrap_or_default()
    }
}

impl From<&str> for LocalizedText {
    /// C# örtük dönüşümü (`string` → `LocalizedText`).
    fn from(text: &str) -> Self {
        LocalizedText::plain(text)
    }
}

impl From<String> for LocalizedText {
    fn from(text: String) -> Self {
        LocalizedText::plain(&text)
    }
}

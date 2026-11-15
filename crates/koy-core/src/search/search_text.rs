use super::SearchCommandSafety;

/// Aramada karşılaştırılacak metni sadeleştirir. Standart küçük harfe çevirme Türkçe I/ı/İ/i için yetmez;
/// ABD klavyesiyle yazan da "sifre" yazınca "Şifre"yi bulsun diye Türkçe harfler sade karşılığına iner.
pub struct SearchText;

const COMBINING_DOT_ABOVE: char = '\u{307}';

impl SearchText {
    /// `Fold(string? text)`: `None` boş metin sayılır.
    pub fn fold(text: Option<&str>) -> String {
        Self::fold_str(text.unwrap_or(""))
    }

    /// `Fold` — boş olamayan metin için kısayol.
    pub fn fold_str(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            if c != COMBINING_DOT_ABOVE {
                out.push(Self::fold_char(c));
            }
        }
        out
    }

    fn fold_char(c: char) -> char {
        match c {
            'I' | 'İ' | 'ı' => 'i',
            'Ş' | 'ş' => 's',
            'Ğ' | 'ğ' => 'g',
            'Ü' | 'ü' => 'u',
            'Ö' | 'ö' => 'o',
            'Ç' | 'ç' => 'c',
            // char.ToLowerInvariant: UTF-16 birimi başına; BMP dışı karakterler (vekil çift) değişmez.
            _ if (c as u32) >= 0x1_0000 => c,
            _ => {
                let mut it = c.to_lowercase();
                match (it.next(), it.next()) {
                    (Some(l), None) => l,
                    _ => c,
                }
            }
        }
    }
}

/// Kullanıcının yazdığı metin ve karşılaştırma için hazırlanmış hâli.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub raw: String,
    /// Metin panodan geldi (Alt+Shift+F2); kullanıcı yazmadı.
    pub from_clipboard: bool,
    pub folded: String,
    pub words: Vec<String>,
}

impl SearchQuery {
    /// C# `SearchQuery(string? raw, bool fromClipboard = false)`: `from_clipboard` zorunlu parametre.
    pub fn new(raw: Option<&str>, from_clipboard: bool) -> Self {
        let raw = raw.map(str::trim).unwrap_or("").to_string();
        let folded = SearchText::fold_str(&raw);
        let words = folded.split(' ').filter(|w| !w.is_empty()).map(str::to_string).collect();
        SearchQuery { raw, from_clipboard, folded, words }
    }

    /// `new SearchQuery(text)` (panodan değil).
    pub fn from_text(raw: &str) -> Self {
        Self::new(Some(raw), false)
    }

    /// Komut ya da program olarak çalıştırmak onay ister: pano metni, satır sonu ya da denetim karakteri var.
    pub fn needs_command_confirmation(&self) -> bool {
        self.from_clipboard || SearchCommandSafety::has_control_characters(&self.raw)
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

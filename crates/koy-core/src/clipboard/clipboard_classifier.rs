use super::ClipboardKind;
use crate::text;
use regex::Regex;
use std::sync::OnceLock;

/// Pano metnini türe ayırır: Bağlantı, Renk, Kod, yoksa Metin. Saf fonksiyon, WinAPI yok.
///
/// Kod sezgisi (kaçırmak, düz yazıya "Kod" etiketi basmaktan iyidir):
/// - Tek satır: yalnız güçlü işaret varsa kod. Satır ; { } ile bitiyor VE ( ) { } = karakterlerinden birini
///   içeriyor, ya da "=>" / "->" ile birlikte parantez var, ya da HTML etiketi içeriyor.
/// - Çok satır: işaretlerden en az 2 ayrı çeşit gerekir (satır sonu ; { }, süslü parantez çifti, parantezli satır sonu,
///   satır başı anahtar sözcük, girinti, ok/eşitlik işleçleri, HTML etiketi). Tek işaret düz yazıda da çıkabilir.
pub struct ClipboardClassifier;

fn hex_color() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$").unwrap())
}

fn rgb_color() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)^rgba?\(\s*\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}\s*(,\s*(\d{1,3}%?|\d*\.\d+)\s*)?\)$").unwrap()
    })
}

fn keyword_line() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^((public|private|protected|internal|static|class|interface|struct|enum|namespace|using|import|from|def|function|const|let|var|return|if|else|for|foreach|while|switch|case|try|catch|SELECT|INSERT|UPDATE|DELETE|CREATE)\b.*[(){}:;=]|#include\b|#!/|@\w+)",
        )
        .unwrap()
    })
}

fn html_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"</?[a-zA-Z][a-zA-Z0-9]*(\s[^<>]*)?/?>").unwrap())
}

/// `Uri.TryCreate(s, UriKind.Absolute, ...)` ile `http`/`https` ve boş olmayan ana makine denetiminin karşılığı.
/// Yalnız `şema://yetki/...` biçimi tanınır; geçersiz ana makine karakterleri ve bozuk bağlantı noktası reddedilir.
fn is_http_url_with_host(s: &str) -> bool {
    let Some(colon) = s.find(':') else { return false };
    let scheme = &s[..colon];
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return false;
    }
    let Some(rest) = s[colon + 1..].strip_prefix("//") else { return false };
    let end = rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len());
    let mut authority = &rest[..end];
    if let Some(at) = authority.rfind('@') {
        authority = &authority[at + 1..];
    }
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        match inner.find(']') {
            Some(close) => (&authority[..close + 2], &inner[close + 1..]),
            None => return false,
        }
    } else {
        match authority.find(':') {
            Some(i) => (&authority[..i], &authority[i..]),
            None => (authority, ""),
        }
    };
    if host.is_empty() {
        return false;
    }
    if !host.starts_with('[') && host.chars().any(|c| c.is_control() || "<>\"{}|^`\\[]%".contains(c)) {
        return false;
    }
    if let Some(digits) = port.strip_prefix(':') {
        if !digits.chars().all(|c| c.is_ascii_digit()) || digits.parse::<u32>().is_ok_and(|p| p > 65535) {
            return false;
        }
    } else if !port.is_empty() {
        return false;
    }
    true
}

/// `string.Split('\n', RemoveEmptyEntries | TrimEntries)`.
fn split_trimmed(s: &str) -> Vec<&str> {
    s.split('\n').map(str::trim).filter(|l| !l.is_empty()).collect()
}

fn ends_like_code(line: &str) -> bool {
    matches!(line.chars().next_back(), Some(';' | '{' | '}'))
}

impl ClipboardClassifier {
    pub fn classify(text: Option<&str>) -> ClipboardKind {
        if text::is_null_or_white_space(text) {
            return ClipboardKind::Text;
        }

        let trimmed = text.unwrap_or("").trim();

        if Self::is_link(trimmed) {
            return ClipboardKind::Link;
        }
        if Self::is_color(trimmed) {
            return ClipboardKind::Color;
        }
        if Self::is_code(trimmed) {
            return ClipboardKind::Code;
        }
        ClipboardKind::Text
    }

    pub fn is_link(trimmed: &str) -> bool {
        if trimmed.is_empty() || trimmed.contains([' ', '\t', '\r', '\n']) {
            return false;
        }
        is_http_url_with_host(trimmed)
    }

    pub fn is_color(trimmed: &str) -> bool {
        hex_color().is_match(trimmed) || rgb_color().is_match(trimmed)
    }

    pub fn is_code(trimmed: &str) -> bool {
        let lines = split_trimmed(trimmed);
        if lines.is_empty() {
            return false;
        }

        if lines.len() == 1 {
            let line = lines[0];
            if ends_like_code(line) && line.contains(['(', ')', '{', '}', '=']) {
                return true;
            }
            if (line.contains("=>") || line.contains("->")) && line.contains('(') && line.contains(')') {
                return true;
            }
            return html_tag().is_match(line);
        }

        let raw: Vec<&str> = trimmed.split('\n').filter(|l| !l.is_empty()).collect();
        let mut score = 0;
        if lines.iter().filter(|l| ends_like_code(l)).count() >= 2 {
            score += 1;
        }
        if trimmed.contains('{') && trimmed.contains('}') {
            score += 1;
        }
        if lines.iter().any(|l| l.ends_with(')') || l.ends_with(");") || l.ends_with('{')) && trimmed.contains('(') {
            score += 1;
        }
        if lines.iter().any(|l| keyword_line().is_match(l)) {
            score += 1;
        }
        if raw.iter().filter(|l| l.starts_with("    ") || l.starts_with('\t')).count() >= 2 {
            score += 1;
        }
        if trimmed.contains("=>") || trimmed.contains("->") || trimmed.contains("==") || trimmed.contains("!=") {
            score += 1;
        }
        // Birden çok etiket (açılış+kapanış) tek başına yeterli kanıt.
        if html_tag().find_iter(trimmed).count() >= 2 {
            score += 2;
        } else if html_tag().is_match(trimmed) {
            score += 1;
        }
        score >= 2
    }
}

//! Arama kaynaklarının ortak küçük yardımcıları (C#'taki `Uri.EscapeDataString`, `string.Format`, `File.ReadAllLines`
//! ve `CultureInfo` ayrıntılarının karşılıkları).

use crate::clock::Culture;

/// `Uri.EscapeDataString`: RFC 3986 "unreserved" dışındaki her bayt (UTF-8) `%XX` olur.
pub fn escape_data_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `string.Format(culture, template, args)`: yalnız `{0}`, `{1}` ... yer tutucuları ve `{{` / `}}` kaçışları.
pub fn format_template(template: &str, args: &[&str]) -> String {
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' && chars.get(i + 1) == Some(&'{') {
            out.push('{');
            i += 2;
        } else if c == '}' && chars.get(i + 1) == Some(&'}') {
            out.push('}');
            i += 2;
        } else if c == '{' {
            if let Some(close) = chars[i + 1..].iter().position(|&x| x == '}') {
                let index: String = chars[i + 1..i + 1 + close].iter().collect();
                if let Some(arg) = index.parse::<usize>().ok().and_then(|n| args.get(n)) {
                    out.push_str(arg);
                    i += close + 2;
                    continue;
                }
            }
            out.push(c);
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// `CultureInfo.TwoLetterISOLanguageName`: kültür adının dil kısmı ("tr-TR" → "tr"; Invariant → "iv").
pub fn two_letter_language_name(culture: &Culture) -> String {
    if culture.name.is_empty() {
        return "iv".to_string();
    }
    culture.name.split('-').next().unwrap_or("").to_string()
}

/// `File.ReadAllLines`: "\r\n", "\n" ve "\r" satır sonudur; sondaki satır sonu boş satır üretmez; UTF-8 BOM atılır.
pub fn split_lines(text: &str) -> Vec<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    let mut pending = false;
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
                pending = false;
            }
            '\n' => {
                lines.push(std::mem::take(&mut current));
                pending = false;
            }
            _ => {
                current.push(c);
                pending = true;
            }
        }
    }
    if pending {
        lines.push(current);
    }
    lines
}

/// `text.Length` (UTF-16 kod birimi sayısı).
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// `Task.Delay(duration, token)`: süre dolarsa `true`, önce iptal edilirse `false`.
pub(crate) fn sleep_unless_canceled(milliseconds: u64, cancel: &crate::cancel::CancellationToken) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(milliseconds);
    loop {
        if cancel.is_cancellation_requested() {
            return false;
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return true;
        }
        std::thread::sleep((deadline - now).min(std::time::Duration::from_millis(5)));
    }
}

/// Yakalanan panik yükünden ileti metni.
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "(panik iletisi yok)".to_string()
    }
}

/// `s[..n]` (UTF-16 kod birimi sayısıyla); vekil çiftin ortasına denk gelirse yarım çift atılır.
pub(crate) fn take_utf16(s: &str, n: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let len = c.len_utf16();
        if used + len > n {
            break;
        }
        used += len;
        out.push(c);
    }
    out
}

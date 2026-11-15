//! `Uri.TryCreate(text, UriKind.Absolute, ...)` ile `AbsoluteUri` için yalnız Konumlar kaynağının ihtiyaç duyduğu
//! alt küme: `http`, `https`, `ftp` (`şema://yetki/yol?sorgu#parça`) ve `mailto:`. Başka şemalar `scheme` olarak
//! tanınır (çağıran süzer) ama `absolute_uri` ham metindir.

/// Çözülmüş mutlak adres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbsoluteUri {
    /// Küçük harfli şema.
    pub scheme: String,
    /// `Uri.AbsoluteUri`.
    pub absolute_uri: String,
}

fn default_port(scheme: &str) -> Option<&'static str> {
    match scheme {
        "http" => Some("80"),
        "https" => Some("443"),
        "ftp" => Some("21"),
        _ => None,
    }
}

/// Kaçış dışı güvenli karakter mi (RFC 3986 ayrılmış + sadeleşmiş + yüzde).
fn is_safe(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-._~:/?#[]@!$&'()*+,;=%".contains(c)
}

fn escape_unsafe(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if is_safe(c) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

pub(crate) fn try_create_absolute(text: &str) -> Option<AbsoluteUri> {
    let colon = text.find(':')?;
    let scheme_raw = &text[..colon];
    let mut sc = scheme_raw.chars();
    let valid_scheme = sc.next().is_some_and(|c| c.is_ascii_alphabetic())
        && sc.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !valid_scheme {
        return None;
    }

    let scheme = scheme_raw.to_ascii_lowercase();
    let rest = &text[colon + 1..];

    if scheme == "mailto" {
        // Kullanıcı@ana-makine biçimi gerekir.
        let (user, host) = rest.split_once('@')?;
        if user.is_empty() || host.is_empty() {
            return None;
        }
        return Some(AbsoluteUri { absolute_uri: format!("mailto:{}", escape_unsafe(rest)), scheme });
    }

    let Some(default) = default_port(&scheme) else {
        // Tanınmayan şema: çağıran süzer.
        return Some(AbsoluteUri { absolute_uri: text.to_string(), scheme });
    };

    let after = rest.strip_prefix("//")?;
    let authority_end = after.find(['/', '?', '#', '\\']).unwrap_or(after.len());
    let mut authority = &after[..authority_end];
    let tail = &after[authority_end..];

    let mut userinfo = "";
    if let Some(at) = authority.rfind('@') {
        userinfo = &authority[..at];
        authority = &authority[at + 1..];
    }

    let (host, port) = match authority.rfind(':') {
        Some(i) if !authority.ends_with(']') => (&authority[..i], &authority[i + 1..]),
        _ => (authority, ""),
    };
    if host.is_empty() || host.chars().any(|c| c.is_control() || "<>\"{}|^`\\ %".contains(c)) {
        return None;
    }
    if !port.chars().all(|c| c.is_ascii_digit()) || port.parse::<u32>().is_ok_and(|p| p > 65535) {
        return None;
    }

    let mut out = format!("{scheme}://");
    if !userinfo.is_empty() {
        out.push_str(userinfo);
        out.push('@');
    }
    out.push_str(&host.to_lowercase());
    if !port.is_empty() && port != default {
        out.push(':');
        out.push_str(port);
    }

    // Yol, sorgu ve parça.
    let tail = tail.replace('\\', "/");
    if tail.is_empty() || !tail.starts_with('/') {
        out.push('/');
    }
    out.push_str(&escape_unsafe(&tail));
    Some(AbsoluteUri { scheme, absolute_uri: out })
}

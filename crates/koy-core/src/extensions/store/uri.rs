//! `System.Uri` için mağazanın ihtiyaç duyduğu alt küme (`Uri.TryCreate(text, UriKind.Absolute, ...)`).
//!
//! `StoreFieldRules.IsHttpsUrl` ve `StoreUrlPolicy` yalnız `https` adreslerin şema, ana makine, kullanıcı bilgisi,
//! varsayılan bağlantı noktası ve kanonik mutlak yoluna bakar. Başka şemalar yalnız `scheme` olarak tanınır. .NET
//! gibi: şema ve ana makine küçük harfe çevrilir, `\` yolda `/` sayılır, kaçışsız (unreserved) `%XX` çözülür ve
//! `.` / `..` bileşenleri sıkıştırılır (`AbsolutePath` bu kanonik hâldir: ".../koy/../evil/a" → ".../evil/a").

use std::fmt;

/// `UriFormatException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriFormatError;

impl fmt::Display for UriFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invalid URI")
    }
}

impl std::error::Error for UriFormatError {}

/// Çözülmüş mutlak adres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uri {
    scheme: String,
    user_info: String,
    host: String,
    port: Option<u16>,
    absolute_path: String,
}

impl Uri {
    /// `new Uri(text)`: geçersizse hata.
    pub fn new(text: &str) -> Result<Uri, UriFormatError> {
        Self::try_create(text).ok_or(UriFormatError)
    }

    /// `Uri.TryCreate(text, UriKind.Absolute, out uri)`.
    pub fn try_create(text: &str) -> Option<Uri> {
        // .NET baştaki/sondaki boşluğu (<= ' ') kırpar.
        let text = text.trim_matches(|c: char| c <= ' ');
        if text.is_empty() {
            return None;
        }

        let Some(colon) = text.find(':') else {
            return Self::implicit_file(text);
        };
        let scheme_raw = &text[..colon];
        let mut chars = scheme_raw.chars();
        let valid_scheme = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if !valid_scheme {
            return Self::implicit_file(text);
        }

        let rest = &text[colon + 1..];
        if scheme_raw.len() == 1 {
            // "C:\dosya" ya da "C:/dosya": örtük dosya yolu.
            return if rest.starts_with(['\\', '/']) { Some(Self::file()) } else { None };
        }

        let scheme = scheme_raw.to_ascii_lowercase();
        match scheme.as_str() {
            "http" | "https" => {}
            // Tanınmayan ya da ana makinesiz şema: çağıran şemaya bakıp eler.
            _ => return Some(Uri { scheme, user_info: String::new(), host: String::new(), port: None, absolute_path: String::new() }),
        };

        // Yalnız "//" ile başlayan yetki; ters bölü de .NET'te eğik çizgi sayılır.
        let normalized: String = rest.chars().map(|c| if c == '\\' { '/' } else { c }).collect();
        let after = normalized.strip_prefix("//")?;
        let authority_end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let authority = &after[..authority_end];
        let tail = &after[authority_end..];

        let (user_info, host_port) = match authority.rfind('@') {
            Some(at) => (&authority[..at], &authority[at + 1..]),
            None => ("", authority),
        };

        let (host_text, port_text) = if host_port.starts_with('[') {
            let close = host_port.find(']')?;
            let after_host = &host_port[close + 1..];
            let port = if after_host.is_empty() { "" } else { after_host.strip_prefix(':')? };
            (&host_port[..=close], port)
        } else {
            match host_port.rfind(':') {
                Some(c) => (&host_port[..c], &host_port[c + 1..]),
                None => (host_port, ""),
            }
        };

        if host_text.is_empty() || !is_valid_host(host_text) {
            return None;
        }

        let port = if port_text.is_empty() {
            None
        } else {
            if !port_text.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            Some(port_text.parse::<u16>().ok()?)
        };

        let path_end = tail.find(['?', '#']).unwrap_or(tail.len());
        let raw_path = &tail[..path_end];
        let absolute_path = if raw_path.is_empty() { "/".to_string() } else { canonical_path(raw_path) };

        Some(Uri {
            scheme,
            user_info: user_info.to_string(),
            host: crate::text::to_lower_invariant(host_text),
            port,
            absolute_path,
        })
    }

    // "//sunucu/paylaşım" (UNC) ve "C:/..." Windows'ta mutlak dosya adresi sayılır.
    fn implicit_file(text: &str) -> Option<Uri> {
        if text.starts_with("//") || text.starts_with("\\\\") { Some(Self::file()) } else { None }
    }

    fn file() -> Uri {
        Uri { scheme: "file".to_string(), user_info: String::new(), host: String::new(), port: None, absolute_path: String::new() }
    }

    /// `Uri.Scheme` (küçük harf).
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// `Uri.UserInfo`.
    pub fn user_info(&self) -> &str {
        &self.user_info
    }

    /// `Uri.Host` (küçük harf).
    pub fn host(&self) -> &str {
        &self.host
    }

    /// `Uri.IsDefaultPort`: bağlantı noktası yazılmamış ya da şemanın varsayılanı (http 80, https 443).
    pub fn is_default_port(&self) -> bool {
        match (self.scheme.as_str(), self.port) {
            (_, None) => true,
            ("http", Some(p)) => p == 80,
            ("https", Some(p)) => p == 443,
            _ => false,
        }
    }

    /// `Uri.AbsolutePath`: kanonik yol (boşsa "/").
    pub fn absolute_path(&self) -> &str {
        &self.absolute_path
    }

    /// `Uri.IsAbsoluteUri`: bu tip yalnız mutlak adres taşır.
    pub fn is_absolute_uri(&self) -> bool {
        true
    }
}

// Harf, rakam, "-", ".", "_" ve ASCII dışı (uluslararası alan adı); IPv6 köşeli parantezli.
fn is_valid_host(host: &str) -> bool {
    if let Some(inner) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return !inner.is_empty() && inner.chars().all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '.'));
    }

    host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_') || !c.is_ascii())
}

// Kaçışsız `%XX` çözülür (yalnız unreserved: harf, rakam, "-", ".", "_", "~"), sonra "." ve ".." sıkıştırılır.
fn canonical_path(raw: &str) -> String {
    let decoded = decode_unreserved(raw);
    let segments: Vec<&str> = decoded.split('/').collect();
    let mut stack: Vec<&str> = Vec::new();
    let mut trailing_slash = false;
    // segments[0] boştur (yol "/" ile başlar); son bileşen "", "." ya da ".." ise sonda "/" kalır.
    for (i, segment) in segments.iter().enumerate().skip(1) {
        let last = i == segments.len() - 1;
        match *segment {
            "." => trailing_slash = last,
            ".." => {
                stack.pop();
                trailing_slash = last;
            }
            "" => trailing_slash = last,
            s => {
                stack.push(s);
                trailing_slash = false;
            }
        }
    }

    let mut path = String::from("/");
    path.push_str(&stack.join("/"));
    if trailing_slash && !stack.is_empty() {
        path.push('/');
    }
    path
}

fn decode_unreserved(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit) && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit) {
            let value = u8::from_str_radix(&raw[i + 1..i + 3], 16).unwrap_or(b'%');
            if value.is_ascii_alphanumeric() || matches!(value, b'-' | b'.' | b'_' | b'~') {
                out.push(value as char);
            } else {
                out.push('%');
                out.push_str(&raw[i + 1..i + 3].to_ascii_uppercase());
            }
            i += 3;
        } else {
            let c = raw[i..].chars().next().unwrap_or('%');
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_parts() {
        let uri = Uri::new("HTTPS://User:pw@Raw.GitHubUserContent.com:443/koyhq/koy/a?x=1#f").unwrap();
        assert_eq!("https", uri.scheme());
        assert_eq!("User:pw", uri.user_info());
        assert_eq!("raw.githubusercontent.com", uri.host());
        assert!(uri.is_default_port());
        assert_eq!("/koyhq/koy/a", uri.absolute_path());
    }

    #[test]
    fn non_default_port_and_empty_path() {
        let uri = Uri::new("https://example.com:8443").unwrap();
        assert!(!uri.is_default_port());
        assert_eq!("/", uri.absolute_path());
        assert!(Uri::try_create("https://example.com:99999/").is_none());
        assert!(Uri::try_create("https://example.com:ab/").is_none());
    }

    #[test]
    fn dot_segments_are_compressed_like_dotnet() {
        assert_eq!("/koyhq/evil/a", Uri::new("https://h.com/koyhq/koy/../evil/a").unwrap().absolute_path());
        assert_eq!("/koyhq/koy/b", Uri::new("https://h.com/koyhq/koy/a/../b").unwrap().absolute_path());
        assert_eq!("/a/", Uri::new("https://h.com/a/b/..").unwrap().absolute_path());
        assert_eq!("/a/b", Uri::new("https://h.com/a/%2e/%62").unwrap().absolute_path());
        assert_eq!("/", Uri::new("https://h.com/../..").unwrap().absolute_path());
    }

    #[test]
    fn other_schemes_and_garbage() {
        assert_eq!("ftp", Uri::new("ftp://example.com/p").unwrap().scheme());
        assert_eq!("file", Uri::new("file:///c:/p.koyext").unwrap().scheme());
        assert_eq!("file", Uri::new("//example.com/p").unwrap().scheme());
        assert!(Uri::try_create("example.com/p").is_none());
        assert!(Uri::try_create("").is_none());
        assert!(Uri::try_create("https://").is_none());
        assert!(Uri::try_create("https:example.com").is_none());
        assert!(Uri::try_create("https://exa mple.com/p").is_none());
    }
}

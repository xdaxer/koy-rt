use super::dock_item_rules::DockItemRules;
use crate::text;

/// Özel düğmenin çalıştıracağı hedef ve (yalnız dosya hedeflerinde) bağımsız değişken metni.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DockCustomLaunch {
    pub target: String,
    pub arguments: Option<String>,
}

impl DockCustomLaunch {
    pub fn new(target: impl Into<String>, arguments: Option<String>) -> Self {
        DockCustomLaunch { target: target.into(), arguments }
    }
}

/// Özel düğme komutunun güvenli ayrıştırması. Komut ayar dosyasından gelir ve güvenilmez sayılır:
/// kabuk komut satırı kurulmaz, hiçbir şey birleştirilmez; çıkan hedef ve bağımsız değişken ayrı ayrı çalıştırıcıya gider.
/// Kabul edilenler: tam yollu dosya/klasör (sürücü harfi ya da UNC) ve http, https, mailto adresleri.
/// Göreli yol, shell:, ::{CLSID}, ms-* ve benzeri özel şemalar, denetim karakterleri reddedilir.
pub struct DockCustomCommand;

fn is_invalid_path_char(c: char) -> bool {
    matches!(c, '"' | '<' | '>' | '|' | '*' | '?')
}

impl DockCustomCommand {
    /// C# `TryParse(command, out launch)`: başarısızsa `None`.
    pub fn try_parse(command: Option<&str>) -> Option<DockCustomLaunch> {
        let command = command?;
        if text::is_null_or_white_space(Some(command)) {
            return None;
        }

        let text = text::trim(command);
        if text.encode_utf16().count() > DockItemRules::MAX_COMMAND_LENGTH as usize || text.chars().any(char::is_control) {
            return None;
        }

        let target: String;
        let mut arguments: Option<String> = None;

        if text.starts_with('"') {
            // text.IndexOf('"', 1): ilk karakter ASCII olduğundan bayt konumu 1'den başlar.
            let close = text[1..].find('"').map(|rel| rel + 1);
            let close = match close {
                Some(c) if c > 1 => c,
                _ => return None,
            };

            target = text[1..close].to_string();
            let rest = text::trim(&text[close + 1..]);
            arguments = if rest.is_empty() { None } else { Some(rest.to_string()) };
        } else {
            target = text.to_string();
        }

        if Self::is_url(&target) {
            // Adres yalnız adres olarak açılır; bağımsız değişken eklenemez.
            if arguments.is_some() {
                return None;
            }
            return Some(DockCustomLaunch::new(target, None));
        }

        if !Self::is_full_path(&target) || target.chars().any(is_invalid_path_char) {
            return None;
        }

        Some(DockCustomLaunch::new(target, arguments))
    }

    fn is_url(target: &str) -> bool {
        let web = text::starts_with_ignore_case(target, "http://") || text::starts_with_ignore_case(target, "https://");
        let mail = text::starts_with_ignore_case(target, "mailto:");
        if !web && !mail {
            return false;
        }

        // `Uri.TryCreate(target, UriKind.Absolute)` ve şema denetimi: .NET'in Uri ayrıştırıcısının karşılığı yok;
        // burada yaklaşığı kullanılır (http/https için dolu ve geçerli karakterli konak adı, mailto için dolu adres).
        // C# ile aynı: boşluk içeren metin reddedilir.
        if target.chars().any(char::is_whitespace) {
            return false;
        }

        if mail {
            return target.len() > "mailto:".len();
        }

        let after_scheme = &target[target.find("//").map_or(0, |i| i + 2)..];
        let authority_end = after_scheme.find(['/', '?', '#']).unwrap_or(after_scheme.len());
        let authority = &after_scheme[..authority_end];
        let host_port = authority.rsplit('@').next().unwrap_or("");
        let host = if let Some(stripped) = host_port.strip_prefix('[') {
            stripped.split(']').next().unwrap_or("")
        } else {
            host_port.split(':').next().unwrap_or("")
        };
        if host.is_empty() {
            return false;
        }

        !host.chars().any(|c| matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '\\' | '^' | '`'))
    }

    fn is_full_path(target: &str) -> bool {
        let chars: Vec<char> = target.chars().collect();
        if chars.len() >= 3 && chars[0].is_ascii_alphabetic() && chars[1] == ':' && (chars[2] == '\\' || chars[2] == '/') {
            return true;
        }

        // UNC: \\sunucu\paylasim. \\?\ ve \\.\ aygit yollari kabul edilmez.
        target.starts_with("\\\\") && chars.len() > 2 && chars[2] != '?' && chars[2] != '.' && chars[2] != '\\'
    }
}

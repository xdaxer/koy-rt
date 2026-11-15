//! Karşılığı: `Input/CustomAction.cs`.

use serde::{Deserialize, Serialize};

use super::input_binding_parser::try_parse_name;
use super::input_binding_parser::NamedValues;
use crate::text;

/// Özel eylemin türü. Koy yalnız bu üç türü bilir; Core hiçbirini çalıştırmaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum CustomActionKind {
    #[default]
    RunCommand,
    OpenUrl,
    OpenPath,
}

impl NamedValues for CustomActionKind {
    fn values() -> &'static [Self] {
        &[CustomActionKind::RunCommand, CustomActionKind::OpenUrl, CustomActionKind::OpenPath]
    }
}

/// Kullanıcının tanımladığı eylem: yalnız veri. Çalıştırma (Process.Start) App/Platform'da, Koy'un kendi sürecinde yapılır
/// (AGENTS #4); Core komut çalıştırmaz.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomAction {
    pub id: String,
    pub kind: CustomActionKind,
    pub target: String,
    pub arguments: Option<String>,
    pub working_directory: Option<String>,
}

/// Ayardan okunan ham özel eylem.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CustomActionData {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub target: Option<String>,
    pub arguments: Option<String>,
    pub working_directory: Option<String>,
}

impl CustomActionData {
    /// `new CustomActionData(id, kind, target)` (öteki alanlar null).
    pub fn new(id: Option<&str>, kind: Option<&str>, target: Option<&str>) -> Self {
        CustomActionData {
            id: id.map(str::to_string),
            kind: kind.map(str::to_string),
            target: target.map(str::to_string),
            arguments: None,
            working_directory: None,
        }
    }
}

/// C#: iç içe `CustomActionParser.Result`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomActionParseResult {
    pub accepted: Vec<CustomAction>,
    pub dropped: i32,
}

/// Özel eylemleri doğrular. Kimlik zorunlu ve tekil (büyük-küçük harf duyarsız; ilk gelen kalır), tür bilinmeli,
/// hedef boş olmamalı. URL yalnız mutlak http/https olabilir. Geçersiz kayıt sessizce düşer ve sayılır.
/// En çok `MAX_ACTIONS` eylem tutulur; fazlası düşer (elle bozulmuş dosya listeyi şişirmesin).
pub struct CustomActionParser;

impl CustomActionParser {
    pub const MAX_ACTIONS: usize = 32;

    pub fn parse(items: impl IntoIterator<Item = Option<CustomActionData>>) -> CustomActionParseResult {
        let mut accepted: Vec<CustomAction> = Vec::new();
        // OrdinalIgnoreCase küme: en çok MAX_ACTIONS kimlik tutulduğundan doğrusal arama yeter.
        let mut ids: Vec<String> = Vec::new();
        let mut dropped = 0;

        for item in items {
            // JSON'daki null öğe okunurken null olarak gelir; bütün listeyi çökertmesin.
            let id = item.as_ref().and_then(|i| i.id.as_deref()).map(text::trim);
            let target = item.as_ref().and_then(|i| i.target.as_deref()).map(text::trim);

            // Kimlik yalnız kayıt geçerliyse rezerve edilir; geçersiz kayıt sonraki geçerli kaydı düşürmesin.
            let valid = (|| {
                let item = item.as_ref()?;
                if accepted.len() >= Self::MAX_ACTIONS {
                    return None;
                }
                let id = id.filter(|s| !s.is_empty())?;
                let target = target.filter(|s| !s.is_empty())?;
                let kind = try_parse_name::<CustomActionKind>(item.kind.as_deref())?;
                if kind == CustomActionKind::OpenUrl && !Self::is_web_url(target) {
                    return None;
                }
                // `ids.Add(id)`: zaten varsa false.
                if ids.iter().any(|known| text::eq_ignore_case(known, id)) {
                    return None;
                }
                Some((id, target, kind))
            })();

            let (Some(item), Some((id, target, kind))) = (item.as_ref(), valid) else {
                dropped += 1;
                continue;
            };

            ids.push(id.to_string());
            accepted.push(CustomAction {
                id: id.to_string(),
                kind,
                target: target.to_string(),
                arguments: Self::blank_to_none(item.arguments.as_deref()),
                working_directory: Self::blank_to_none(item.working_directory.as_deref()),
            });
        }

        CustomActionParseResult { accepted, dropped }
    }

    /// `string.IsNullOrWhiteSpace(x) ? null : x.Trim()`.
    fn blank_to_none(value: Option<&str>) -> Option<String> {
        if text::is_null_or_white_space(value) {
            None
        } else {
            value.map(|v| text::trim(v).to_string())
        }
    }

    /// `Uri.TryCreate(text, UriKind.Absolute, out var uri)` + şema http/https denetimi.
    /// .NET'in tam URI ayrıştırıcısı taşınmadı: "http(s)://" + boş olmayan, boşluk/denetim karakteri içermeyen
    /// bir sunucu bölümü (kullanıcı bilgisi ve kapı hariç) aranır.
    fn is_web_url(value: &str) -> bool {
        let Some(colon) = value.find(':') else {
            return false;
        };
        let scheme = &value[..colon];
        if !(text::eq_ignore_case(scheme, "http") || text::eq_ignore_case(scheme, "https")) {
            return false;
        }
        let Some(rest) = value[colon + 1..].strip_prefix("//") else {
            return false;
        };
        if rest.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return false;
        }

        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let authority = &rest[..authority_end];
        let host_port = authority.rsplit('@').next().unwrap_or("");
        let host = if host_port.starts_with('[') {
            // IPv6: [::1]:8080
            host_port.find(']').map(|end| &host_port[..=end]).unwrap_or("")
        } else {
            host_port.split(':').next().unwrap_or("")
        };

        !host.is_empty()
    }
}

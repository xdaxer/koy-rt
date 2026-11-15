use super::guid::Guid;
use crate::time::DateTimeOffset;
use serde::{Deserialize, Serialize};

/// Pano öğesinin türü (sınıflandırma: [`super::ClipboardClassifier`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ClipboardKind {
    #[default]
    Text,
    Link,
    Color,
    Code,
}

/// Arayüzdeki süzgeç sekmeleri: Tümü / Sabit / Bağlantı / Renk / Kod.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ClipboardFilter {
    #[default]
    All,
    Pinned,
    Link,
    Color,
    Code,
}

/// Tek pano kaydı. Değişmez; sabitleme ve taşıma yeni kayıt üretir.
///
/// JSON alan adları C# kaynak üreticisiyle aynı (PascalCase, adlandırma ilkesi yok); `CopiedAt` .NET'in
/// `DateTimeOffset` biçiminde (`2026-11-06T10:00:00+00:00`) yazılır.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ClipboardItem {
    pub id: Guid,
    pub text: String,
    pub kind: ClipboardKind,
    pub pinned: bool,
    #[serde(with = "dotnet_date_time_offset")]
    pub copied_at: DateTimeOffset,
}

impl ClipboardItem {
    pub fn new(id: Guid, text: impl Into<String>, kind: ClipboardKind, pinned: bool, copied_at: DateTimeOffset) -> Self {
        ClipboardItem { id, text: text.into(), kind, pinned, copied_at }
    }
}

/// System.Text.Json'ın `DateTimeOffset` biçimi: `yyyy-MM-ddTHH:mm:ss[.fffffff]+hh:mm` (kesir sondaki sıfırsız).
mod dotnet_date_time_offset {
    use crate::time::DateTimeOffset;
    use chrono::{DateTime, Timelike};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &DateTimeOffset, s: S) -> Result<S::Ok, S::Error> {
        let mut text = value.format("%Y-%m-%dT%H:%M:%S").to_string();
        let ticks = value.nanosecond() / 100;
        if ticks > 0 {
            let fraction = format!("{ticks:07}");
            text.push('.');
            text.push_str(fraction.trim_end_matches('0'));
        }
        text.push_str(&value.format("%:z").to_string());
        s.serialize_str(&text)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTimeOffset, D::Error> {
        let text = String::deserialize(d)?;
        DateTime::parse_from_rfc3339(&text).map_err(serde::de::Error::custom)
    }
}

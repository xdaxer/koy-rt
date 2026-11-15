//! Karşılığı: `koy/src/Platform/Shelf/ShelfJsonContext.cs`.
//!
//! `WriteIndented = true, UseStringEnumConverter = true` (System.Text.Json): özellik adları PascalCase, `Kind` metin,
//! `AddedAt` ISO 8601 (`2025-01-01T12:00:00.1234567+00:00`; kesir yalnız sıfır değilse ve sondaki sıfırlar atılarak).
//! Core'daki `ShelfItem` serde taşımadığı için bu dosyada eşlenir.

use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Timelike};
use koy_core::shelf::{ShelfItem, ShelfItemKind};
use koy_core::time::DateTimeOffset;
use serde::{Deserialize, Serialize};

/// `default(DateTimeOffset)`: 0001-01-01T00:00:00+00:00.
fn default_added_at() -> String {
    "0001-01-01T00:00:00+00:00".to_string()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ShelfItemJson {
    #[serde(default)]
    id: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    path_or_content: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default = "default_added_at")]
    added_at: String,
}

fn default_kind() -> String {
    "File".to_string()
}

pub(crate) struct ShelfJsonContext;

impl ShelfJsonContext {
    /// `JsonSerializer.Serialize(items, ShelfJsonContext.Default.ListShelfItem)`.
    pub(crate) fn serialize(items: &[ShelfItem]) -> String {
        let list: Vec<ShelfItemJson> = items
            .iter()
            .map(|i| ShelfItemJson {
                id: i.id.clone(),
                display_name: i.display_name.clone(),
                path_or_content: i.path_or_content.clone(),
                kind: kind_name(i.kind).to_string(),
                added_at: format_offset(&i.added_at),
            })
            .collect();
        // STJ girintisi `Environment.NewLine` (Windows'ta CRLF) kullanır; JSON metinlerinde satır sonu kaçışlı olduğundan
        // düz değiştirme güvenlidir.
        serde_json::to_string_pretty(&list).unwrap_or_default().replace('\n', "\r\n")
    }

    /// `JsonSerializer.Deserialize(json, ShelfJsonContext.Default.ListShelfItem)`: `null` JSON'u `Ok(None)`; bozuk biçim
    /// ya da bilinmeyen `Kind` (STJ'de istisna) `Err`.
    pub(crate) fn deserialize(json: &str) -> Result<Option<Vec<ShelfItem>>, String> {
        let list: Option<Vec<ShelfItemJson>> = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let Some(list) = list else { return Ok(None) };
        let mut items = Vec::with_capacity(list.len());
        for entry in list {
            let kind = parse_kind(&entry.kind).ok_or_else(|| format!("bilinmeyen Kind: {}", entry.kind))?;
            let added_at = parse_offset(&entry.added_at).ok_or_else(|| format!("bozuk AddedAt: {}", entry.added_at))?;
            items.push(ShelfItem::new(entry.id, entry.display_name, entry.path_or_content, kind, added_at));
        }
        Ok(Some(items))
    }
}

fn kind_name(kind: ShelfItemKind) -> &'static str {
    match kind {
        ShelfItemKind::File => "File",
        ShelfItemKind::Directory => "Directory",
        ShelfItemKind::Text => "Text",
    }
}

/// STJ'nin enum dönüştürücüsü adı büyük/küçük harf duyarsız okur.
fn parse_kind(name: &str) -> Option<ShelfItemKind> {
    [ShelfItemKind::File, ShelfItemKind::Directory, ShelfItemKind::Text]
        .into_iter()
        .find(|k| kind_name(*k).eq_ignore_ascii_case(name))
}

fn format_offset(value: &DateTimeOffset) -> String {
    let mut text = value.format("%Y-%m-%dT%H:%M:%S").to_string();
    let ticks = value.nanosecond() / 100;
    if ticks != 0 {
        let fraction = format!("{ticks:07}");
        text.push('.');
        text.push_str(fraction.trim_end_matches('0'));
    }
    text.push_str(&value.format("%:z").to_string());
    text
}

fn parse_offset(text: &str) -> Option<DateTimeOffset> {
    if text == default_added_at() {
        let zero = NaiveDate::from_ymd_opt(1, 1, 1)?.and_hms_opt(0, 0, 0)?;
        return FixedOffset::east_opt(0)?.from_local_datetime(&zero).single();
    }

    DateTime::parse_from_rfc3339(text).ok()
}

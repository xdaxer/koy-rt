//! Karşılığı: `Lyrics/LrclibApi.cs`.

use std::fmt::Write;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use super::lyrics_query::LyricsQuery;
use crate::text;

const BASE_URL: &str = "https://lrclib.net/api";

// Oturumun bildirdiği süre saniyeye yuvarlanmış gelebilir; farklı kayıtların (radyo sürümü, canlı) ayrımı için dar tutulur.
const DURATION_TOLERANCE_SECONDS: f64 = 3.0;

/// LRCLIB adresleri ve yanıt çözümleme. Ağ çağrısı Platform'dadır, burası saf ve testlidir.
pub struct LrclibApi;

impl LrclibApi {
    pub fn get_url(query: &LyricsQuery) -> String {
        let mut url = String::from(BASE_URL);
        url.push_str("/get?track_name=");
        url.push_str(&escape_data_string(&query.title));
        url.push_str("&artist_name=");
        url.push_str(&escape_data_string(&query.artist));

        // Hizmet 1-3600 saniye dışındaki süreleri kabul etmez.
        if let Some(duration) = query.duration {
            let total = duration.total_seconds();
            if (1.0..=3600.0).contains(&total) {
                url.push_str("&duration=");
                url.push_str(&(text::round(total) as i32).to_string());
            }
        }

        url
    }

    pub fn search_url(query: &LyricsQuery) -> String {
        format!(
            "{BASE_URL}/search?track_name={}&artist_name={}",
            escape_data_string(&query.title),
            escape_data_string(&query.primary_artist())
        )
    }

    /// Tek kayıt yanıtındaki eşzamanlı söz; yoksa ya da yanıt bozuksa `None`.
    pub fn parse_synced(json: &str) -> Option<String> {
        let document: Value = serde_json::from_str(json).ok()?;
        if document.is_object() { read_synced(&document) } else { None }
    }

    /// Arama sonuçlarından parçaya en yakın eşzamanlı sözü seçer. Süre biliniyorsa toleransı aşan kayıt elenir:
    /// aynı adlı başka bir sürümün sözleri yanlış yerde akardı.
    pub fn pick_best(json: &str, query: &LyricsQuery) -> Option<String> {
        let document: Value = serde_json::from_str(json).ok()?;
        let items = document.as_array()?;

        let mut best: Option<String> = None;
        let mut best_difference = f64::MAX;

        for item in items {
            if !item.is_object() {
                continue;
            }
            let Some(synced) = read_synced(item) else { continue };
            if !title_matches(item, &query.title) {
                continue;
            }

            let Some(duration) = query.duration else {
                return Some(synced);
            };

            let Some(value) = item.get("duration").and_then(Value::as_f64) else { continue };

            let difference = (value - duration.total_seconds()).abs();
            if difference <= DURATION_TOLERANCE_SECONDS && difference < best_difference {
                best = Some(synced);
                best_difference = difference;
            }
        }

        best
    }
}

/// `Uri.EscapeDataString`: RFC 3986 ayrılmamış karakterler (`A-Z a-z 0-9 - _ . ~`) olduğu gibi, gerisi UTF-8 baytları
/// büyük harfli `%XX`.
fn escape_data_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

fn read_synced(element: &Value) -> Option<String> {
    match element.get("syncedLyrics") {
        Some(Value::String(s)) if !text::is_null_or_white_space(Some(s)) => Some(s.clone()),
        _ => None,
    }
}

// Arama gevşektir; başlığı tutmayan kayıt başka bir şarkıdır.
fn title_matches(item: &Value, title: &str) -> bool {
    let Some(Value::String(track_name)) = item.get("trackName") else { return false };

    let candidate = comparable(Some(track_name));
    let wanted = comparable(Some(title));
    !candidate.is_empty() && !wanted.is_empty() && (candidate.contains(&wanted) || wanted.contains(&candidate))
}

// `char.IsLetterOrDigit` (L* ya da Nd; UTF-16 kod birimi başına, yani BMP dışı karakterler harf sayılmaz) sonra
// `ToLowerInvariant`.
fn comparable(value: Option<&str>) -> String {
    static LETTER_OR_DIGIT: OnceLock<Regex> = OnceLock::new();
    let re = LETTER_OR_DIGIT.get_or_init(|| Regex::new(r"[\p{L}\p{Nd}]").unwrap());

    let kept: String = value
        .unwrap_or("")
        .chars()
        .filter(|c| c.len_utf16() == 1 && re.is_match(c.encode_utf8(&mut [0u8; 4])))
        .collect();
    text::to_lower_invariant(&kept)
}

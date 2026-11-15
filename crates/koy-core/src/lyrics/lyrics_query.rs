//! Karşılığı: `Lyrics/LyricsQuery.cs`.

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

use crate::text;
use crate::time::TimeSpan;

const MAX_FIELD_LENGTH: usize = 200;

fn title_noise() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)\s*[\(\[][^\)\]]*\b(?:official|lyrics?|audio|video|visuali[sz]er|hd|4k|remaster(?:ed)?|feat|ft|resmi|klip|sözleri?)\b[^\)\]]*[\)\]]",
        )
        .unwrap()
    })
}

fn trailing_remaster() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)\s+-\s+(?:\d{4}\s+)?(?:digital\s+)?remaster(?:ed)?(?:\s+\d{4})?\s*$").unwrap())
}

fn channel_suffix() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)(?:\s+-\s+Topic|VEVO)\s*$").unwrap())
}

/// Söz aranacak parça. Süre bilinmiyorsa `None`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LyricsQuery {
    pub title: String,
    pub artist: String,
    pub duration: Option<TimeSpan>,
}

impl LyricsQuery {
    pub fn new(title: impl Into<String>, artist: impl Into<String>, duration: Option<TimeSpan>) -> Self {
        LyricsQuery { title: title.into(), artist: artist.into(), duration }
    }

    /// Parça kimliği. Süre dışarıda: sonradan gelen süre aynı parçayı yeniden yüklemesin.
    pub fn key(&self) -> String {
        format!("{}\n{}", text::to_lower_invariant(&self.artist), text::to_lower_invariant(&self.title))
    }

    /// "A, B" gibi birleşik sanatçı alanında ilk ad; söz veritabanları çoğunlukla onu tutar.
    pub fn primary_artist(&self) -> String {
        // Split(',', 2)[0]
        let first = self.artist.split(',').next().unwrap_or("");
        text::trim(first).to_string()
    }

    /// Medya oturumundaki ham başlık ve sanatçıdan sorgu üretir. Tarayıcıdaki videolarda başlık "Sanatçı - Parça (Official
    /// Video)", sanatçı kanal adı olur; bu gürültü temizlenmezse hiçbir kaynak eşleşmez. Başlık yoksa `None`.
    pub fn create(raw_title: Option<&str>, raw_artist: Option<&str>, duration: TimeSpan) -> Option<LyricsQuery> {
        let without_noise = title_noise().replace_all(raw_title.unwrap_or(""), "");
        let mut title = text::trim(&trailing_remaster().replace_all(&without_noise, "")).to_string();
        let artist_trimmed = text::trim(raw_artist.map(text::trim).unwrap_or(""));
        let mut artist = text::trim(&channel_suffix().replace_all(artist_trimmed, "")).to_string();

        if let Some(dash) = title.find(" - ") {
            if dash > 0 {
                let left = text::trim(&title[..dash]).to_string();
                let right = text::trim(&title[dash + 3..]).to_string();
                if !right.is_empty() && (artist.is_empty() || names_overlap(&left, &artist)) {
                    artist = left;
                    title = right;
                }
            }
        }

        // Uzunluklar UTF-16 kod birimi (C# `string.Length`).
        let title_len = title.encode_utf16().count();
        let artist_len = artist.encode_utf16().count();
        if title_len == 0 || title_len > MAX_FIELD_LENGTH || artist_len > MAX_FIELD_LENGTH {
            return None;
        }

        Some(LyricsQuery::new(title, artist, if duration > TimeSpan::ZERO { Some(duration) } else { None }))
    }
}

// Kanal adı çoğu zaman sanatçı adına ek taşır ("Queen Official"). Harf değil kelime karşılaştırılır:
// "A" sanatçısı "Anything - Else" başlığını ayırmasın.
fn names_overlap(first: &str, second: &str) -> bool {
    let a = words(first);
    let b = words(second);
    if a.is_empty() || b.is_empty() {
        return false;
    }

    if a.len() <= b.len() { a.is_subset(&b) } else { b.is_subset(&a) }
}

fn words(value: &str) -> HashSet<String> {
    // Split(' ', RemoveEmptyEntries | TrimEntries): önce kırp, sonra boşları at.
    text::to_lower_invariant(value).split(' ').map(text::trim).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

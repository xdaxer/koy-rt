//! Karşılığı: `koy/src/Core/Updates/ChangelogParser.cs`.

use std::sync::LazyLock;

use regex::Regex;

use crate::settings::AppLanguage;
use crate::text::{eq_ignore_case, is_null_or_white_space, starts_with_ignore_case, to_upper_invariant, to_upper_tr};

/// Ayrıştırılmış sürüm notları bilgisi.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseNotes {
    pub version: String,
    pub compare_url: Option<String>,
    pub date: Option<String>,
    pub summary: Option<String>,
    pub highlights: Vec<String>,
    pub sections: Vec<ReleaseSection>,
    pub raw_markdown: String,
}

/// Sürüm notları alt bölümü (Örn: Eklenenler, Düzeltmeler).
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseSection {
    pub title: String,
    pub items: Vec<String>,
}

static VERSION_HEADER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^##\s+\[?v?([0-9]+\.[0-9]+(?:\.[0-9]+)?)\]?(?:\(([^\)]+)\))?\s*(?:\(([^)]+)\))?").unwrap()
});

// release-please'in madde sonuna eklediği bağlantılar: " ([#32](url))", " ([9aa22ea](url))", " ([#24](url), [#27](url))".
static REF_GROUP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\(\[[^\]]+\]\([^)]*\)(?:\s*,\s*\[[^\]]+\]\([^)]*\))*\)").unwrap());

static LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\([^)]*\)").unwrap());

// Conventional commit kapsamı: "**dock:** ..." (küçük harfli tek sözcük). Öne çıkanlardaki "**Atlama listesi:**" başlığı kapsam değildir.
static SCOPE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\*\*([a-z0-9-]+):\*\*\s*").unwrap());

/// Kapsamı uygulamaya ait olmayan maddeler "Neler Geldi"de gösterilmez (tanıtım sitesi, CI).
const HIDDEN_SCOPES: [&str; 3] = ["site", "ci", "deps"];

// C#'ta `EmbeddedResource`; burada derleme zamanında gömülür (yol: koy-rt/crates/koy-core/src/updates → koy/).
const EMBEDDED_CHANGELOG_EN: &str = include_str!("../../../../../koy/CHANGELOG.md");
const EMBEDDED_CHANGELOG_TR: &str = include_str!("../../../../../koy/CHANGELOG.tr.md");

/// CHANGELOG.md dosyasından sürüme özel notları, öne çıkanları ve kategorileri ayrıştıran saf mantık.
pub struct ChangelogParser;

impl ChangelogParser {
    /// Yayın akışının paket notunda İngilizce ve Türkçe bölümleri ayıran satır.
    pub const TURKISH_NOTES_MARKER: &'static str = "<!-- koy-notes:tr -->";

    /// Derlemeye gömülü değişiklik günlüğünü okur: Türkçe için CHANGELOG.tr.md, diğer dillerde CHANGELOG.md.
    /// C# varsayılanı: `language = AppLanguage.English`. (C#'taki `assembly` parametresi Rust'ta yok: kaynaklar bu crate'e gömülü.)
    pub fn read_embedded_changelog(language: &str) -> Option<String> {
        let text = if language == AppLanguage::TURKISH { EMBEDDED_CHANGELOG_TR } else { EMBEDDED_CHANGELOG_EN };
        // StreamReader(UTF8) baştaki BOM'u atar.
        Some(text.strip_prefix('\u{FEFF}').unwrap_or(text).to_string())
    }

    /// Belirtilen sürüme ait notları ayıklar. C# varsayılanı: `language = AppLanguage.English`.
    pub fn extract_version(markdown: Option<&str>, target_version: &str, language: &str) -> Option<ReleaseNotes> {
        let markdown = markdown?;
        if is_null_or_white_space(Some(markdown)) || is_null_or_white_space(Some(target_version)) {
            return None;
        }

        let clean_target = normalize_version(target_version);
        let entries = split_version_entries(markdown, language);

        entries.into_iter().find(|entry| eq_ignore_case(normalize_version(&entry.version), clean_target))
    }

    /// Belgedeki en son (en üstteki) sürüm notunu ayrıştırır. C# varsayılanı: `language = AppLanguage.English`.
    pub fn parse_latest(markdown: Option<&str>, language: &str) -> Option<ReleaseNotes> {
        let markdown = markdown?;
        if is_null_or_white_space(Some(markdown)) {
            return None;
        }

        split_version_entries(markdown, language).into_iter().next()
    }

    /// Sürümün notunu seçili dilde arar, o dilde yoksa öbür dile düşer; ekran hiç boş kalmaz.
    pub fn resolve_version(version: &str, language: &str, read_changelog: impl Fn(&str) -> Option<String>) -> Option<ReleaseNotes> {
        let preferred = if language == AppLanguage::TURKISH { AppLanguage::TURKISH } else { AppLanguage::ENGLISH };
        let other = if preferred == AppLanguage::TURKISH { AppLanguage::ENGLISH } else { AppLanguage::TURKISH };
        Self::extract_version(read_changelog(preferred).as_deref(), version, preferred)
            .or_else(|| Self::extract_version(read_changelog(other).as_deref(), version, other))
    }

    /// Gömülü günlüklerden [`ChangelogParser::resolve_version`].
    pub fn resolve_embedded(version: &str, language: &str) -> Option<ReleaseNotes> {
        Self::resolve_version(version, language, Self::read_embedded_changelog)
    }

    /// Güncelleme beslemesindeki paket notundan seçili dilin bölümünü ayırır. İşaret yoksa not İngilizcedir;
    /// Türkçe bölüm boşsa İngilizce kullanılır. Dönen dil, metnin gerçek dilidir.
    pub fn select_feed_notes(notes: Option<&str>, language: &str) -> (Option<String>, &'static str) {
        let notes = match notes {
            Some(n) if !is_null_or_white_space(Some(n)) => n,
            _ => return (notes.map(str::to_string), AppLanguage::ENGLISH),
        };

        let index = match notes.find(Self::TURKISH_NOTES_MARKER) {
            Some(i) => i,
            None => return (Some(notes.to_string()), AppLanguage::ENGLISH),
        };

        let english = notes[..index].trim();
        let turkish = notes[index + Self::TURKISH_NOTES_MARKER.len()..].trim();
        if language == AppLanguage::TURKISH && !turkish.is_empty() {
            (Some(turkish.to_string()), AppLanguage::TURKISH)
        } else {
            (Some(english.to_string()), AppLanguage::ENGLISH)
        }
    }

    /// Belgedeki tüm sürüm numaralarını listeler.
    pub fn get_all_versions(markdown: Option<&str>) -> Vec<String> {
        let markdown = match markdown {
            Some(m) if !is_null_or_white_space(Some(m)) => m,
            _ => return Vec::new(),
        };

        VERSION_HEADER_REGEX.captures_iter(markdown).filter_map(|m| m.get(1)).map(|g| g.as_str().to_string()).collect()
    }

    /// Sürüm notu maddesini düz metne çevirir: bağlantılar, commit ve issue referansları, kalın/kod işaretleri atılır;
    /// conventional commit kapsamı ("**dock:**") düşer ve ilk harf büyür. Uygulamayla ilgisiz kapsamda `None` döner.
    /// C# varsayılanı: `language = AppLanguage.English`.
    pub fn clean_item(item: &str, language: &str) -> Option<String> {
        if is_null_or_white_space(Some(item)) {
            return None;
        }

        let s = REF_GROUP_REGEX.replace_all(item, "");
        let s = LINK_REGEX.replace_all(&s, "${1}");

        let mut s: &str = &s;
        let mut dropped_scope = false;
        if let Some(scope) = SCOPE_REGEX.captures(s) {
            if HIDDEN_SCOPES.iter().any(|h| eq_ignore_case(h, &scope[1])) {
                return None;
            }
            s = &s[scope.get(0).unwrap().end()..];
            dropped_scope = true;
        }

        let s = s.replace("**", "").replace('`', "");
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let first = s.chars().next().unwrap();
        // C#: s[0] tek UTF-16 kod birimidir; yedek çift (astral) karakterin ilk yarısı küçük harf sayılmaz ve büyümez.
        if (dropped_scope || first.is_lowercase()) && first.len_utf16() == 1 {
            let upper = if language == AppLanguage::TURKISH {
                to_upper_tr(&first.to_string())
            } else {
                to_upper_invariant(&first.to_string())
            };
            return Some(format!("{upper}{}", &s[first.len_utf8()..]));
        }

        Some(s.to_string())
    }
}

fn split_version_entries(markdown: &str, language: &str) -> Vec<ReleaseNotes> {
    let mut result = Vec::new();
    let matches: Vec<regex::Captures> = VERSION_HEADER_REGEX.captures_iter(markdown).collect();

    for (i, m) in matches.iter().enumerate() {
        let version = m[1].to_string();
        let group2 = m.get(2).map(|g| g.as_str());
        let group3 = m.get(3).map(|g| g.as_str());

        let compare_url = group2.filter(|g| starts_with_ignore_case(g, "http")).map(str::to_string);

        let mut date: Option<String> = None;
        if let Some(g3) = group3 {
            date = Some(g3.to_string());
        } else if let Some(g2) = group2 {
            if !starts_with_ignore_case(g2, "http") {
                date = Some(g2.to_string());
            }
        }

        let start_index = m.get(0).unwrap().start();
        let end_index = if i + 1 < matches.len() { matches[i + 1].get(0).unwrap().start() } else { markdown.len() };
        let section_raw = markdown[start_index..end_index].trim();

        // Başlık satırını atlayıp gövdeyi al
        let body = match section_raw.find('\n') {
            Some(body_index) => section_raw[body_index + 1..].trim(),
            None => "",
        };

        result.push(parse_entry_body(version, compare_url, date, body, section_raw, language));
    }

    result
}

fn parse_entry_body(version: String, compare_url: Option<String>, date: Option<String>, body: &str, raw: &str, language: &str) -> ReleaseNotes {
    let mut highlights: Vec<String> = Vec::new();
    let mut sections: Vec<ReleaseSection> = Vec::new();

    let mut current_section_title: Option<String> = None;
    let mut current_section_items: Vec<String> = Vec::new();
    let mut in_highlights = false;
    let mut summary_builder = String::new();

    for raw_line in body.split(['\r', '\n']).filter(|l| !l.is_empty()) {
        let line = raw_line.trim();
        if line.starts_with("### ") {
            if let Some(title) = current_section_title.take() {
                sections.push(ReleaseSection { title, items: std::mem::take(&mut current_section_items) });
            }
            current_section_title = Some(line[4..].trim().to_string());
            in_highlights = false;
            continue;
        }

        if starts_with_ignore_case(line, "Öne çıkanlar:") || starts_with_ignore_case(line, "Highlights:") {
            in_highlights = true;
            continue;
        }

        if line.starts_with("- ") || line.starts_with("* ") {
            let bullet = match ChangelogParser::clean_item(line[2..].trim(), language) {
                Some(b) => b,
                None => continue,
            };
            if in_highlights {
                highlights.push(bullet);
            } else if current_section_title.is_some() {
                current_section_items.push(bullet);
            } else {
                // Başlık yokken gelen maddeler de öne çıkan sayılır
                highlights.push(bullet);
            }
        } else if !in_highlights && current_section_title.is_none() && !line.starts_with('>') {
            if !summary_builder.is_empty() {
                summary_builder.push(' ');
            }
            summary_builder.push_str(line);
        }
    }

    if let Some(title) = current_section_title {
        if !current_section_items.is_empty() {
            sections.push(ReleaseSection { title, items: current_section_items });
        }
    }

    let summary = if summary_builder.is_empty() { None } else { Some(summary_builder) };

    ReleaseNotes { version, compare_url, date, summary, highlights, sections, raw_markdown: raw.to_string() }
}

fn normalize_version(v: &str) -> &str {
    let mut s = v.trim().trim_start_matches(['v', 'V']);
    if let Some(plus) = s.find('+') {
        s = &s[..plus];
    }
    if let Some(dash) = s.find('-') {
        s = &s[..dash];
    }
    s
}

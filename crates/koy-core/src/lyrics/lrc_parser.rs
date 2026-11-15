//! Karşılığı: `Lyrics/LrcParser.cs`.

use std::sync::OnceLock;

use regex::{Captures, Regex};

use super::lyric_line::LyricLine;
use crate::text;
use crate::time::TimeSpan;

// Bozuk ya da kötü niyetli bir dosya belleği şişirmesin; gerçek şarkılar birkaç yüz satırdır.
const MAX_LINES: usize = 5000;

// C#'ta `\G` ile konumdan başlayan eşleşme; burada kalan dilime `^` ile uygulanır (aynı anlam).
// `\d` yerine `[0-9]`: .NET'te `\d` Unicode rakamlarını da eşler ve `int.Parse` bunlarda FormatException fırlatıp tüm
// ayrıştırmayı düşürürdü; burada böyle bir damga yalnız eşleşmez ve satır atlanır (tek bilinçli sapma).
fn time_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\[([0-9]{1,3}):([0-9]{1,2})(?:[.:]([0-9]{1,3}))?\]").unwrap())
}

fn word_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"<\d{1,3}:\d{1,2}(?:[.:]\d{1,3})?>").unwrap())
}

fn offset_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)^\[offset:\s*([+-]?[0-9]{1,7})\s*\]").unwrap())
}

/// LRC metnini zamana göre sıralı satırlara çevirir. Hem `[mm:ss.xx]` hem `[mm:ss:xx]` yazımını kabul eder,
/// kelime bazlı `<mm:ss.xx>` etiketlerini metinden siler. Damgasız satırlar (meta etiketleri) elenir.
pub struct LrcParser;

impl LrcParser {
    pub fn parse(lrc: Option<&str>) -> Vec<LyricLine> {
        let lrc = match lrc {
            Some(l) if !text::is_null_or_white_space(Some(l)) => l,
            _ => return Vec::new(),
        };

        let mut offset = TimeSpan::ZERO;
        let mut lines: Vec<LyricLine> = Vec::new();

        for raw_line in lrc.trim_start_matches('\u{FEFF}').split('\n') {
            let line = text::trim(raw_line);
            if line.is_empty() {
                continue;
            }

            if let Some(offset_match) = offset_tag().captures(line) {
                // LRC standardında pozitif offset sözü erkene alır.
                let millis: i32 = offset_match[1].parse().expect("en çok 7 haneli işaretli sayı int'e sığar");
                offset = TimeSpan::from_millis(millis as i64);
                continue;
            }

            let mut position = 0;
            let mut stamps: Vec<TimeSpan> = Vec::with_capacity(1);
            while let Some(m) = time_tag().captures(&line[position..]) {
                stamps.push(read_stamp(&m));
                position += m[0].len();
            }

            if stamps.is_empty() {
                continue;
            }

            let body = text::trim(&word_tag().replace_all(&line[position..], "")).to_string();
            for stamp in stamps {
                if lines.len() >= MAX_LINES {
                    break;
                }
                lines.push(LyricLine::new(stamp, body.clone()));
            }
        }

        if lines.iter().all(|l| l.text.is_empty()) {
            return Vec::new();
        }

        normalize(lines, offset)
    }
}

fn read_stamp(m: &Captures) -> TimeSpan {
    let minutes: i64 = m[1].parse().expect("en çok 3 hane");
    let seconds: i64 = m[2].parse().expect("en çok 2 hane");

    let mut milliseconds: i64 = 0;
    if let Some(fraction) = m.get(3) {
        // ".5" 500 ms, ".50" 500 ms, ".500" 500 ms: basamak sayısı ölçeği belirler.
        let value: i64 = fraction.as_str().parse().expect("en çok 3 hane");
        milliseconds = value
            * match fraction.as_str().len() {
                1 => 100,
                2 => 10,
                _ => 1,
            };
    }

    TimeSpan::new_full(0, 0, minutes, seconds, milliseconds)
}

fn normalize(mut lines: Vec<LyricLine>, offset: TimeSpan) -> Vec<LyricLine> {
    let mut result: Vec<LyricLine> = Vec::with_capacity(lines.len());
    // OrderBy kararlıdır: aynı damgalı satırlar dosyadaki sırayı korur.
    lines.sort_by_key(|l| l.time);
    for line in lines {
        // Art arda boş satırlar ekranda anlamsız "♪" dizisi olurdu.
        if line.text.is_empty() && result.last().is_some_and(|last| last.text.is_empty()) {
            continue;
        }

        let time = line.time - offset;
        result.push(LyricLine { time: if time < TimeSpan::ZERO { TimeSpan::ZERO } else { time }, ..line });
    }

    result
}

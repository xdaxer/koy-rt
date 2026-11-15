//! Karşılığı: `Lyrics/LyricsTimeline.cs`.

use super::lyric_line::LyricLine;
use crate::time::TimeSpan;

/// Çalma konumundan etkin satırı ve bir sonraki satır değişimine kalan süreyi hesaplar.
pub struct LyricsTimeline;

impl LyricsTimeline {
    /// Satır, sesten biraz önce vurgulanır: göz, geç gelen vurguyu erken gelenden daha çok fark eder.
    pub const LEAD: TimeSpan = TimeSpan::from_millis(150);

    /// Konumda etkin olan son satırın sırası; ilk satırdan önceyse -1.
    pub fn index_at(lines: &[LyricLine], position: TimeSpan) -> i32 {
        let target = position + Self::LEAD;
        let mut low: i32 = 0;
        let mut high: i32 = lines.len() as i32 - 1;
        let mut found: i32 = -1;

        while low <= high {
            let middle = low + (high - low) / 2;
            if lines[middle as usize].time <= target {
                found = middle;
                low = middle + 1;
            } else {
                high = middle - 1;
            }
        }

        found
    }

    /// Bir sonraki satıra kalan süre; son satırdaysa `None`. Zamanlayıcı yalnız satır değişiminde uyanır.
    pub fn until_next_change(lines: &[LyricLine], position: TimeSpan) -> Option<TimeSpan> {
        let next = Self::index_at(lines, position) + 1;
        if next >= lines.len() as i32 {
            return None;
        }

        let delay = lines[next as usize].time - Self::LEAD - position;
        Some(if delay > TimeSpan::ZERO { delay } else { TimeSpan::ZERO })
    }
}

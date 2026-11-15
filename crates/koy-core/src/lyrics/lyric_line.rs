//! Karşılığı: `Lyrics/LyricLine.cs`.

use crate::time::TimeSpan;

/// Zaman damgalı tek söz satırı. Metin boşsa enstrümantal aradır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    pub time: TimeSpan,
    pub text: String,
}

impl LyricLine {
    pub fn new(time: TimeSpan, text: impl Into<String>) -> Self {
        LyricLine { time, text: text.into() }
    }
}

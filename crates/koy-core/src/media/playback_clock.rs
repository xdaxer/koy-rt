use chrono::{FixedOffset, TimeZone};

use super::PlaybackTimeline;
use crate::time::{span_between, DateTimeOffset, TimeSpan};

/// Oynatıcının seyrek bildirdiği zaman çizelgesinden şu anki konumu hesaplar.
pub struct PlaybackClock;

// Bu kadar eski bir damga güvenilmez: oynatıcı bildirimi kesmiştir, süreyi eklemek konumu sona fırlatırdı.
const MAX_EXTRAPOLATION: TimeSpan = TimeSpan::from_hours(1);

/// `DateTimeOffset.MinValue`: 0001-01-01T00:00:00+00:00.
pub fn date_time_offset_min_value() -> DateTimeOffset {
    FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(1, 1, 1, 0, 0, 0).unwrap()
}

impl PlaybackClock {
    pub fn duration(timeline: &PlaybackTimeline) -> TimeSpan {
        let duration = timeline.end_time - timeline.start_time;
        if duration < TimeSpan::ZERO { TimeSpan::ZERO } else { duration }
    }

    pub fn position_at(timeline: &PlaybackTimeline, is_playing: bool, now: DateTimeOffset) -> TimeSpan {
        let mut position = timeline.position - timeline.start_time;

        if is_playing && timeline.last_updated != date_time_offset_min_value() {
            let elapsed = span_between(now, timeline.last_updated);
            if elapsed > TimeSpan::ZERO && elapsed < MAX_EXTRAPOLATION {
                position += elapsed;
            }
        }

        let duration = Self::duration(timeline);
        if position < TimeSpan::ZERO {
            return TimeSpan::ZERO;
        }

        if position > duration { duration } else { position }
    }
}

use chrono::{NaiveDateTime, Timelike};

use crate::time::TimeSpan;

/// Çentik saati yalnız dakika değişince yenilenir: zamanlayıcı her tıkta bir sonraki dakika başına yeniden kurulur.
/// Saniyede ya da birkaç saniyede bir uyanmaktan ucuz, saat de dakika döner dönmez değişir. Saniye gösterilirse
/// bu hesap yetmez.
pub struct ClockTick;

impl ClockTick {
    /// Dakika döndükten sonra uyanma payı; zamanlayıcı birkaç ms erken düşse de yeni dakika okunur.
    pub const MARGIN: TimeSpan = TimeSpan::from_millis(30);

    /// En kısa bekleme; dakikanın son anında çok kısa bir zamanlayıcı kurulmasın.
    pub const MINIMUM_DELAY: TimeSpan = TimeSpan::from_millis(100);

    /// `now` anından bir sonraki dakika başına (artı pay) kalan süre.
    pub fn delay_to_next_minute(now: NaiveDateTime) -> TimeSpan {
        // `now.Ticks % TicksPerMinute`: gün başlangıcı dakika sınırına denk gelir, yalnız saniye ve altı kalır.
        let into_minute = TimeSpan::from_ticks(now.second() as i64 * TimeSpan::TICKS_PER_SECOND + (now.nanosecond() / 100) as i64);
        let delay = TimeSpan::from_mins(1) - into_minute + Self::MARGIN;
        if delay < Self::MINIMUM_DELAY { Self::MINIMUM_DELAY } else { delay }
    }
}

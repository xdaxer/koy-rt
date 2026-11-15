use crate::time::TimeSpan;

/// Zamanlayıcı ve kronometre metinleri. Kültürden bağımsız (iki nokta ve nokta sabit).
pub struct DurationFormat;

impl DurationFormat {
    /// "05:00", bir saati geçince "1:05:00". Saniyenin kesri yukarı yuvarlanır: 0,4 sn kalmışsa "00:01".
    pub fn countdown(value: TimeSpan) -> String {
        if value <= TimeSpan::ZERO {
            return "00:00".to_string();
        }

        let seconds = value.total_seconds().ceil() as i64;
        Self::clock(seconds / 3600, seconds / 60 % 60, seconds % 60)
    }

    /// "00:12.3" (dakika:saniye.onda bir); bir saati geçince "1:00:12.3". Kesir aşağı yuvarlanır.
    pub fn stopwatch(value: TimeSpan) -> String {
        let value = if value < TimeSpan::ZERO { TimeSpan::ZERO } else { value };

        let tenths = (value.total_milliseconds() / 100.0) as i64;
        let seconds = tenths / 10;
        format!("{}.{}", Self::clock(seconds / 3600, seconds / 60 % 60, seconds % 60), tenths % 10)
    }

    fn clock(hours: i64, minutes: i64, seconds: i64) -> String {
        if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{minutes:02}:{seconds:02}") }
    }
}

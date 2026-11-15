use chrono::{NaiveDate, NaiveDateTime};

use crate::time::TimeSpan;

/// `DateTime.MinValue` (0001-01-01 00:00:00).
pub fn date_time_min_value() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(1, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayHostAction {
    /// Bir şey yapma.
    None,

    /// Tepsi penceresini kur ve düğmeyi geri getir.
    Acquire,

    /// Tepsi penceresini bırak (koşul bozuldu).
    Release,

    /// Elimizdeki tepsi penceresi ölmüş: bırak, sonra yeniden kur.
    Reacquire,
}

/// Dock'ın tepsi penceresiyle ilgili tek karar yeri (saf mantık). Önceki sürümde ilk kurulum hatası kalıcı
/// bayrağa dönüşüyor ve düğme ekran değişimine kadar geri gelmiyordu; artık başarısız kurulum zamanla yeniden
/// denenir (üstel bekleme, üst sınırlı). Yoklama zamanlayıcısı gerekmez: olaylar (pencere değişimi, tepsi
/// penceresinin ölüm bildirimi) bu sınıfa soru sorar. Zamanlar UTC'dir (C#'ta `DateTime` + `Kind=Utc`).
#[derive(Debug)]
pub struct TrayHostRecovery {
    failures: i32,
    retry_at_utc: NaiveDateTime,
}

impl Default for TrayHostRecovery {
    fn default() -> Self {
        TrayHostRecovery { failures: 0, retry_at_utc: date_time_min_value() }
    }
}

impl TrayHostRecovery {
    pub const FIRST_RETRY_DELAY: TimeSpan = TimeSpan::from_secs(2);
    pub const MAX_RETRY_DELAY: TimeSpan = TimeSpan::from_secs(30);

    pub fn new() -> Self {
        Self::default()
    }

    /// Ardışık başarısız kurulum sayısı.
    pub fn failures(&self) -> i32 {
        self.failures
    }

    /// Şu an ne yapılmalı.
    ///
    /// `should_host`: `TrayHostPolicy::should_host` sonucu (elle bırakma bayrağı dahil). `has_host`: dock'un elinde bir
    /// tepsi servisi var. `host_running`: eldeki servis çalışıyor (pencere ayakta).
    pub fn decide(&mut self, should_host: bool, has_host: bool, host_running: bool, now_utc: NaiveDateTime) -> TrayHostAction {
        if !should_host {
            self.reset_failures();
            return if has_host { TrayHostAction::Release } else { TrayHostAction::None };
        }

        if has_host && host_running {
            return TrayHostAction::None;
        }

        // Bekleme sürerken ölü tutamaç yine de bırakılır: düğme ölü pencereye bağlı görünmesin.
        if now_utc < self.retry_at_utc {
            return if has_host { TrayHostAction::Release } else { TrayHostAction::None };
        }

        if has_host { TrayHostAction::Reacquire } else { TrayHostAction::Acquire }
    }

    /// Kurulum sonucu bildirilir: başarıda sayaç sıfırlanır, hatada bir sonraki deneme ertelenir.
    pub fn report_acquire(&mut self, success: bool, now_utc: NaiveDateTime) {
        if success {
            self.reset_failures();
            return;
        }

        self.failures += 1;
        let seconds = Self::MAX_RETRY_DELAY
            .total_seconds()
            .min(Self::FIRST_RETRY_DELAY.total_seconds() * 2f64.powi((self.failures - 1).min(10)));
        self.retry_at_utc = now_utc + TimeSpan::from_seconds_f64(seconds).to_chrono();
    }

    /// Beklemeyi bırakır (ekran değişti, ayar değişti): bir sonraki soruda hemen denenir.
    pub fn reset_failures(&mut self) {
        self.failures = 0;
        self.retry_at_utc = date_time_min_value();
    }
}

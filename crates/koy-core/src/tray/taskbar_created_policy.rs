use chrono::NaiveDateTime;

use crate::time::TimeSpan;

/// "TaskbarCreated" yayını Gezgin yeniden başlayınca gelir. Koy'un tepsi penceresi de simgeleri yeniden eklettirmek için
/// aynı mesajı kendisi yayınlar (Gezgin'in tepsisi öne geçtikçe, en sık 5 sn'de bir) ve yayın Koy'un kendi pencerelerine
/// de ulaşır. Kendi yayınımız "Gezgin yeniden başladı" sanılırsa dock görev çubuğunu yeniden gizler ve AppBar kaydını
/// silip yeniden yapar; çalışma alanı her seferinde oynar, büyütülmüş pencereler 5 sn'de bir yeniden boyutlanırdı.
/// Ayrım yalnız zamana değil kimliğe de bakar: gerçek yeniden başlamada Gezgin'in Shell_TrayWnd tutamacı değişir.
pub struct TaskbarCreatedPolicy;

impl TaskbarCreatedPolicy {
    /// Kendi yayınımızın bize ulaşması için tanınan süre.
    pub const OWN_BROADCAST_WINDOW: TimeSpan = TimeSpan::from_secs(3);

    /// Gelen yayın bizim son yayınımızın yankısı mı. Yankı: süre içinde VE Gezgin'in tepsi tutamacı yayın anındakiyle
    /// aynı. Tutamaç değiştiyse (ya da bilinmiyorsa / Gezgin'in tepsisi şu an yoksa) yayın kendi yayınımızla çakışsa bile
    /// gerçek bir yeniden başlamadır.
    ///
    /// Zamanlar UTC'dir (C#'ta `DateTime` + `DateTimeKind.Utc`). `explorer_tray_at_broadcast`: yayın anında Gezgin'in
    /// Shell_TrayWnd tutamacı (0 = bilinmiyor); `explorer_tray_now`: şu an (0 = yok).
    pub fn is_own_broadcast(
        last_own_broadcast_utc: NaiveDateTime,
        now_utc: NaiveDateTime,
        explorer_tray_at_broadcast: isize,
        explorer_tray_now: isize,
    ) -> bool {
        TimeSpan::from_chrono(now_utc - last_own_broadcast_utc) <= Self::OWN_BROADCAST_WINDOW
            && explorer_tray_at_broadcast != 0
            && explorer_tray_now == explorer_tray_at_broadcast
    }
}

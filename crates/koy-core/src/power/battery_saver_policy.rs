use super::i_power_service::BatteryStatus;

/// Pil tasarrufu kararları (saf mantık, test edilebilir).
pub struct BatterySaverPolicy;

impl BatterySaverPolicy {
    /// Animasyonlar yalnız şu üç koşul birlikteyse kapatılır: kullanıcı "pil tasarrufunda animasyonları azalt"ı açmış,
    /// cihazda pil var ve şu an şarj olmadan pille çalışıyor. Prizde (şarj olan ya da dolu) tam animasyon sürer.
    pub fn should_reduce_motion(setting_enabled: bool, has_battery: bool, status: BatteryStatus) -> bool {
        setting_enabled && has_battery && status == BatteryStatus::Discharging
    }
}

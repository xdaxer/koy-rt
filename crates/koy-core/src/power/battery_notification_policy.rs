use super::i_power_service::{BatteryStatus, PowerChangedEventArgs};

/// Pil bildirim türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BatteryAlertKind {
    #[default]
    None,
    PluggedIn,
    Unplugged,
    LowBattery,
    FullyCharged,
}

/// Pil bildirim sonucu: bildirim türü ve o anki pil yüzdesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatteryAlertResult {
    pub kind: BatteryAlertKind,
    pub percent: i32,
}

impl BatteryAlertResult {
    pub const fn new(kind: BatteryAlertKind, percent: i32) -> Self {
        BatteryAlertResult { kind, percent }
    }
}

/// Pil durumu geçişlerini inceleyip OSD bildirimi üretilip üretilmeyeceğini belirleyen saf mantık.
/// Açılışta tohumlama (seeding) bastırılır; histerezis ile düşük pil uyarısı döngü başına bir kez verilir.
#[derive(Debug, Default)]
pub struct BatteryNotificationPolicy {
    is_initialized: bool,
    // C# ile aynı: yazılıyor ama hiç okunmuyor (kayıt için tutulur).
    #[allow(dead_code)]
    last_has_battery: bool,
    last_is_plugged: bool,
    last_percent: i32,

    warned_low20: bool,
    warned_low10: bool,
}

impl BatteryNotificationPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Güç durumu değişikliğini işleyip bildirim sonucunu döner.
    pub fn update(&mut self, current: PowerChangedEventArgs) -> BatteryAlertResult {
        if !current.has_battery {
            self.warned_low20 = false;
            self.warned_low10 = false;
            self.last_has_battery = false;
            self.last_is_plugged = false;
            self.is_initialized = true;
            return BatteryAlertResult::new(BatteryAlertKind::None, current.remaining_percent);
        }

        let is_plugged = Self::is_plugged(&current);

        if !self.is_initialized {
            self.is_initialized = true;
            self.last_has_battery = current.has_battery;
            self.last_is_plugged = is_plugged;
            self.last_percent = current.remaining_percent;

            // Açılışta düşük pildeyse başlangıç uyarısını bastır, ancak daha alt eşik için izin ver.
            if !is_plugged {
                if current.remaining_percent <= 20 {
                    self.warned_low20 = true;
                }
                if current.remaining_percent <= 10 {
                    self.warned_low10 = true;
                }
            }

            return BatteryAlertResult::new(BatteryAlertKind::None, current.remaining_percent);
        }

        // Histerezis sıfırlama: şarja takıldığında veya pil %25 ve üstüne çıktığında
        if is_plugged || current.remaining_percent >= 25 {
            self.warned_low20 = false;
            self.warned_low10 = false;
        }

        let mut kind = BatteryAlertKind::None;

        // 1. Şarja takılma geçişi
        if !self.last_is_plugged && is_plugged {
            kind = BatteryAlertKind::PluggedIn;
            self.warned_low20 = false;
            self.warned_low10 = false;
        }
        // 2. Şarjdan çıkarılma geçişi
        else if self.last_is_plugged && !is_plugged {
            kind = BatteryAlertKind::Unplugged;
        }
        // 3. Şarj tamamlandı (%100'e ulaşıldı)
        else if is_plugged && current.remaining_percent == 100 && self.last_percent < 100 {
            kind = BatteryAlertKind::FullyCharged;
        }
        // 4. Düşük pil eşikleri (yalnız pildeyken)
        else if !is_plugged {
            if current.remaining_percent <= 10 && !self.warned_low10 {
                kind = BatteryAlertKind::LowBattery;
                self.warned_low10 = true;
                self.warned_low20 = true;
            } else if current.remaining_percent <= 20 && !self.warned_low20 {
                kind = BatteryAlertKind::LowBattery;
                self.warned_low20 = true;
            }
        }

        self.last_has_battery = current.has_battery;
        self.last_is_plugged = is_plugged;
        self.last_percent = current.remaining_percent;

        BatteryAlertResult::new(kind, current.remaining_percent)
    }

    /// `update` ile aynı işlevi görür; alternatif adlandırma için.
    pub fn process_change(&mut self, current: PowerChangedEventArgs) -> BatteryAlertResult {
        self.update(current)
    }

    fn is_plugged(current: &PowerChangedEventArgs) -> bool {
        current.is_charging || matches!(current.status, BatteryStatus::Charging | BatteryStatus::Idle)
    }
}

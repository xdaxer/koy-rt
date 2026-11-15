use crate::event::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BatteryStatus {
    #[default]
    NotPresent,
    Discharging,
    Charging,
    Idle,
}

/// Pil ve güç yönetimi sözleşmesi. (`IDisposable` → uygulayıcının `Drop`'u.)
pub trait IPowerService: Send + Sync {
    fn has_battery(&self) -> bool;
    fn remaining_percent(&self) -> i32;
    fn is_charging(&self) -> bool;
    fn status(&self) -> BatteryStatus;

    fn power_changed(&self) -> &Event<PowerChangedEventArgs>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PowerChangedEventArgs {
    pub has_battery: bool,
    pub remaining_percent: i32,
    pub is_charging: bool,
    pub status: BatteryStatus,
}

impl PowerChangedEventArgs {
    pub const fn new(has_battery: bool, remaining_percent: i32, is_charging: bool, status: BatteryStatus) -> Self {
        PowerChangedEventArgs { has_battery, remaining_percent, is_charging, status }
    }
}

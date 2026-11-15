//! Karşılığı: `Platform/Power/PowerService.cs`.

use std::sync::{Arc, Mutex, Weak};

use koy_core::event::Event;
use koy_core::power::{BatteryStatus, IPowerService, PowerChangedEventArgs};
use windows::Foundation::EventHandler;
use windows::System::Power::{BatteryStatus as WinBatteryStatus, PowerManager, PowerSupplyStatus as WinPowerSupply};
use windows::core::IInspectable;

#[derive(Clone, Copy)]
struct State {
    has_battery: bool,
    remaining_percent: i32,
    is_charging: bool,
    status: BatteryStatus,
}

struct Inner {
    state: Mutex<State>,
    power_changed: Event<PowerChangedEventArgs>,
}

/// Pil durumu (WinRT `PowerManager`); olay tabanlı, yoklama yok.
pub struct PowerService {
    inner: Arc<Inner>,
    tokens: Mutex<Option<[i64; 3]>>,
}

impl PowerService {
    pub fn new() -> Self {
        let inner = Arc::new(Inner {
            state: Mutex::new(State {
                has_battery: false,
                remaining_percent: 100,
                is_charging: false,
                status: BatteryStatus::NotPresent,
            }),
            power_changed: Event::new(),
        });
        inner.refresh();

        let make = |weak: Weak<Inner>| {
            EventHandler::<IInspectable>::new(move |_, _| {
                if let Some(inner) = weak.upgrade() {
                    inner.refresh();
                }
                Ok(())
            })
        };
        let w = Arc::downgrade(&inner);
        let t1 = PowerManager::BatteryStatusChanged(&make(w.clone()));
        let t2 = PowerManager::RemainingChargePercentChanged(&make(w.clone()));
        let t3 = PowerManager::PowerSupplyStatusChanged(&make(w));
        let tokens = Some([t1.unwrap_or(0), t2.unwrap_or(0), t3.unwrap_or(0)]);

        PowerService { inner, tokens: Mutex::new(tokens) }
    }
}

impl Default for PowerService {
    fn default() -> Self {
        Self::new()
    }
}

impl Inner {
    fn refresh(&self) {
        let args = {
            let mut s = self.state.lock().unwrap();
            match read_into(&mut s) {
                Ok(()) => Some(PowerChangedEventArgs::new(s.has_battery, s.remaining_percent, s.is_charging, s.status)),
                Err(_) => {
                    s.has_battery = false;
                    s.status = BatteryStatus::NotPresent;
                    None
                }
            }
        };

        // C# ile aynı: olay yalnız okuma başarılıysa yayınlanır (kilit dışında).
        if let Some(args) = args {
            self.power_changed.invoke(&args);
        }
    }
}

/// C#'taki gibi sırayla atar: ortada hata çıkarsa o ana kadar atananlar kalır.
fn read_into(s: &mut State) -> windows::core::Result<()> {
    s.status = map_battery_status(PowerManager::BatteryStatus()?);
    s.has_battery = s.status != BatteryStatus::NotPresent;
    s.remaining_percent = PowerManager::RemainingChargePercent()?;
    s.is_charging = s.status == BatteryStatus::Charging
        || (s.has_battery && PowerManager::PowerSupplyStatus()? == WinPowerSupply::Adequate && s.remaining_percent < 100);
    Ok(())
}

fn map_battery_status(status: WinBatteryStatus) -> BatteryStatus {
    match status {
        WinBatteryStatus::Charging => BatteryStatus::Charging,
        WinBatteryStatus::Discharging => BatteryStatus::Discharging,
        WinBatteryStatus::Idle => BatteryStatus::Idle,
        _ => BatteryStatus::NotPresent,
    }
}

impl IPowerService for PowerService {
    fn has_battery(&self) -> bool {
        self.inner.state.lock().unwrap().has_battery
    }

    fn remaining_percent(&self) -> i32 {
        self.inner.state.lock().unwrap().remaining_percent
    }

    fn is_charging(&self) -> bool {
        self.inner.state.lock().unwrap().is_charging
    }

    fn status(&self) -> BatteryStatus {
        self.inner.state.lock().unwrap().status
    }

    fn power_changed(&self) -> &Event<PowerChangedEventArgs> {
        &self.inner.power_changed
    }
}

impl Drop for PowerService {
    fn drop(&mut self) {
        if let Some([a, b, c]) = self.tokens.lock().unwrap().take() {
            let _ = PowerManager::RemoveBatteryStatusChanged(a);
            let _ = PowerManager::RemoveRemainingChargePercentChanged(b);
            let _ = PowerManager::RemovePowerSupplyStatusChanged(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_status_is_mapped() {
        assert_eq!(map_battery_status(WinBatteryStatus::Charging), BatteryStatus::Charging);
        assert_eq!(map_battery_status(WinBatteryStatus::Discharging), BatteryStatus::Discharging);
        assert_eq!(map_battery_status(WinBatteryStatus::Idle), BatteryStatus::Idle);
        assert_eq!(map_battery_status(WinBatteryStatus::NotPresent), BatteryStatus::NotPresent);
    }
}

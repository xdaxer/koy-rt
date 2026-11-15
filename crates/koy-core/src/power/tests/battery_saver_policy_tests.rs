use crate::power::{BatterySaverPolicy, BatteryStatus};

#[test]
fn reduces_motion_only_while_discharging_with_setting_on() {
    assert!(BatterySaverPolicy::should_reduce_motion(true, true, BatteryStatus::Discharging));
}

#[test]
fn keeps_full_motion_on_ac_power_or_without_battery() {
    for status in [BatteryStatus::Charging, BatteryStatus::Idle, BatteryStatus::NotPresent] {
        assert!(!BatterySaverPolicy::should_reduce_motion(true, status != BatteryStatus::NotPresent, status), "{status:?}");
    }
}

#[test]
fn keeps_full_motion_when_setting_is_off() {
    assert!(!BatterySaverPolicy::should_reduce_motion(false, true, BatteryStatus::Discharging));
}

#[test]
fn desktop_without_battery_never_reduces_motion() {
    assert!(!BatterySaverPolicy::should_reduce_motion(true, false, BatteryStatus::Discharging));
}

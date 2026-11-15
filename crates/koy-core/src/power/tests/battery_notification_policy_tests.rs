use crate::power::*;

fn create_args(has_battery: bool, percent: i32, is_charging: bool, status: BatteryStatus) -> PowerChangedEventArgs {
    PowerChangedEventArgs::new(has_battery, percent, is_charging, status)
}

/// C# `CreateArgs(percent:, isCharging:, status:)` (`hasBattery` varsayılanı true).
fn args(percent: i32, is_charging: bool, status: BatteryStatus) -> PowerChangedEventArgs {
    create_args(true, percent, is_charging, status)
}

const DISCHARGING: BatteryStatus = BatteryStatus::Discharging;
const CHARGING: BatteryStatus = BatteryStatus::Charging;

#[test]
fn initial_call_seeds_state_does_not_emit_alert() {
    let mut policy = BatteryNotificationPolicy::new();

    let result = policy.update(create_args(true, 75, false, DISCHARGING));

    assert_eq!(BatteryAlertKind::None, result.kind);
    assert_eq!(75, result.percent);
}

#[test]
fn desktop_no_battery_always_returns_none() {
    let mut policy = BatteryNotificationPolicy::new();

    let initial = policy.update(create_args(false, 100, false, BatteryStatus::NotPresent));
    let next = policy.update(create_args(false, 100, true, BatteryStatus::Idle));

    assert_eq!(BatteryAlertKind::None, initial.kind);
    assert_eq!(BatteryAlertKind::None, next.kind);
}

#[test]
fn plugged_in_transition_emits_plugged_in() {
    let mut policy = BatteryNotificationPolicy::new();
    policy.update(args(50, false, DISCHARGING));

    let result = policy.update(args(50, true, CHARGING));

    assert_eq!(BatteryAlertKind::PluggedIn, result.kind);
    assert_eq!(50, result.percent);
}

#[test]
fn unplugged_transition_emits_unplugged() {
    let mut policy = BatteryNotificationPolicy::new();
    policy.update(args(60, true, CHARGING));

    let result = policy.update(args(60, false, DISCHARGING));

    assert_eq!(BatteryAlertKind::Unplugged, result.kind);
    assert_eq!(60, result.percent);
}

#[test]
fn fully_charged_transition_emits_fully_charged_only_once() {
    let mut policy = BatteryNotificationPolicy::new();
    policy.update(args(98, true, CHARGING));

    let step1 = policy.update(args(99, true, CHARGING));
    assert_eq!(BatteryAlertKind::None, step1.kind);

    let full_result = policy.update(args(100, false, BatteryStatus::Idle));
    assert_eq!(BatteryAlertKind::FullyCharged, full_result.kind);
    assert_eq!(100, full_result.percent);

    // Bir sonraki olayda hâlâ %100 ve şarjda iken tekrar uyarı vermez
    let next_result = policy.update(args(100, false, BatteryStatus::Idle));
    assert_eq!(BatteryAlertKind::None, next_result.kind);
}

#[test]
fn low_battery_20_percent_fires_once_and_hysteresis_prevents_fluctuation_bouncing() {
    let mut policy = BatteryNotificationPolicy::new();
    policy.update(args(25, false, DISCHARGING));

    // 22%'ye düşer - henüz uyarı yok
    let res22 = policy.update(args(22, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, res22.kind);

    // 20%'ye düşer - LowBattery tetiklenmeli
    let res20 = policy.update(args(20, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::LowBattery, res20.kind);
    assert_eq!(20, res20.percent);

    // 19%'a düşer - aynı döngüde tekrar tetiklenmemeli
    let res19 = policy.update(args(19, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, res19.kind);

    // Dalgalanma: 21%'e çıkıp tekrar 20%'ye inerse (%25'e ulaşmadığı için) tekrar tetiklenmemeli
    let res21 = policy.update(args(21, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, res21.kind);

    let res20_again = policy.update(args(20, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, res20_again.kind);
}

#[test]
fn low_battery_10_percent_fires_once_after_20_percent() {
    let mut policy = BatteryNotificationPolicy::new();
    policy.update(args(25, false, DISCHARGING));
    policy.update(args(20, false, DISCHARGING));

    // 15% - 20 uyarısı verilmişti, 10 henüz değil
    let res15 = policy.update(args(15, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, res15.kind);

    // 10% - LowBattery tetiklenmeli
    let res10 = policy.update(args(10, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::LowBattery, res10.kind);
    assert_eq!(10, res10.percent);

    // 9% - tekrar tetiklenmemeli
    let res9 = policy.update(args(9, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, res9.kind);
}

#[test]
fn low_battery_direct_drop_to_under_10_percent_fires_once() {
    let mut policy = BatteryNotificationPolicy::new();
    policy.update(args(25, false, DISCHARGING));

    // Ani düşüş: 25'ten 8%'e
    let res8 = policy.update(args(8, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::LowBattery, res8.kind);
    assert_eq!(8, res8.percent);

    // 7%'ye devam edince tekrar tetiklenmemeli
    let res7 = policy.update(args(7, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, res7.kind);
}

#[test]
fn hysteresis_resets_when_plugged_in() {
    let mut policy = BatteryNotificationPolicy::new();
    policy.update(args(25, false, DISCHARGING));

    // 20%'de uyarı verir
    let res20 = policy.update(args(20, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::LowBattery, res20.kind);

    // Şarja takılır
    let res_plug = policy.update(args(20, true, CHARGING));
    assert_eq!(BatteryAlertKind::PluggedIn, res_plug.kind);

    // Şarjdan çıkarılır
    let res_unplug = policy.update(args(21, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::Unplugged, res_unplug.kind);

    // Tekrar 20%'ye düşünce histerezis sıfırlandığı için yeniden uyarı vermelidir
    let res20_new = policy.update(args(20, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::LowBattery, res20_new.kind);
}

#[test]
fn hysteresis_resets_when_charge_recovers_to_25_percent() {
    let mut policy = BatteryNotificationPolicy::new();
    policy.update(args(22, false, DISCHARGING));

    // 20%'de uyarı verir
    policy.update(args(20, false, DISCHARGING));

    // Şarj %25'e yükselir (ör. kalibrasyon veya kısa şarj)
    policy.update(args(25, false, DISCHARGING));

    // Tekrar 20%'ye düşünce yeniden uyarı vermelidir
    let res20_new = policy.update(args(20, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::LowBattery, res20_new.kind);
}

#[test]
fn startup_already_low_suppressed_until_next_threshold() {
    let mut policy = BatteryNotificationPolicy::new();

    // 15% ile açılır - başlangıç uyarısı bastırılır
    let seed = policy.update(args(15, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, seed.kind);

    // 14% - 20 uyarısı verilmez
    let res14 = policy.update(args(14, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, res14.kind);

    // 10%'a düşünce alt eşik uyarısı verilmelidir
    let res10 = policy.update(args(10, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::LowBattery, res10.kind);
    assert_eq!(10, res10.percent);
}

#[test]
fn startup_already_critically_low_suppressed_completely() {
    let mut policy = BatteryNotificationPolicy::new();

    // 8% ile açılır - başlangıç uyarısı bastırılır
    let seed = policy.update(args(8, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, seed.kind);

    // 7%'ye iner - zaten kritik açıldığı için yeniden uyarmaz
    let res7 = policy.update(args(7, false, DISCHARGING));
    assert_eq!(BatteryAlertKind::None, res7.kind);
}

#[test]
fn plugged_in_while_low_does_not_emit_low_battery() {
    let mut policy = BatteryNotificationPolicy::new();
    policy.update(args(15, true, CHARGING));

    // Şarjdayken 10%'a düşse bile LowBattery vermemeli
    let res = policy.update(args(10, true, CHARGING));
    assert_eq!(BatteryAlertKind::None, res.kind);
}

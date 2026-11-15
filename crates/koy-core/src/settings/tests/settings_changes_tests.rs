//! `Settings/SettingsChangesTests.cs`.

use crate::settings::*;

#[test]
fn varsayilan_ayarlar_hic_degisiklik_icermez() {
    assert!(SettingsChanges::changed_keys(&AppSettings::default(), &AppSettings::default()).is_empty());
}

#[test]
fn tek_skaler_degisiklik_bolum_ve_alan_adiyla_bulunur() {
    let defaults = AppSettings::default();
    let current = AppSettings { dock: DockSettings { enabled: !defaults.dock.enabled, ..Default::default() }, ..Default::default() };

    let changed = SettingsChanges::changed_keys(&current, &defaults);

    assert_eq!(vec!["Dock.Enabled".to_string()], changed);
}

#[test]
fn liste_degisikligi_de_algilanir() {
    let defaults = AppSettings::default();
    let mut current = AppSettings::default();
    current.dock_items.pinned_order.push("explorer".to_string());

    let changed = SettingsChanges::changed_keys(&current, &defaults);

    assert_eq!(vec!["DockItems.PinnedOrder".to_string()], changed);
}

#[test]
fn surum_alani_degisiklik_sayilmaz() {
    let defaults = AppSettings::default();
    let current = AppSettings { version: AppSettings::CURRENT_VERSION - 1, ..Default::default() };

    assert!(SettingsChanges::changed_keys(&current, &defaults).is_empty());
}

#[test]
fn birden_cok_degisiklik_bildirim_sirasinda_doner() {
    let defaults = AppSettings::default();
    let current = AppSettings {
        dock: DockSettings {
            enabled: !defaults.dock.enabled,
            hide_windows_taskbar: !defaults.dock.hide_windows_taskbar,
            ..Default::default()
        },
        ..Default::default()
    };

    let changed = SettingsChanges::changed_keys(&current, &defaults);

    assert_eq!(2, changed.len());
    assert!(changed.contains(&"Dock.Enabled".to_string()));
    assert!(changed.contains(&"Dock.HideWindowsTaskbar".to_string()));
}

#[test]
fn sayfa_vurgusu_yalniz_o_bolumun_anahtarlarina_bakar() {
    let keys = ["Dock.Enabled", "Notch.Enabled"];

    assert!(SettingsChanges::has_changes(keys, "Dock"));
    assert!(SettingsChanges::has_changes(keys, "Notch"));
    assert!(!SettingsChanges::has_changes(keys, "Dockx"));
    assert!(!SettingsChanges::has_changes(keys, "Music"));
}

#[test]
fn uygulamanin_kendi_yazdigi_durum_alanlari_degisiklik_sayilmaz() {
    let defaults = AppSettings::default();
    let current = AppSettings {
        general: GeneralSettings { first_run_completed: true, last_seen_version: "1.2.3".to_string(), ..Default::default() },
        weather: WeatherSettings { location_updated_unix: 1_700_000_000, ..Default::default() },
        ..Default::default()
    };

    assert!(SettingsChanges::changed_keys(&current, &defaults).is_empty());
}

#[test]
fn ic_durum_alanlari_gercek_degisikligi_gizlemez() {
    let defaults = AppSettings::default();
    let current = AppSettings {
        general: GeneralSettings { first_run_completed: true, auto_check_updates: false, ..Default::default() },
        ..Default::default()
    };

    assert_eq!(vec!["General.AutoCheckUpdates".to_string()], SettingsChanges::changed_keys(&current, &defaults));
}

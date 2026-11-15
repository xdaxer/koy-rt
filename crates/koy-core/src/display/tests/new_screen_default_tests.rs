//! Karar (2026-11-06): yeni takılan ekranda çentik ve dock varsayılan açık; ayar ekran başına baştan yapılmaz.

use crate::display::*;
use crate::settings::{AppSettings, SettingsStore};
use crate::shell::WindowRect;

const ID_MAIN: &str = r"\\?\DISPLAY#GSM5B09#5&1a2b&0&UID4352#{e6f07b5f}";
const ID_SIDE: &str = r"\\?\DISPLAY#DEL4161#5&1a2b&0&UID4353#{e6f07b5f}";
const ID_TV: &str = r"\\?\DISPLAY#SAM0F00#5&1a2b&0&UID7001#{e6f07b5f}";

fn mon(id: &str, name: &str, primary: bool, left: i32) -> DisplayMonitor {
    DisplayMonitor::new(id, name, WindowRect::new(left, 0, left + 1920, 1080), WindowRect::new(left, 0, left + 1920, 1040), primary).with_handle(left as isize + 1)
}

/// Testlerde ham JSON'u gerçek `SettingsStore` yolundan (göç dahil) okur (`tests/Core.Tests/Settings/SettingsFile.cs`).
fn settings_file_load(json: &str) -> AppSettings {
    let dir = std::env::temp_dir().join(format!("koy-sf-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(&path, json).unwrap();
    let loaded = SettingsStore::new(path).load();
    let _ = std::fs::remove_dir_all(&dir);
    loaded
}

#[test]
fn varsayilan_yeni_ekran_modu_cenik_ve_dock_acik() {
    assert_eq!(NewScreenMode::NotchAndDock, MonitorPolicy::DEFAULT_NEW_SCREEN_MODE);
    assert_eq!(NewScreenMode::NotchAndDock, AppSettings::default().displays.new_screen_default);
}

#[test]
fn kayitsiz_ekranlar_varsayilan_ayarlarla_hepsi_acik() {
    let settings = AppSettings::default();

    let result = MonitorPolicy::resolve(
        Some(&settings.displays.monitors),
        settings.displays.new_screen_default,
        &[mon(ID_MAIN, "A", true, 0), mon(ID_SIDE, "B", false, 1920)],
    );

    assert!(result.iter().all(|a| a.notch_enabled && a.dock_enabled));
    assert_eq!(2, MonitorPolicy::dock_monitors(&result).len());
    assert_eq!(2, MonitorPolicy::notch_monitors(&result).len());
}

#[test]
fn bir_ekrani_kapatmak_yalniz_o_ekranin_kaydini_yazar_yeni_ekran_yine_acik_gelir() {
    let settings = AppSettings::default();
    let side = mon(ID_SIDE, "B", false, 1920);

    // Kullanıcı Ekranlar sayfasında yan ekranın dock'unu kapatır.
    let monitors = MonitorPolicy::update(
        Some(&settings.displays.monitors),
        settings.displays.new_screen_default,
        &side,
        |s| MonitorSettings { dock_enabled: false, ..s },
    );
    assert_eq!(1, monitors.len());

    // Sonra üçüncü bir ekran (TV) takılır: kaydı yok, varsayılan açık uygulanır; yan ekran kapalı kalır.
    let result = MonitorPolicy::resolve(
        Some(&monitors),
        settings.displays.new_screen_default,
        &[mon(ID_MAIN, "A", true, 0), side, mon(ID_TV, "TV", false, 3840)],
    );

    assert!(result[0].dock_enabled && !result[0].is_known);
    assert!(!result[1].dock_enabled);
    assert!(result[1].notch_enabled);
    assert!(result[2].notch_enabled && result[2].dock_enabled && !result[2].is_known);
}

#[test]
fn eski_primary_ve_all_gocu_kayit_yazmaz_ve_butun_ekranlar_acik_olur() {
    for target in ["Primary", "All", ""] {
        let migrated = settings_file_load(&format!("{{ \"Version\": 2, \"MonitorTarget\": \"{target}\" }}"));

        assert_eq!(0, migrated.displays.monitors.len());
        assert_eq!(NewScreenMode::NotchAndDock, migrated.displays.new_screen_default);

        let result = MonitorPolicy::resolve(
            Some(&migrated.displays.monitors),
            migrated.displays.new_screen_default,
            &[mon(ID_MAIN, "A", true, 0), mon(ID_SIDE, "B", false, 1920)],
        );
        assert!(result.iter().all(|a| a.notch_enabled && a.dock_enabled), "{target:?}");
    }
}

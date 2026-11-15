//! `Settings/FeatureStateSyncTests.cs`.

use super::TempDir;
use crate::appearance::AppTheme;
use crate::settings::*;

fn new_store(dir: &TempDir) -> SettingsStore {
    SettingsStore::new(dir.file("settings.json"))
}

#[test]
fn reconcile_updates_only_specified_states() {
    let initial = AppSettings {
        notch: NotchSettings { enabled: true, ..Default::default() },
        dock: DockSettings { enabled: true, ..Default::default() },
        ..Default::default()
    };

    let dock_disabled = FeatureStateSyncPolicy::reconcile(&initial, None, Some(false));
    assert!(dock_disabled.notch.enabled);
    assert!(!dock_disabled.dock.enabled);

    let notch_disabled = FeatureStateSyncPolicy::reconcile(&initial, Some(false), None);
    assert!(!notch_disabled.notch.enabled);
    assert!(notch_disabled.dock.enabled);

    let both_changed = FeatureStateSyncPolicy::reconcile(&initial, Some(false), Some(false));
    assert!(!both_changed.notch.enabled);
    assert!(!both_changed.dock.enabled);

    let unchanged = FeatureStateSyncPolicy::reconcile(&initial, None, None);
    assert_eq!(initial, unchanged);
}

#[test]
fn reconcile_preserves_all_other_settings() {
    let complex = AppSettings {
        general: GeneralSettings { language: "en".to_string(), ..Default::default() },
        appearance: AppearanceSettings { theme: AppTheme::Light, corner_radius: 18.0, ..Default::default() },
        dock: DockSettings { icon_size: 36.0, hide_windows_taskbar: false, enabled: true, ..Default::default() },
        notch: NotchSettings { enabled: true, ..Default::default() },
        ..Default::default()
    };

    let reconciled = FeatureStateSyncPolicy::reconcile(&complex, None, Some(false));

    assert!(!reconciled.dock.enabled);
    assert!(reconciled.notch.enabled);
    assert_eq!("en", reconciled.general.language);
    assert_eq!(AppTheme::Light, reconciled.appearance.theme);
    assert_eq!(18.0, reconciled.appearance.corner_radius);
    assert_eq!(36.0, reconciled.dock.icon_size);
    assert!(!reconciled.dock.hide_windows_taskbar);
}

#[test]
fn simulated_external_toggle_prevents_stale_overwrite() {
    let dir = TempDir::unique("koy-sync-tests-");
    let store = new_store(&dir);
    store
        .save(&AppSettings {
            notch: NotchSettings { enabled: true, ..Default::default() },
            dock: DockSettings { enabled: true, ..Default::default() },
            appearance: AppearanceSettings { theme: AppTheme::Dark, ..Default::default() },
            ..Default::default()
        })
        .unwrap();

    // 1. Bellek kopyası başlangıç ayarlarını yükler
    let in_memory_copy = store.load();
    assert!(in_memory_copy.dock.enabled);

    // 2. Harici işlem (Sağ alt menü veya kısayol) dock'u kapatıp diske yazar
    let current_disk = store.load();
    store
        .save(&AppSettings { dock: DockSettings { enabled: false, ..current_disk.dock.clone() }, ..current_disk.clone() })
        .unwrap();

    // 3. Kullanıcı arayüzde Theme ayarını "Light" olarak değiştirir
    let in_memory_copy = AppSettings {
        appearance: AppearanceSettings { theme: AppTheme::Light, ..in_memory_copy.appearance.clone() },
        ..in_memory_copy
    };

    // 4. Kapanışta diski ezmeden önce disk durumuyla uzlaştırılır
    let disk_state_on_close = store.load();
    let reconciled_to_save = FeatureStateSyncPolicy::reconcile(
        &in_memory_copy,
        Some(disk_state_on_close.notch.enabled),
        Some(disk_state_on_close.dock.enabled),
    );
    store.save(&reconciled_to_save).unwrap();

    // 5. Sonuç: Tema "Light" olur, ancak DockEnabled false kalır (ezilmez)
    let final_disk = store.load();
    assert_eq!(AppTheme::Light, final_disk.appearance.theme);
    assert!(!final_disk.dock.enabled);
    assert!(final_disk.notch.enabled);
}

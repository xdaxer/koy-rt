use super::TempDir;
use crate::dock::{DockIconAlignment, DockSettingsDiff};
use crate::settings::{AppSettings, DockSettings, SettingsStore};

fn with_alignment(alignment: DockIconAlignment) -> AppSettings {
    AppSettings { dock: DockSettings { icon_alignment: alignment, ..Default::default() }, ..Default::default() }
}

#[test]
fn default_is_center() {
    assert_eq!(DockIconAlignment::Center, AppSettings::default().dock.icon_alignment);
}

#[test]
fn old_settings_file_without_the_field_stays_centered() {
    let dir = TempDir::new("koy-tests-");
    std::fs::write(dir.settings_path(), r#"{ "DockStyle": "FullWidth" }"#).unwrap();

    assert_eq!(DockIconAlignment::Center, SettingsStore::new(dir.settings_path()).load().dock.icon_alignment);
}

#[test]
fn round_trips_by_name() {
    for (alignment, name) in [(DockIconAlignment::Left, "Left"), (DockIconAlignment::Right, "Right")] {
        let dir = TempDir::new("koy-tests-");
        let store = SettingsStore::new(dir.settings_path());

        store.save(&with_alignment(alignment)).unwrap();

        let text = std::fs::read_to_string(dir.settings_path()).unwrap();
        assert!(text.contains(&format!("\"IconAlignment\": \"{name}\"")), "{name}: {text}");
        assert_eq!(alignment, store.load().dock.icon_alignment);
    }
}

#[test]
fn alignment_takes_part_in_equality() {
    assert_ne!(AppSettings::default(), with_alignment(DockIconAlignment::Left));
}

#[test]
fn changing_alignment_only_rebuilds_view_models() {
    let d = DockSettingsDiff::compare(&AppSettings::default(), &with_alignment(DockIconAlignment::Right));

    assert!(d.rebuild_view_models);
    assert!(!d.reconcile_app_bar);
    assert!(!d.reconcile_taskbar);
    assert!(!d.reconcile_monitors);
    assert!(!d.reconcile_tray);
}

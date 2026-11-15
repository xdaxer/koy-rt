use crate::appearance::{AnimationSpeed, IconPack, MaterialLevel};
use crate::display::{MonitorSettings, NewScreenMode};
use crate::dock::{DockEdge, DockProximityMode, DockSettingsDiff, DockStyleMode};
use crate::settings::{AppSettings, DockSettings};
use crate::visibility::VisibilityMode;

/// `new AppSettings { Dock = new DockSettings { ... } }`: varsayılan ayarlarda yalnız dock bölümü değişir.
fn dock(f: impl FnOnce(&mut DockSettings)) -> AppSettings {
    let mut s = AppSettings::default();
    f(&mut s.dock);
    s
}

fn appearance(f: impl FnOnce(&mut crate::settings::AppearanceSettings)) -> AppSettings {
    let mut s = AppSettings::default();
    f(&mut s.appearance);
    s
}

#[test]
fn identical_settings_need_nothing() {
    assert!(DockSettingsDiff::compare(&AppSettings::default(), &AppSettings::default()).is_empty());
}

#[test]
fn notch_only_change_needs_nothing() {
    let mut after = AppSettings::default();
    after.notch.visibility = VisibilityMode::Smart;
    assert!(DockSettingsDiff::compare(&AppSettings::default(), &after).is_empty());
}

#[test]
fn toggling_a_button_only_rebuilds_view_models() {
    let d = DockSettingsDiff::compare(&AppSettings::default(), &dock(|d| d.show_recents = false));

    assert!(d.rebuild_view_models);
    assert!(!d.reconcile_app_bar);
    assert!(!d.reconcile_taskbar);
    assert!(!d.reconcile_monitors);
    assert!(!d.reconcile_tray);
}

#[test]
fn icon_pack_and_app_icon_changes_rebuild_view_models_only() {
    let baseline = AppSettings::default();
    let variants = [
        appearance(|a| a.icon_pack = IconPack::Folder),
        appearance(|a| a.icon_pack_folder = r"C:\Simgeler".to_string()),
        appearance(|a| {
            a.app_icon_overrides.insert("Notepad.lnk", r"C:\a.png".to_string());
        }),
    ];

    for after in &variants {
        let d = DockSettingsDiff::compare(&baseline, after);
        assert!(d.rebuild_view_models);
        assert!(!d.reconcile_app_bar);
        assert!(!d.reconcile_taskbar);
        assert!(!d.reconcile_tray);
    }
}

#[test]
fn style_change_reconciles_appbar_but_never_the_taskbar() {
    let d = DockSettingsDiff::compare(&AppSettings::default(), &dock(|d| d.style = DockStyleMode::FullWidth));

    assert!(d.rebuild_view_models);
    assert!(d.reconcile_app_bar);
    assert!(!d.reconcile_taskbar);
}

#[test]
fn proximity_change_without_effective_change_keeps_the_appbar() {
    // Tam genişlikte Aşağı yapış ve Yer ayır aynı etkin modu verir: AppBar'a dokunulmaz.
    let a = dock(|d| {
        d.style = DockStyleMode::FullWidth;
        d.proximity = DockProximityMode::DockDown;
    });
    let b = dock(|d| {
        d.style = DockStyleMode::FullWidth;
        d.proximity = DockProximityMode::ReserveSpace;
    });

    assert!(!DockSettingsDiff::compare(&a, &b).reconcile_app_bar);
}

#[test]
fn capsule_reserve_to_dockdown_reconciles_appbar() {
    assert!(
        DockSettingsDiff::compare(
            &dock(|d| d.proximity = DockProximityMode::ReserveSpace),
            &dock(|d| d.proximity = DockProximityMode::DockDown)
        )
        .reconcile_app_bar
    );
}

#[test]
fn capsule_reserve_to_always_floating_releases_the_appbar() {
    let d = DockSettingsDiff::compare(
        &dock(|d| d.proximity = DockProximityMode::ReserveSpace),
        &dock(|d| d.proximity = DockProximityMode::AlwaysFloating),
    );

    assert!(d.reconcile_app_bar);
    assert!(d.rebuild_view_models);
    assert!(!d.reconcile_taskbar);
}

#[test]
fn dockdown_to_always_floating_keeps_the_appbar_but_rebuilds_view_models() {
    // İkisi de yer ayırmaz: AppBar'a dokunulmaz, ama yakınlık ayarı ViewModel'e okunur, yeniden kurulur.
    let d = DockSettingsDiff::compare(
        &dock(|d| d.proximity = DockProximityMode::DockDown),
        &dock(|d| d.proximity = DockProximityMode::AlwaysFloating),
    );

    assert!(!d.reconcile_app_bar);
    assert!(d.rebuild_view_models);
}

#[test]
fn full_width_always_floating_maps_to_reserve_space_so_the_appbar_stays() {
    // Tam genişlikte Hep havada etkin olarak Yer ayır'dır: Yer ayır'dan geçişte AppBar'a dokunulmaz.
    let a = dock(|d| {
        d.style = DockStyleMode::FullWidth;
        d.proximity = DockProximityMode::AlwaysFloating;
    });
    let b = dock(|d| {
        d.style = DockStyleMode::FullWidth;
        d.proximity = DockProximityMode::ReserveSpace;
    });

    assert!(!DockSettingsDiff::compare(&a, &b).reconcile_app_bar);
}

#[test]
fn hide_taskbar_toggle_reconciles_taskbar_and_tray() {
    let d = DockSettingsDiff::compare(&dock(|d| d.hide_windows_taskbar = true), &dock(|d| d.hide_windows_taskbar = false));

    assert!(d.reconcile_taskbar);
    assert!(d.reconcile_tray);
    assert!(d.reconcile_app_bar); // alt kenar sahipliği değişir: AppBar yeniden iddia edilmeli
}

#[test]
fn monitor_changes_reconcile_monitors() {
    let mut new_screen = AppSettings::default();
    new_screen.displays.new_screen_default = NewScreenMode::NotchOnly;
    assert!(DockSettingsDiff::compare(&AppSettings::default(), &new_screen).reconcile_monitors);

    let mut monitors = AppSettings::default();
    monitors.displays.monitors.insert("a", MonitorSettings { dock_enabled: false, ..Default::default() });
    assert!(DockSettingsDiff::compare(&AppSettings::default(), &monitors).reconcile_monitors);
}

#[test]
fn tray_only_change_reconciles_tray_and_rebuilds_without_touching_taskbar_or_appbar() {
    let d = DockSettingsDiff::compare(&dock(|d| d.show_tray = true), &dock(|d| d.show_tray = false));

    assert!(d.reconcile_tray);
    assert!(d.rebuild_view_models);
    assert!(!d.reconcile_taskbar);
    assert!(!d.reconcile_app_bar);
}

#[test]
fn appearance_changes_only_rebuild_view_models() {
    for field in ["Material", "AnimationSpeed"] {
        let after = match field {
            "Material" => appearance(|a| a.material = MaterialLevel::Frosted),
            _ => appearance(|a| a.animation_speed = AnimationSpeed::Fast),
        };

        let d = DockSettingsDiff::compare(&AppSettings::default(), &after);

        assert!(d.rebuild_view_models, "{field}");
        assert!(!d.reconcile_app_bar, "{field}");
        assert!(!d.reconcile_monitors, "{field}");
        assert!(!d.reconcile_taskbar, "{field}");
    }
}

#[test]
fn accent_color_does_not_touch_the_dock_it_updates_live_through_dynamic_resources() {
    let after = appearance(|a| {
        a.accent_color = Some("#FF0000".to_string());
        a.use_windows_accent = true;
    });
    assert!(DockSettingsDiff::compare(&AppSettings::default(), &after).is_empty());
}

#[test]
fn monitor_change_reconciles_monitors_only() {
    let mut after = AppSettings::default();
    after.displays.monitors.insert("x", MonitorSettings { dock_enabled: false, ..Default::default() });

    let d = DockSettingsDiff::compare(&AppSettings::default(), &after);

    assert!(d.reconcile_monitors);
    assert!(!d.rebuild_view_models);
    assert!(!d.reconcile_app_bar);
}

#[test]
fn reserve_to_dodge_removes_the_appbar() {
    let d = DockSettingsDiff::compare(
        &dock(|d| d.proximity = DockProximityMode::ReserveSpace),
        &dock(|d| d.proximity = DockProximityMode::Dodge),
    );

    assert!(d.reconcile_app_bar);
    assert!(d.rebuild_view_models);
}

#[test]
fn battery_saver_toggle_reconciles_power_and_rebuilds() {
    let d = DockSettingsDiff::compare(
        &appearance(|a| a.battery_saver_reduce_motion = true),
        &appearance(|a| a.battery_saver_reduce_motion = false),
    );

    assert!(d.reconcile_power);
    assert!(d.rebuild_view_models);
}

#[test]
fn edge_change_reconciles_appbar_and_rebuilds() {
    let d = DockSettingsDiff::compare(&dock(|d| d.edge = DockEdge::Bottom), &dock(|d| d.edge = DockEdge::Top));

    assert!(d.reconcile_app_bar);
    assert!(d.rebuild_view_models);
}

#[test]
fn length_percent_change_reconciles_appbar_and_rebuilds() {
    let d = DockSettingsDiff::compare(&dock(|d| d.length_percent = 100), &dock(|d| d.length_percent = 80));

    assert!(d.reconcile_app_bar);
    assert!(d.rebuild_view_models);
}

#[test]
fn edge_offset_dip_change_reconciles_appbar_and_rebuilds() {
    let d = DockSettingsDiff::compare(&dock(|d| d.edge_offset_dip = 0.0), &dock(|d| d.edge_offset_dip = 10.0));

    assert!(d.reconcile_app_bar);
    assert!(d.rebuild_view_models);
}

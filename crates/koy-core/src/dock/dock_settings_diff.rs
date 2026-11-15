use super::dock_proximity_mapping::DockProximityMapping;
use super::dock_proximity_mode::DockProximityMode;
use crate::settings::{AppSettings, DockSettings};

/// Ayar değişince çalışan dock'ta NELERİN yeniden uygulanacağı. Dock'u durdurup başlatmak Windows görev çubuğunu geri
/// getirir, AppBar kaydını siler ve çalışma alanını oynatır (büyütülmüş pencereler sıçrar); bu yüzden ayar değişimi hiçbir
/// zaman yeniden başlatma gerektirmez: pencereler ve AppBar yerinde kalır, yalnız değişenler yeniden kurulur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DockSettingsDiff {
    pub rebuild_view_models: bool,
    pub reconcile_monitors: bool,
    pub reconcile_app_bar: bool,
    pub reconcile_taskbar: bool,
    pub reconcile_tray: bool,
    pub reconcile_power: bool,
}

impl DockSettingsDiff {
    /// Dock'u ilgilendiren hiçbir şey değişmedi (örn. yalnız çentik ayarı).
    pub fn is_empty(&self) -> bool {
        *self == DockSettingsDiff::default()
    }

    pub fn compare(before: &AppSettings, after: &AppSettings) -> DockSettingsDiff {
        let monitors = before.displays != after.displays;

        let before_reserves =
            DockProximityMapping::effective(before.dock.style, before.dock.proximity) == DockProximityMode::ReserveSpace;
        let after_reserves =
            DockProximityMapping::effective(after.dock.style, after.dock.proximity) == DockProximityMode::ReserveSpace;
        let app_bar = before_reserves != after_reserves
            || before.dock.style != after.dock.style
            || before.dock.hide_windows_taskbar != after.dock.hide_windows_taskbar
            || before.dock.edge != after.dock.edge
            || before.dock.length_percent != after.dock.length_percent
            || before.dock.edge_offset_dip != after.dock.edge_offset_dip;

        let taskbar = before.dock.hide_windows_taskbar != after.dock.hide_windows_taskbar;
        let tray = before.dock.show_tray != after.dock.show_tray || taskbar;
        let power = before.appearance.battery_saver_reduce_motion != after.appearance.battery_saver_reduce_motion;

        // ViewModel / VisibilityController / EdgeTracker ayarı kurulurken okur: Dock bölümündeki herhangi bir alan
        // (Enabled ve HideWindowsTaskbar hariç: onları DockFeature ayrıca ele alır) ya da dock'un çizimini etkileyen
        // görünüm alanları değiştiyse yeniden kurulur (pencere ve tutamaç aynı kalır). DockItems (sıra, klasörler)
        // bilerek dışarıda: dock sürüklenince kendini yeniden kurmamalı.
        let rebuild = Self::rebuildable(&before.dock) != Self::rebuildable(&after.dock)
            || before.appearance.material != after.appearance.material
            || before.appearance.material_readability_guard != after.appearance.material_readability_guard
            || before.appearance.animation_speed != after.appearance.animation_speed
            || before.appearance.respect_reduce_motion != after.appearance.respect_reduce_motion
            || before.appearance.icon_pack != after.appearance.icon_pack
            || before.appearance.icon_pack_folder != after.appearance.icon_pack_folder
            || before.appearance.app_icon_overrides != after.appearance.app_icon_overrides
            || power;

        DockSettingsDiff {
            rebuild_view_models: rebuild,
            reconcile_monitors: monitors,
            reconcile_app_bar: app_bar,
            reconcile_taskbar: taskbar,
            reconcile_tray: tray,
            reconcile_power: power,
        }
    }

    // Enabled ve HideWindowsTaskbar yeniden kurmayı değil, uzlaştırmayı (görev çubuğu, AppBar) ilgilendirir.
    fn rebuildable(dock: &DockSettings) -> DockSettings {
        DockSettings { enabled: false, hide_windows_taskbar: false, ..dock.clone() }
    }
}

mod fakes;

mod app_bar_math_tests;
mod app_bar_ownership_tests;
mod app_bar_placement_cache_tests;
mod dock_drop_zone_tests;
mod dock_fit_math_tests;
mod dock_folders_tests;
mod dock_foreground_filter_tests;
mod dock_icon_alignment_tests;
mod dock_item_app_id_tests;
mod dock_item_layout_tests;
mod dock_item_rules_tests;
mod dock_item_settings_tests;
mod dock_items_diff_tests;
mod dock_layout_math_tests;
mod dock_position_resolver_tests;
mod dock_presentation_math_tests;
mod dock_press_tests;
mod dock_proximity_mapping_tests;
mod dock_right_section_tests;
mod dock_settings_diff_tests;
mod dock_start_icon_tests;
mod dock_tests;
mod dock_window_behavior_tests;
mod dock_window_grouping_tests;
mod edge_reveal_and_fullscreen_tests;
mod pinned_app_matcher_tests;
mod pinned_order_tests;
mod recent_files_policy_tests;
mod refresh_debouncer_tests;
mod snap_debounce_tests;
mod taskband_packaged_pins_tests;
mod thumbnail_fit_tests;
mod tray_glyphs_tests;

use std::path::PathBuf;

/// `Path.Combine(Path.GetTempPath(), prefix + Guid.NewGuid())` + `Directory.CreateDirectory`; düşünce klasörü siler.
pub(crate) struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("{prefix}{}", crate::dock::dock_folder::new_short_id(16)));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }

    pub fn settings_path(&self) -> PathBuf {
        self.0.join("settings.json")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `tests/Core.Tests/Settings/SettingsFile.cs`: ham JSON'u gerçek `SettingsStore` yolundan (göç dahil) okur.
pub(crate) fn load_settings(json: &str) -> crate::settings::AppSettings {
    let dir = TempDir::new("koy-sf-");
    let path = dir.settings_path();
    std::fs::write(&path, json).unwrap();
    crate::settings::SettingsStore::new(path).load()
}

/// `string[]` / `List<string>` kısayolu.
pub(crate) fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

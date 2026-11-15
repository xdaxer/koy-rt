//! Karşılığı: `koy/src/Core/Dock/`.
//!
//! Dock'un saf mantığı: konum çözücü, yerleşim hesabı, AppBar kararları, öğe sırası ve klasörler, denetleyici.
//! `Items/` alt klasörü `items` alt modülüdür; tipleri burada da `pub use items::*;` ile açılır
//! (`crate::dock::DockItemRules`).

mod app_bar_math;
mod app_bar_ownership;
mod app_bar_placement_cache;
mod dock_click_policy;
mod dock_controller;
mod dock_drop_zone;
mod dock_edge;
mod dock_enums;
mod dock_fit_math;
mod dock_folder;
mod dock_folders;
mod dock_foreground_filter;
mod dock_icon_alignment;
mod dock_item;
mod dock_layout_limits;
mod dock_layout_math;
mod dock_monitor_filter;
mod dock_position_resolver;
mod dock_presentation_enums;
mod dock_presentation_math;
mod dock_press;
mod dock_proximity_mapping;
mod dock_proximity_mode;
mod dock_reorder_math;
mod dock_right_section;
mod dock_settings_diff;
mod dock_start_icon;
mod dock_window_grouping;
mod edge_reveal_tracker;
mod i_app_bar_service;
mod i_pinned_apps_service;
pub mod items;
mod known_app_user_model_ids;
mod ordered_pinned_apps_service;
mod pinned_app_matcher;
mod pinned_order;
mod recent_files;
mod refresh_debouncer;
mod snap_debounce;
mod taskband_packaged_pins;
mod thumbnail_fit;
mod tray_glyphs;

pub use app_bar_math::*;
pub use app_bar_ownership::*;
pub use app_bar_placement_cache::*;
pub use dock_click_policy::*;
pub use dock_controller::*;
pub use dock_drop_zone::*;
pub use dock_edge::*;
pub use dock_enums::*;
pub use dock_fit_math::*;
pub use dock_folder::DockFolder;
pub use dock_folders::*;
pub use dock_foreground_filter::*;
pub use dock_icon_alignment::*;
pub use dock_item::*;
pub use dock_layout_limits::*;
pub use dock_layout_math::*;
pub use dock_monitor_filter::*;
pub use dock_position_resolver::*;
pub use dock_presentation_enums::*;
pub use dock_presentation_math::*;
pub use dock_press::*;
pub use dock_proximity_mapping::*;
pub use dock_proximity_mode::*;
pub use dock_reorder_math::*;
pub use dock_right_section::*;
pub use dock_settings_diff::*;
pub use dock_start_icon::*;
pub use dock_window_grouping::*;
pub use edge_reveal_tracker::*;
pub use i_app_bar_service::*;
pub use i_pinned_apps_service::*;
pub use items::*;
pub use known_app_user_model_ids::*;
pub use ordered_pinned_apps_service::*;
pub use pinned_app_matcher::*;
pub use pinned_order::*;
pub use recent_files::*;
pub use refresh_debouncer::*;
pub use snap_debounce::*;
pub use taskband_packaged_pins::*;
pub use thumbnail_fit::*;
pub use tray_glyphs::*;

#[cfg(test)]
mod tests;

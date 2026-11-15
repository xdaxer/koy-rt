//! Karşılığı: `koy/src/Core/Display/`.
//!
//! Monitör modeli, hedef monitör çözümü, ekran şeması, parlaklık kaynak seçimi ve pencere yerleşim hesapları.

mod brightness_source_picker;
mod brightness_write_queue;
mod display_layout_resolver;
mod display_schema_layout;
mod i_brightness_service;
mod i_monitor_service;
mod monitor_layout_signature;
mod monitor_policy;
mod monitor_settings;
mod window_placement_math;

pub use brightness_source_picker::*;
pub use brightness_write_queue::*;
pub use display_layout_resolver::*;
pub use display_schema_layout::*;
pub use i_brightness_service::*;
pub use i_monitor_service::*;
pub use monitor_layout_signature::*;
pub use monitor_policy::*;
pub use monitor_settings::*;
pub use window_placement_math::*;

#[cfg(test)]
mod tests;

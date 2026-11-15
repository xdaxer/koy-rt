//! Karşılığı: `koy/src/Core/JumpList/`.
//!
//! Uygulama bazlı atlama listesi (görev çubuğu sağ tık menüsü): `customDestinations-ms` ikili okuyucusu, bölüm
//! düzeni / sınırları, önbellekler ve çalıştırma planı. Windows API'si yok; kabuk okuması Platform'a aittir.

pub mod custom_destinations;
pub mod exe_app_id_cache;
pub mod i_jump_list_service;
pub mod jump_list_app_id;
pub mod jump_list_cache;
pub mod jump_list_entry_factory;
pub mod jump_list_launch_plan;
pub mod jump_list_layout;
pub mod jump_list_limits;
pub mod jump_list_models;
pub mod shortcut_app_id_matcher;
pub mod win_path;

pub use custom_destinations::{
    CustomDestinationsCategory, CustomDestinationsCategoryKind, CustomDestinationsFile, CustomDestinationsReader,
    JumpListKnownCategory,
};
pub use exe_app_id_cache::ExeAppIdCache;
pub use i_jump_list_service::IJumpListService;
pub use jump_list_app_id::JumpListAppId;
pub use jump_list_cache::{FileStamp, JumpListCache};
pub use jump_list_entry_factory::{JumpListEntryFactory, JumpListText};
pub use jump_list_launch_plan::JumpListLaunchPlan;
pub use jump_list_layout::JumpListLayout;
pub use jump_list_limits::JumpListLimits;
pub use jump_list_models::{JumpListEntry, JumpListLaunch, JumpListSection, JumpListSectionKind};
pub use shortcut_app_id_matcher::ShortcutAppIdMatcher;

#[cfg(test)]
mod tests;

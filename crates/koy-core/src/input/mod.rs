//! Karşılığı: `koy/src/Core/Input/`.
//!
//! Eylem/tetikleyici bağ modeli (`InputBinding`), ham ayarı çözen `InputBindingParser`, bağları denetleyen
//! `InputBindingValidator`, özel eylemler, köşe matematiği ve köşe ayarının saf kararları.

pub mod corner_actions;
pub mod custom_action;
pub mod hot_corner_math;
pub mod i_corner_hook_service;
pub mod input_action;
pub mod input_binding;
pub mod input_binding_parser;
pub mod input_binding_validator;

pub use corner_actions::*;
pub use custom_action::*;
pub use hot_corner_math::*;
pub use i_corner_hook_service::*;
pub use input_action::*;
pub use input_binding::*;
pub use input_binding_parser::*;
pub use input_binding_validator::*;

#[cfg(test)]
mod tests;

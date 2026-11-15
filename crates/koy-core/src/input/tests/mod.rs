//! `koy/tests/Core.Tests` altındaki Input testleri: CustomActionParserTests, HotCornerActionsTests (+ HotCornerTrackerTests),
//! HotCornerMathTests, InputBindingTests.
//!
//! Atlananlar (HotCornerActionsTests):
//! - `None_and_undefined_values_map_to_nothing` içindeki `(CornerAction)999` kısmı: Rust enum'unda tanımsız değer
//!   temsil edilemez; `None` kısmı taşındı.

mod custom_action_parser_tests;
mod hot_corner_actions_tests;
mod hot_corner_math_tests;
mod input_binding_tests;

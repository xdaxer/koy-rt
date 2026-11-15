//! Karşılığı: `koy/src/Core/Appearance/`.
//!
//! Tema, vurgu rengi, malzeme (saydamlık), hareket, yazı tipi ölçeği, köşe, simge ve kullanıcı stil dosyası kararları.
//! Saf mantık: WPF, dosya çizimi ya da Windows API yok.

mod accent_palette;
mod app_icon_key;
mod app_icon_overrides;
mod appearance_enums;
mod color_math;
mod corner_style;
mod font_scale_policy;
mod icon_pack;
mod icon_pack_files;
mod icon_resolver;
mod material_setting;
mod motion_policy;
mod readability_guard;
mod theme_setting;
mod user_style_file;
mod user_style_overrides;

pub use accent_palette::*;
pub use app_icon_key::*;
pub use app_icon_overrides::*;
pub use appearance_enums::*;
pub use color_math::*;
pub use corner_style::*;
pub use font_scale_policy::*;
pub use icon_pack::*;
pub use icon_pack_files::*;
pub use icon_resolver::*;
pub use material_setting::*;
pub use motion_policy::*;
pub use readability_guard::*;
pub use theme_setting::*;
pub use user_style_file::*;
pub use user_style_overrides::*;

/// C# `string.Length` (UTF-16 kod birimi sayısı). Uzunluk sınırları C#'ta bununla ölçülür.
pub(crate) fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `char.IsControl` (UTF-16 kod birimi için): U+0000..U+001F ve U+007F..U+009F.
pub(crate) fn is_control_unit(unit: u16) -> bool {
    unit <= 0x1F || (0x7F..=0x9F).contains(&unit)
}

#[cfg(test)]
mod tests;

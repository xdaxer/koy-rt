//! Pano sayfası durumunu Slint yapılarına çevirir ve sabit metinleri kurar.

use slint::{ComponentHandle, ModelRc, VecModel};

use koy_core::clipboard::ClipboardFilter;

use super::clipboard_view_model::ClipboardState;
use crate::resources::strings::Strings;
use crate::{ClipCardData, ClipFilterData, ClipText};

pub fn filter_index(f: ClipboardFilter) -> i32 {
    match f {
        ClipboardFilter::All => 0,
        ClipboardFilter::Pinned => 1,
        ClipboardFilter::Link => 2,
        ClipboardFilter::Color => 3,
        ClipboardFilter::Code => 4,
    }
}

pub fn filter_from_index(i: i32) -> ClipboardFilter {
    match i {
        1 => ClipboardFilter::Pinned,
        2 => ClipboardFilter::Link,
        3 => ClipboardFilter::Color,
        4 => ClipboardFilter::Code,
        _ => ClipboardFilter::All,
    }
}

pub fn filters(s: &ClipboardState) -> ModelRc<ClipFilterData> {
    let v: Vec<ClipFilterData> = s
        .filters
        .iter()
        .map(|f| ClipFilterData { filter: filter_index(f.filter), label: f.label.clone().into(), selected: f.is_selected })
        .collect();
    ModelRc::new(VecModel::from(v))
}

pub fn cards(s: &ClipboardState) -> ModelRc<ClipCardData> {
    let v: Vec<ClipCardData> = s
        .items
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let (a, r, g, b) = c.swatch.unwrap_or_default();
            ClipCardData {
                index: i as i32,
                header: c.header.clone().into(),
                preview: c.preview.clone().into(),
                is_pinned: c.is_pinned,
                is_link: c.is_link(),
                is_copied: c.is_copied,
                has_swatch: c.swatch.is_some(),
                swatch: slint::Color::from_argb_u8(a, r, g, b),
                pin_tip: c.pin_tip().into(),
            }
        })
        .collect();
    ModelRc::new(VecModel::from(v))
}

pub fn install_text<C: ComponentHandle>(c: &C)
where
    for<'a> ClipText<'a>: slint::Global<'a, C>,
{
    let t = c.global::<ClipText>();
    t.set_search(Strings::get("ClipSearch").into());
    t.set_clear(Strings::get("ClipClear").into());
    t.set_copied(Strings::get("ClipCopied").into());
    t.set_footer(Strings::get("ClipFooter").into());
}

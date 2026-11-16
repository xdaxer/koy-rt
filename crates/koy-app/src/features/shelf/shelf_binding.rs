//! Raf durumunu Slint yapılarına çevirir ve panelin sabit metinlerini kurar.

use slint::{ComponentHandle, ModelRc, VecModel};

use koy_core::shelf::{ShelfItem, ShelfItemKind};

use super::shelf_view_model::badge;
use crate::resources::strings::Strings;
use crate::{ShelfItemData, ShelfText};

pub fn to_items(items: &[ShelfItem]) -> ModelRc<ShelfItemData> {
    let v: Vec<ShelfItemData> = items
        .iter()
        .enumerate()
        .map(|(i, it)| ShelfItemData {
            index: i as i32,
            id: it.id.clone().into(),
            name: it.display_name.clone().into(),
            tooltip: it.path_or_content.clone().into(),
            kind: match it.kind {
                ShelfItemKind::File => 0,
                ShelfItemKind::Directory => 1,
                ShelfItemKind::Text => 2,
            },
            badge: badge(it).into(),
        })
        .collect();
    ModelRc::new(VecModel::from(v))
}

pub fn install_text<C: ComponentHandle>(c: &C)
where
    for<'a> ShelfText<'a>: slint::Global<'a, C>,
{
    let t = c.global::<ShelfText>();
    t.set_shelf(Strings::get("Shelf").into());
    t.set_clear(Strings::get("ShelfClear").into());
    t.set_drop_here(Strings::get("ShelfDropHere").into());
}

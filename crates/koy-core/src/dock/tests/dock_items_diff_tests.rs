//! Dock'un kendi yazdığı veriler (sıra, klasörler) ayrı bölümde. Sürükleyerek sıralama dock'u yeniden kurmamalı;
//! yoksa her sürüklemede pencereler sıçrar.

use super::strs;
use crate::dock::{DockFolder, DockSettingsDiff};
use crate::settings::{AppSettings, DockItemsSettings, ValueList};

#[test]
fn reordering_pinned_apps_needs_nothing() {
    let before = AppSettings {
        dock_items: DockItemsSettings { pinned_order: ValueList::from(strs(&["a.lnk", "b.lnk"])), ..Default::default() },
        ..Default::default()
    };
    let after = AppSettings {
        dock_items: DockItemsSettings { pinned_order: ValueList::from(strs(&["b.lnk", "a.lnk"])), ..Default::default() },
        ..Default::default()
    };

    assert!(DockSettingsDiff::compare(&before, &after).is_empty());
}

#[test]
fn changing_folders_needs_nothing() {
    let before = AppSettings::default();
    let after = AppSettings {
        dock_items: DockItemsSettings {
            folders: ValueList::from(vec![DockFolder::new("f1", "Araçlar", strs(&["calc.lnk"]))]),
            ..Default::default()
        },
        ..Default::default()
    };

    assert!(DockSettingsDiff::compare(&before, &after).is_empty());
}

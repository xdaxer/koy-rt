//! Yeni öğe listesi eski ayar dosyalarını bozmamalı; PinnedOrder aynen okunmalı.

use super::{load_settings, strs};
use crate::dock::{DockItemEntry, DockItemKind, DockItemRules};
use crate::settings::{AppSettings, DockItemsSettings, ValueList};

#[test]
fn eski_pinned_order_verisi_degismeden_yuklenir_ve_items_bos_kalir() {
    let json = r#"{ "Version": 4, "DockItems": { "PinnedOrder": ["Obsidian.lnk", "folder:f1"] } }"#;

    let loaded = load_settings(json);

    assert_eq!(strs(&["Obsidian.lnk", "folder:f1"]), *loaded.dock_items.pinned_order);
    assert!(loaded.dock_items.items.is_empty());
}

#[test]
fn items_kaydi_okunur_ve_bilinmeyen_tur_dogrulamada_atilir() {
    let json = r#"
        {
          "Version": 4,
          "DockItems": {
            "Items": [
              { "Kind": "app", "Id": "Obsidian.lnk" },
              { "Kind": "battery", "Id": "" },
              { "Kind": "custom", "Id": "c1", "Command": "calc.exe" }
            ]
          }
        }
        "#;

    let loaded = load_settings(json);
    let specs = DockItemRules::normalize(&loaded.dock_items.items, DockItemRules::MAX_ITEM_COUNT);

    assert_eq!(3, loaded.dock_items.items.len());
    assert_eq!(vec![DockItemKind::App, DockItemKind::Custom], specs.iter().map(|s| s.kind).collect::<Vec<_>>());
    assert_eq!("calc.exe", specs[1].command);
}

#[test]
fn items_bos_ya_da_yoksa_varsayilan_bos_listedir() {
    let loaded = load_settings(r#"{ "Version": 4, "DockItems": { "Items": null } }"#);

    assert!(loaded.dock_items.items.is_empty());
}

#[test]
fn eski_pinned_order_kullanicisi_items_olmadan_ayni_docku_gorur() {
    let json = r#"{ "Version": 4, "DockItems": { "PinnedOrder": ["Obsidian.lnk", "folder:f1", "Code.lnk"] } }"#;

    let loaded = load_settings(json);
    let specs = DockItemRules::resolve(&loaded.dock_items.items, &loaded.dock_items.pinned_order, DockItemRules::MAX_ITEM_COUNT);

    assert_eq!(strs(&["Obsidian.lnk", "f1", "Code.lnk"]), specs.iter().map(|s| s.id.clone()).collect::<Vec<_>>());
}

#[test]
fn items_eslitligi_icerige_gore_ve_sira_duyarlidir() {
    let list = |kinds: &[&str]| -> DockItemsSettings {
        DockItemsSettings {
            items: ValueList::from(kinds.iter().map(|k| DockItemEntry::new(*k, "", "")).collect::<Vec<_>>()),
            ..Default::default()
        }
    };
    let a = list(&["spacer", "clock"]);
    let same = list(&["spacer", "clock"]);
    let swapped = list(&["clock", "spacer"]);

    assert_eq!(a, same);
    assert_ne!(a, swapped);
}

#[test]
fn varsayilan_ayarlarda_items_bos_ve_pinned_order_degismemistir() {
    let settings = AppSettings::default();

    assert!(settings.dock_items.items.is_empty());
    assert!(settings.dock_items.pinned_order.is_empty());
}

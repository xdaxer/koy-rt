//! `AppSettingsEqualityTests.cs`: ayar bölümleri kayıttır; eşitlik alanlarından gelir. Bu testler her bölümdeki her alanı
//! tek tek değiştirir ve eşitliğin bunu gördüğünü doğrular. (GetHashCode karşılaştırmaları atlandı: Rust kayıtları `Hash` uygulamaz.)

use crate::display::MonitorSettings;
use crate::dock::{DockFolder, DockItemEntry};
use crate::notch::HomeCard;
use crate::settings::*;

fn defaults() -> AppSettings {
    AppSettings::default()
}

#[test]
fn two_default_instances_are_equal() {
    assert_eq!(AppSettings::default(), AppSettings::default());
}

#[test]
fn changing_any_scalar_field_makes_settings_unequal() {
    let original = defaults();
    let mut checked = 0;
    for section in original.section_views(&original) {
        for field in &section.fields {
            // Skaler: bool, sayı, metin, enum (liste/sözlük/iç içe kayıt değil).
            if field.kind.is_none() {
                continue;
            }
            let changed = original
                .perturb(section.name, field.name)
                .unwrap_or_else(|| panic!("{}.{} değiştirilemedi", section.name, field.name));

            assert_ne!(original, changed, "{}.{}", section.name, field.name);
            checked += 1;
        }
    }
    assert!(checked > 80, "{checked}");
}

/// Her `ValueList` alanını tek örnek öğeli listeyle değiştirir (C# `ListFields` + `SampleItem`).
fn with_sample_list(section: &str, field: &str) -> AppSettings {
    let mut s = defaults();
    match (section, field) {
        ("DockItems", "PinnedOrder") => s.dock_items.pinned_order = ValueList::from(["yeni"]),
        ("DockItems", "Folders") => {
            s.dock_items.folders = ValueList::from(vec![DockFolder::new("yeni", "Yeni", vec!["x.lnk".to_string()])]);
        }
        ("DockItems", "Items") => s.dock_items.items = ValueList::from(vec![DockItemEntry { kind: "spacer".to_string(), ..Default::default() }]),
        ("Notch", "PanelOrder") => s.notch.panel_order = ValueList::from(["yeni"]),
        ("Notch", "HomeCards") => s.notch.home_cards = ValueList::from(vec![HomeCard::Timers]),
        ("Notch", "EnabledPanels") => s.notch.enabled_panels = ValueList::from(["yeni"]),
        ("Notch", "ControlCenterLayout") => s.notch.control_center_layout = ValueList::from(["yeni"]),
        ("Search", "DisabledSources") => s.search.disabled_sources = ValueList::from(["yeni"]),
        ("QuickLaunch", "Items") => s.quick_launch.items = ValueList::from(["yeni"]),
        _ => panic!("örnek öğe tanımlı değil: {section}.{field}"),
    }
    s
}

#[test]
fn changing_any_list_field_makes_settings_unequal() {
    let list_fields = [
        ("DockItems", "PinnedOrder"),
        ("DockItems", "Folders"),
        ("DockItems", "Items"),
        ("Notch", "PanelOrder"),
        ("Notch", "HomeCards"),
        ("Notch", "EnabledPanels"),
        ("Notch", "ControlCenterLayout"),
        ("Search", "DisabledSources"),
        ("QuickLaunch", "Items"),
    ];
    for (section, field) in list_fields {
        assert_ne!(defaults(), with_sample_list(section, field), "{section}.{field}");
    }
}

#[test]
fn every_value_list_field_of_the_model_is_covered_by_the_list_test() {
    // Yeni bir liste alanı eklenirse (alan tablosunda skaler olmayan ve sözlük olmayan) bu testin tablosuna da girmeli.
    let d = defaults();
    let mut count = 0;
    for section in d.section_views(&d) {
        for field in &section.fields {
            if field.kind.is_none() && field.is_string_list {
                count += 1;
            }
        }
    }
    // Metin listeleri: PinnedOrder, PanelOrder, EnabledPanels, ControlCenterLayout, DisabledSources, QuickLaunch.Items.
    assert_eq!(6, count);
}

#[test]
fn same_list_content_in_another_instance_is_equal() {
    let a = AppSettings { quick_launch: QuickLaunchSettings { items: ValueList::from(["a.exe", "docs"]) }, ..Default::default() };
    let b = AppSettings { quick_launch: QuickLaunchSettings { items: ValueList::from(["a.exe", "docs"]) }, ..Default::default() };

    assert_eq!(a, b);
}

#[test]
fn list_order_matters() {
    let a = AppSettings {
        dock_items: DockItemsSettings { pinned_order: ValueList::from(["a.lnk", "b.lnk"]), ..Default::default() },
        ..Default::default()
    };
    let b = AppSettings {
        dock_items: DockItemsSettings { pinned_order: ValueList::from(["b.lnk", "a.lnk"]), ..Default::default() },
        ..Default::default()
    };

    assert_ne!(a, b);
}

fn monitors(key: &str, dock_enabled: bool) -> AppSettings {
    let mut monitors = ValueDictionary::new();
    monitors.insert(key, MonitorSettings { dock_enabled, ..Default::default() });
    AppSettings { displays: DisplaysSettings { monitors, ..Default::default() }, ..Default::default() }
}

#[test]
fn changing_monitor_settings_makes_settings_unequal() {
    let changed = monitors("a", false);
    let other = monitors("a", true);

    assert_ne!(AppSettings::default(), changed);
    assert_ne!(changed, other);
    // Monitör kimliği büyük ve küçük harf duyarsız: aynı ekran tek kayıt sayılır.
    assert_eq!(changed, monitors("A", false));
}

fn folders(id: &str, name: &str, items: &[&str]) -> AppSettings {
    AppSettings {
        dock_items: DockItemsSettings {
            folders: ValueList::from(vec![DockFolder::new(id, name, items.iter().map(|s| s.to_string()).collect())]),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn changing_dock_folders_makes_settings_unequal() {
    let original = AppSettings::default();
    let changed = folders("f1", "Araçlar", &["calc.lnk", "notepad.lnk"]);

    assert_ne!(original, changed);
    assert_eq!(changed, folders("f1", "Araçlar", &["calc.lnk", "notepad.lnk"]));
    assert_ne!(changed, folders("f1", "Farklı", &["calc.lnk", "notepad.lnk"]));
    assert_ne!(changed, folders("f1", "Araçlar", &["calc.lnk"]));
}

#[test]
fn changing_the_schema_version_makes_settings_unequal() {
    assert_ne!(AppSettings::default(), AppSettings { version: AppSettings::CURRENT_VERSION - 1, ..Default::default() });
}

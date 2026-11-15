use super::support::s;
use crate::notch::ControlCenterTiles;
use crate::settings::{AppSettings, NotchSettings, SettingsStore, ValueList};

#[test]
fn default_settings_resolve_to_the_default_order() {
    assert_eq!(
        ControlCenterTiles::default_order(),
        ControlCenterTiles::resolve(Some(&AppSettings::default().notch.control_center_layout.iter().cloned().collect::<Vec<String>>()))
    );
}

#[test]
fn saved_order_is_respected() {
    assert_eq!(
        s(&["donotdisturb", "wifi", "bluetooth"]),
        ControlCenterTiles::resolve(Some(&s(&["donotdisturb", "wifi", "bluetooth"])))
    );
}

#[test]
fn missing_tiles_are_appended_in_default_order() {
    assert_eq!(s(&["bluetooth", "wifi", "donotdisturb"]), ControlCenterTiles::resolve(Some(&s(&["bluetooth"]))));
}

#[test]
fn unknown_duplicate_blank_and_null_entries_are_dropped_and_case_is_ignored() {
    // C#'taki `null!` öğesi Rust'ta `String` olamaz; boş metinle aynı yoldan elenir.
    let resolved = ControlCenterTiles::resolve(Some(&s(&["WiFi", "wifi", "ses", "", "", " Bluetooth "])));

    assert_eq!(s(&["wifi", "bluetooth", "donotdisturb"]), resolved);
}

#[test]
fn null_or_empty_layout_gives_the_default_order() {
    assert_eq!(ControlCenterTiles::default_order(), ControlCenterTiles::resolve(None));
    assert_eq!(ControlCenterTiles::default_order(), ControlCenterTiles::resolve(Some(&[])));
}

#[test]
fn move_places_the_tile_at_the_target_slot() {
    let rows: [(&str, i32, [&str; 3]); 5] = [
        ("wifi", 2, ["bluetooth", "donotdisturb", "wifi"]),
        ("donotdisturb", 0, ["donotdisturb", "wifi", "bluetooth"]),
        ("bluetooth", 1, ["wifi", "bluetooth", "donotdisturb"]),
        ("wifi", 99, ["bluetooth", "donotdisturb", "wifi"]),
        ("wifi", -4, ["wifi", "bluetooth", "donotdisturb"]),
    ];
    for (id, to, expected) in rows {
        assert_eq!(s(&expected), ControlCenterTiles::r#move(&ControlCenterTiles::default_order(), id, to), "{id} {to}");
    }
}

#[test]
fn move_does_not_change_the_input_and_ignores_unknown_ids() {
    let original = ControlCenterTiles::default_order();

    let moved = ControlCenterTiles::r#move(&original, "wifi", 2);
    let same = ControlCenterTiles::r#move(&original, "yok", 1);

    assert_eq!(ControlCenterTiles::default_order(), original);
    assert_ne!(original, moved);
    assert_eq!(original, same);
}

#[test]
fn slot_at_maps_the_drag_position_to_a_slot() {
    // Üç yuva, her biri 116 geniş, 8 boşluk: sınırlar 124 ve 248.
    let rows = [(-30.0, 0), (10.0, 0), (116.0, 0), (130.0, 1), (260.0, 2), (9999.0, 2)];
    for (center_x, expected) in rows {
        assert_eq!(expected, ControlCenterTiles::slot_at(center_x, 116.0, 8.0, 3), "{center_x}");
    }
}

#[test]
fn slot_at_is_safe_for_degenerate_input() {
    assert_eq!(0, ControlCenterTiles::slot_at(50.0, 0.0, 8.0, 3));
    assert_eq!(0, ControlCenterTiles::slot_at(50.0, 100.0, 8.0, 1));
}

#[test]
fn same_order_compares_sequences() {
    assert!(ControlCenterTiles::same_order(&s(&["a", "b"]), &s(&["a", "b"])));
    assert!(!ControlCenterTiles::same_order(&s(&["a", "b"]), &s(&["b", "a"])));
}

#[test]
fn order_survives_a_settings_save_and_load() {
    let dir = std::env::temp_dir().join(format!("koy-cc-{}", std::process::id()));
    let store = SettingsStore::new(dir.join("settings.json"));
    let layout = ValueList::from(ControlCenterTiles::r#move(&ControlCenterTiles::default_order(), "donotdisturb", 0));
    let settings = AppSettings { notch: NotchSettings { control_center_layout: layout, ..NotchSettings::default() }, ..AppSettings::default() };
    store.save(&settings);

    assert_eq!(s(&["donotdisturb", "wifi", "bluetooth"]), ControlCenterTiles::resolve(Some(&store.load().notch.control_center_layout.iter().cloned().collect::<Vec<String>>())));
    let _ = std::fs::remove_dir_all(&dir);
}

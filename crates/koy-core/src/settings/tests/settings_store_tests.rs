//! `SettingsStoreTests.cs`.

use super::TempDir;
use crate::appearance::{AnimationSpeed, AppTheme, MaterialLevel};
use crate::dock::{DockIndicatorStyle, DockProximityMode, MiddleClickAction};
use crate::notch::{IdleDisplayMode, NotchOpenBehavior, WheelAction};
use crate::settings::*;
use crate::visibility::VisibilityMode;
use crate::voice::VoiceHarnessType;

fn new_store(dir: &TempDir) -> SettingsStore {
    SettingsStore::new(dir.file("settings.json"))
}

fn with_general_language(language: &str) -> GeneralSettings {
    GeneralSettings { language: language.to_string(), ..Default::default() }
}

#[test]
fn missing_file_returns_defaults() {
    let dir = TempDir::unique("koy-tests-");
    assert_eq!(AppSettings::default(), new_store(&dir).load());
}

#[test]
fn round_trips() {
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    let settings = AppSettings {
        general: with_general_language("en"),
        dock: DockSettings { visibility: VisibilityMode::Peek, ..Default::default() },
        appearance: AppearanceSettings { accent_color: Some("#FF8800".to_string()), ..Default::default() },
        ..Default::default()
    };

    store.save(&settings).unwrap();

    assert_eq!(settings, store.load());
}

#[test]
fn partial_file_keeps_defaults_for_missing_fields() {
    // Eski ya da elle yazılmış dosyada olmayan alanlar varsayılanını korumalı (NotchEnabled false olursa çentik hiç açılmaz).
    let dir = TempDir::new("koy-tests-");
    std::fs::write(dir.file("settings.json"), r#"{ "DockEnabled": true }"#).unwrap();

    let loaded = new_store(&dir).load();

    assert_eq!(AppSettings { dock: DockSettings { enabled: true, ..Default::default() }, ..Default::default() }, loaded);
}

#[test]
fn corrupt_file_returns_defaults() {
    let dir = TempDir::new("koy-tests-");
    let store = new_store(&dir);
    std::fs::write(store.path(), "{ bozuk").unwrap();

    assert_eq!(AppSettings::default(), store.load());
}

#[test]
fn enums_are_written_as_text() {
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    store.save(&AppSettings { dock: DockSettings { visibility: VisibilityMode::Smart, ..Default::default() }, ..Default::default() }).unwrap();

    assert!(std::fs::read_to_string(store.path()).unwrap().contains("\"Smart\""));
}

#[test]
fn round_trips_with_custom_panels() {
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    let settings = AppSettings {
        general: with_general_language("en"),
        notch: NotchSettings {
            enabled_panels: ValueList::from(["music", "calendar"]),
            control_center_layout: ValueList::from(["bluetooth", "wifi"]),
            ..Default::default()
        },
        ..Default::default()
    };

    store.save(&settings).unwrap();
    let loaded = store.load();

    assert_eq!(settings, loaded);
    assert_eq!(["music", "calendar"], loaded.notch.enabled_panels);
}

#[test]
fn round_trips_with_dock_and_monitor_settings() {
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    let settings = AppSettings {
        dock: DockSettings {
            enabled: true,
            visibility: VisibilityMode::Fixed,
            hover_magnify: false,
            hide_in_fullscreen: false,
            ..Default::default()
        },
        ..Default::default()
    };

    store.save(&settings).unwrap();
    let loaded = store.load();

    assert_eq!(settings, loaded);
    assert!(loaded.dock.enabled);
    assert_eq!(VisibilityMode::Fixed, loaded.dock.visibility);
    assert!(!loaded.dock.hover_magnify);
    assert!(!loaded.dock.hide_in_fullscreen);
}

#[test]
fn round_trips_with_first_run_completed_and_theme() {
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    let settings = AppSettings {
        general: GeneralSettings { first_run_completed: true, ..Default::default() },
        appearance: AppearanceSettings {
            theme: AppTheme::Light,
            corner_radius: 14.0,
            animation_speed: AnimationSpeed::Fast,
            ..Default::default()
        },
        ..Default::default()
    };

    store.save(&settings).unwrap();
    let loaded = store.load();

    assert_eq!(settings, loaded);
    assert!(loaded.general.first_run_completed);
    assert_eq!(AppTheme::Light, loaded.appearance.theme);
    assert_eq!(14.0, loaded.appearance.corner_radius);
}

#[test]
fn round_trips_with_new_ui_settings() {
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    let settings = AppSettings {
        dock: DockSettings {
            icon_size: 24.0,
            middle_click_action: MiddleClickAction::Close,
            indicator_style: DockIndicatorStyle::Line,
            ..Default::default()
        },
        notch: NotchSettings {
            idle_display: IdleDisplayMode::ClockOnly,
            open_behavior: NotchOpenBehavior::Hover,
            wheel_action: WheelAction::Volume,
            ..Default::default()
        },
        appearance: AppearanceSettings { material: MaterialLevel::Frosted, ..Default::default() },
        ..Default::default()
    };

    store.save(&settings).unwrap();
    let loaded = store.load();

    assert_eq!(settings, loaded);
    assert_eq!(24.0, loaded.dock.icon_size);
    assert_eq!(IdleDisplayMode::ClockOnly, loaded.notch.idle_display);
    assert_eq!(NotchOpenBehavior::Hover, loaded.notch.open_behavior);
    assert_eq!(MaterialLevel::Frosted, loaded.appearance.material);
    assert_eq!(WheelAction::Volume, loaded.notch.wheel_action);
    assert_eq!(MiddleClickAction::Close, loaded.dock.middle_click_action);
    assert_eq!(DockIndicatorStyle::Line, loaded.dock.indicator_style);
}

#[test]
fn default_dock_proximity_is_reserve_space() {
    let dir = TempDir::unique("koy-tests-");
    assert_eq!(DockProximityMode::ReserveSpace, AppSettings::default().dock.proximity);
    assert_eq!(DockProximityMode::ReserveSpace, new_store(&dir).load().dock.proximity);
}

#[test]
fn dock_proximity_missing_in_existing_file_gets_new_default() {
    let dir = TempDir::new("koy-tests-");
    let store = new_store(&dir);
    std::fs::write(store.path(), r#"{ "DockEnabled": true }"#).unwrap();

    assert_eq!(DockProximityMode::ReserveSpace, store.load().dock.proximity);
}

#[test]
fn dock_down_is_kept_on_load_and_file_is_untouched() {
    let dir = TempDir::new("koy-tests-");
    let store = new_store(&dir);
    let json = r#"{ "DockProximity": "DockDown" }"#;
    std::fs::write(store.path(), json).unwrap();

    let loaded = store.load();

    assert_eq!(DockProximityMode::DockDown, loaded.dock.proximity);
    assert_eq!(json, std::fs::read_to_string(store.path()).unwrap());
}

#[test]
fn dodge_stays_selectable() {
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    store.save(&AppSettings { dock: DockSettings { proximity: DockProximityMode::Dodge, ..Default::default() }, ..Default::default() }).unwrap();

    assert_eq!(DockProximityMode::Dodge, store.load().dock.proximity);
}

#[test]
fn always_floating_round_trips_and_is_written_by_name() {
    // Enum ADIYLA yazılır (UseStringEnumConverter): yeni değerin sayısı eski dosyaları etkilemez.
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    store
        .save(&AppSettings { dock: DockSettings { proximity: DockProximityMode::AlwaysFloating, ..Default::default() }, ..Default::default() })
        .unwrap();

    assert!(std::fs::read_to_string(store.path()).unwrap().contains("\"Proximity\": \"AlwaysFloating\""));
    assert_eq!(DockProximityMode::AlwaysFloating, store.load().dock.proximity);
}

#[test]
fn existing_proximity_names_load_unchanged_and_file_is_untouched() {
    // Eski dosyalar göçsüz okunur: bilinen adlar aynı değere döner, dosya yeniden yazılmaz.
    let cases = [
        ("ReserveSpace", DockProximityMode::ReserveSpace),
        ("DockDown", DockProximityMode::DockDown),
        ("Dodge", DockProximityMode::Dodge),
        ("AlwaysHidden", DockProximityMode::AlwaysHidden),
        ("AlwaysFloating", DockProximityMode::AlwaysFloating),
    ];
    for (name, expected) in cases {
        let dir = TempDir::new("koy-tests-");
        let store = new_store(&dir);
        let json = format!("{{ \"DockProximity\": \"{name}\" }}");
        std::fs::write(store.path(), &json).unwrap();

        assert_eq!(expected, store.load().dock.proximity, "{name}");
        assert_eq!(json, std::fs::read_to_string(store.path()).unwrap(), "{name}");
    }
}

#[test]
fn existing_numeric_proximity_values_keep_their_meaning() {
    // Elle ya da eski sürümle sayı yazılmış olsa bile yeni değer en sonda olduğu için anlamlar kaymaz.
    assert_eq!(0, DockProximityMode::DockDown as i32);
    assert_eq!(1, DockProximityMode::ReserveSpace as i32);
    assert_eq!(2, DockProximityMode::Dodge as i32);
    assert_eq!(3, DockProximityMode::AlwaysHidden as i32);
    assert_eq!(4, DockProximityMode::AlwaysFloating as i32);
}

#[test]
fn dock_show_tray_defaults_on_for_old_files_and_round_trips_off() {
    // Tepsi alanı olmayan eski dosyada tepsi açık gelmeli; kapatılınca kapalı kalmalı.
    let dir = TempDir::new("koy-tests-");
    let store = new_store(&dir);
    std::fs::write(store.path(), r#"{ "DockEnabled": true }"#).unwrap();
    assert!(store.load().dock.show_tray);

    store.save(&AppSettings { dock: DockSettings { show_tray: false, ..Default::default() }, ..Default::default() }).unwrap();
    let loaded = store.load();

    assert!(!loaded.dock.show_tray);
    assert_ne!(AppSettings::default(), loaded);
}

#[test]
fn voice_settings_round_trips_with_json_context() {
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    let original = AppSettings {
        shortcuts: ShortcutsSettings { voice_command: "Ctrl+Shift+V".to_string(), ..Default::default() },
        voice: VoiceSettings {
            harness: VoiceHarnessType::Codex,
            custom_command: "my-ai {prompt}".to_string(),
            working_directory: r"C:\Workspace".to_string(),
            timeout_seconds: 120,
            ..Default::default()
        },
        ..Default::default()
    };

    store.save(&original).unwrap();
    let loaded = store.load();

    assert_eq!(original, loaded);
    assert_eq!("Ctrl+Shift+V", loaded.shortcuts.voice_command);
    assert_eq!(VoiceHarnessType::Codex, loaded.voice.harness);
    assert_eq!("my-ai {prompt}", loaded.voice.custom_command);
    assert_eq!(r"C:\Workspace", loaded.voice.working_directory);
    assert_eq!(120, loaded.voice.timeout_seconds);
}

// ---- Rust'a özgü: JSON biçimi C# ile birebir (PascalCase, girintili, \uXXXX kaçışı) ----

#[test]
fn saved_file_uses_pascal_case_names_and_indentation_like_system_text_json() {
    let dir = TempDir::unique("koy-tests-");
    let store = new_store(&dir);
    store
        .save(&AppSettings {
            weather: WeatherSettings { city: "İstanbul + <x>".to_string(), latitude: 12.0, ..Default::default() },
            ..Default::default()
        })
        .unwrap();

    let text = std::fs::read_to_string(store.path()).unwrap();
    let nl = if cfg!(windows) { "\r\n" } else { "\n" };
    assert!(text.starts_with(&format!("{{{nl}  \"Version\": 4,{nl}  \"General\": {{{nl}")));
    // .NET varsayılan kodlayıcısı: ASCII dışı ve HTML duyarlı karakterler \uXXXX (büyük hex); tam sayı değerli double "12".
    assert!(text.contains(r#""City": "\u0130stanbul \u002B \u003Cx\u003E""#), "{text}");
    assert!(text.contains("\"Latitude\": 12,"), "{text}");
    assert!(text.contains("\"AccentColor\": null"), "{text}");
    assert!(text.contains("\"EnabledPanels\": ["), "{text}");
}

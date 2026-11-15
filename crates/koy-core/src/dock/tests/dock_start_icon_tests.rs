use super::TempDir;
use crate::dock::{DockSettingsDiff, DockStartIcon, DockStartIconResolver, DockStartIconSource};
use crate::settings::{AppSettings, DockSettings, SettingsStore};

fn koy() -> DockStartIconSource {
    DockStartIconSource::new(DockStartIcon::Koy, None)
}

fn dock(f: impl FnOnce(&mut DockSettings)) -> AppSettings {
    let mut s = AppSettings::default();
    f(&mut s.dock);
    s
}

#[test]
fn default_is_the_koy_icon_with_no_custom_path() {
    let s = AppSettings::default();

    assert_eq!(DockStartIcon::Koy, s.dock.start_icon);
    assert_eq!("", s.dock.start_icon_path);
    assert_eq!(koy(), DockStartIconResolver::resolve_settings(&s, None));
}

#[test]
fn old_settings_file_without_the_fields_gets_the_koy_icon() {
    let dir = TempDir::new("koy-tests-");
    std::fs::write(dir.settings_path(), r#"{ "DockStyle": "FullWidth", "DockShowStartButton": true }"#).unwrap();

    let loaded = SettingsStore::new(dir.settings_path()).load();

    assert_eq!(DockStartIcon::Koy, loaded.dock.start_icon);
    assert_eq!("", loaded.dock.start_icon_path);
}

#[test]
fn round_trips_by_name_with_path() {
    for (icon, name) in [(DockStartIcon::Windows, "Windows"), (DockStartIcon::Custom, "Custom")] {
        let dir = TempDir::new("koy-tests-");
        let store = SettingsStore::new(dir.settings_path());

        store
            .save(&dock(|d| {
                d.start_icon = icon;
                d.start_icon_path = r"C:\icons\baslat.png".to_string();
            }))
            .unwrap();

        let text = std::fs::read_to_string(dir.settings_path()).unwrap();
        assert!(text.contains(&format!("\"StartIcon\": \"{name}\"")), "{name}: {text}");
        let loaded = store.load();
        assert_eq!(icon, loaded.dock.start_icon);
        assert_eq!(r"C:\icons\baslat.png", loaded.dock.start_icon_path);
    }
}

#[test]
fn icon_and_path_take_part_in_equality() {
    assert_ne!(AppSettings::default(), dock(|d| d.start_icon = DockStartIcon::Windows));
    assert_ne!(AppSettings::default(), dock(|d| d.start_icon_path = r"C:\a.png".to_string()));
}

#[test]
fn windows_choice_ignores_the_path() {
    let always = |_: &str| true;
    assert_eq!(
        DockStartIconSource::new(DockStartIcon::Windows, None),
        DockStartIconResolver::resolve(DockStartIcon::Windows, Some(r"C:\a.png"), Some(&always))
    );
}

#[test]
fn koy_choice_ignores_the_path() {
    let always = |_: &str| true;
    assert_eq!(koy(), DockStartIconResolver::resolve(DockStartIcon::Koy, Some(r"C:\a.png"), Some(&always)));
}

#[test]
fn custom_with_an_existing_supported_file_uses_it() {
    let always = |_: &str| true;
    for path in [r"C:\icons\a.png", r"C:\icons\a.JPG", r"C:\icons\a.jpeg", r"C:\icons\a.ico"] {
        assert_eq!(
            DockStartIconSource::new(DockStartIcon::Custom, Some(path.to_string())),
            DockStartIconResolver::resolve(DockStartIcon::Custom, Some(path), Some(&always)),
            "{path}"
        );
    }
}

#[test]
fn custom_with_an_empty_path_falls_back_to_koy() {
    let always = |_: &str| true;
    for path in [None, Some(""), Some("   ")] {
        assert_eq!(koy(), DockStartIconResolver::resolve(DockStartIcon::Custom, path, Some(&always)), "{path:?}");
    }
}

#[test]
fn custom_with_a_missing_file_falls_back_to_koy() {
    let never = |_: &str| false;
    assert_eq!(koy(), DockStartIconResolver::resolve(DockStartIcon::Custom, Some(r"C:\icons\gone.png"), Some(&never)));
}

#[test]
fn custom_with_an_unsupported_extension_falls_back_to_koy() {
    let always = |_: &str| true;
    for path in [r"C:\icons\a.svg", r"C:\icons\a.exe", r"C:\icons\noextension"] {
        assert_eq!(koy(), DockStartIconResolver::resolve(DockStartIcon::Custom, Some(path), Some(&always)), "{path}");
    }
}

#[test]
#[ignore = "C# testi dosya denetiminin IOException fırlatmasını sınar; Rust'ta denetim geri çağrısı `bool` döner, fırlatma yolu yok (File.Exists zaten atmaz)"]
fn a_throwing_file_check_falls_back_to_koy() {}

#[test]
fn real_file_system_check_finds_an_existing_file() {
    let dir = TempDir::new("koy-tests-");
    let file = dir.path().join("a.png");
    std::fs::write(&file, [1u8]).unwrap();
    let missing = dir.path().join("b.png");

    assert_eq!(DockStartIcon::Custom, DockStartIconResolver::resolve(DockStartIcon::Custom, file.to_str(), None).kind);
    assert_eq!(DockStartIcon::Koy, DockStartIconResolver::resolve(DockStartIcon::Custom, missing.to_str(), None).kind);
}

#[test]
fn changing_the_icon_only_rebuilds_view_models() {
    let variants = [
        dock(|d| d.start_icon = DockStartIcon::Windows),
        dock(|d| {
            d.start_icon = DockStartIcon::Custom;
            d.start_icon_path = r"C:\a.png".to_string();
        }),
        dock(|d| d.start_icon_path = r"C:\a.png".to_string()),
    ];

    for after in &variants {
        let d = DockSettingsDiff::compare(&AppSettings::default(), after);

        assert!(d.rebuild_view_models);
        assert!(!d.reconcile_app_bar);
        assert!(!d.reconcile_taskbar);
        assert!(!d.reconcile_monitors);
        assert!(!d.reconcile_tray);
        assert!(!d.reconcile_power);
    }
}

use crate::display::*;
use crate::settings::ValueDictionary;
use crate::shell::WindowRect;

const ID_MAIN: &str = r"\\?\DISPLAY#GSM5B09#5&1a2b&0&UID4352#{e6f07b5f}";
const ID_SIDE: &str = r"\\?\DISPLAY#DEL4161#5&1a2b&0&UID4353#{e6f07b5f}";

fn main_mon(device_name: &str, width: i32) -> DisplayMonitor {
    DisplayMonitor::new(ID_MAIN, "LG ULTRAGEAR", WindowRect::new(0, 0, width, 1080), WindowRect::new(0, 0, width, 1040), true)
        .with_dpi(1.0, 1.0)
        .with_handle(1)
        .with_device_name(device_name)
}

fn main_default() -> DisplayMonitor {
    main_mon(r"\\.\DISPLAY1", 1920)
}

fn side(device_name: &str) -> DisplayMonitor {
    DisplayMonitor::new(ID_SIDE, "DELL U2415", WindowRect::new(1920, 0, 3840, 1200), WindowRect::new(1920, 0, 3840, 1160), false)
        .with_dpi(1.25, 1.25)
        .with_handle(2)
        .with_device_name(device_name)
}

fn side_default() -> DisplayMonitor {
    side(r"\\.\DISPLAY2")
}

fn settings(notch: bool, dock: bool) -> MonitorSettings {
    MonitorSettings { notch_enabled: notch, dock_enabled: dock, name: None }
}

fn dict(entries: Vec<(&str, MonitorSettings)>) -> ValueDictionary<MonitorSettings> {
    let mut d = ValueDictionary::new();
    for (k, v) in entries {
        d.insert(k, v);
    }
    d
}

#[test]
fn unknown_monitor_gets_new_screen_default() {
    let result = MonitorPolicy::resolve(None, NewScreenMode::NotchOnly, &[main_default(), side_default()]);

    assert!(result.iter().all(|a| !a.is_known));
    assert!(result.iter().all(|a| a.notch_enabled));
    assert!(result.iter().all(|a| !a.dock_enabled));
}

#[test]
fn new_screen_default_maps_to_notch_and_dock() {
    let rows = [(NewScreenMode::NotchAndDock, true, true), (NewScreenMode::NotchOnly, true, false), (NewScreenMode::None, false, false)];
    for (mode, notch, dock) in rows {
        let s = MonitorPolicy::default_for(mode);
        assert_eq!(notch, s.notch_enabled, "{mode:?}");
        assert_eq!(dock, s.dock_enabled, "{mode:?}");
    }
}

#[test]
fn saved_settings_apply_per_monitor() {
    let saved = dict(vec![(ID_MAIN, settings(true, true)), (ID_SIDE, settings(true, false))]);

    let result = MonitorPolicy::resolve(Some(&saved), NewScreenMode::None, &[main_default(), side_default()]);

    assert!(result[0].dock_enabled);
    assert!(result[1].notch_enabled);
    assert!(!result[1].dock_enabled);
    assert!(result.iter().all(|a| a.is_known));
    let dock_ids: Vec<String> = MonitorPolicy::dock_monitors(&result).into_iter().map(|m| m.id).collect();
    assert_eq!(vec![main_default().id], dock_ids);
    assert_eq!(2, MonitorPolicy::notch_monitors(&result).len());
}

#[test]
fn identity_survives_resolution_position_and_device_name_changes() {
    let saved = dict(vec![(ID_SIDE, settings(false, true))]);

    // Aygıt adı DISPLAY2 -> DISPLAY3, çözünürlük ve konum da değişti: ayar yine bu ekrana gider.
    let moved = DisplayMonitor { bounds: WindowRect::new(-2560, 0, 0, 1440), ..side(r"\\.\DISPLAY3") };
    let result = MonitorPolicy::resolve(Some(&saved), NewScreenMode::NotchAndDock, &[main_default(), moved]);

    assert!(!result[1].notch_enabled);
    assert!(result[1].is_known);
    assert!(result[0].notch_enabled);
    assert!(!result[0].is_known);
}

#[test]
fn legacy_device_name_key_matches_until_rewritten_with_stable_id() {
    let saved = dict(vec![(r"\\.\display2", settings(true, false))]);

    let result = MonitorPolicy::resolve(Some(&saved), NewScreenMode::None, &[main_default(), side_default()]);
    assert!(result[1].is_known);
    assert!(!result[1].dock_enabled);

    let updated = MonitorPolicy::update(Some(&saved), NewScreenMode::None, &side_default(), |s| MonitorSettings { dock_enabled: true, ..s });
    assert!(!updated.contains_key(r"\\.\display2"));
    assert!(updated.get(ID_SIDE).unwrap().dock_enabled);
    assert!(updated.get(ID_SIDE).unwrap().notch_enabled);
    assert_eq!(Some("DELL U2415".to_string()), updated.get(ID_SIDE).unwrap().name);
}

#[test]
fn update_keeps_settings_of_disconnected_monitors_and_does_not_mutate_input() {
    let saved = dict(vec![(
        "ayrilmis-ekran",
        MonitorSettings { notch_enabled: false, dock_enabled: true, name: Some("Eski TV".to_string()) },
    )]);

    let updated = MonitorPolicy::update(Some(&saved), NewScreenMode::NotchOnly, &main_default(), |s| MonitorSettings { notch_enabled: false, ..s });

    assert_eq!(1, saved.len());
    assert_eq!(2, updated.len());
    assert_eq!(Some("Eski TV".to_string()), updated.get("ayrilmis-ekran").unwrap().name);
    // Kayıtsız ekranın düzenlemesi yeni ekran varsayılanından (yalnız çentik) başlar.
    assert!(!updated.get(ID_MAIN).unwrap().notch_enabled);
    assert!(!updated.get(ID_MAIN).unwrap().dock_enabled);
}

#[test]
fn same_model_on_two_ports_is_two_different_monitors() {
    let twin = DisplayMonitor {
        id: r"\\?\DISPLAY#GSM5B09#5&1a2b&0&UID4999#{e6f07b5f}".to_string(),
        name: "LG ULTRAGEAR".to_string(),
        ..side_default()
    };
    let saved = dict(vec![(ID_MAIN, MonitorSettings { dock_enabled: false, ..MonitorSettings::default() })]);

    let result = MonitorPolicy::resolve(Some(&saved), NewScreenMode::NotchAndDock, &[main_default(), twin]);

    assert!(!result[0].dock_enabled);
    assert!(result[1].dock_enabled);
}

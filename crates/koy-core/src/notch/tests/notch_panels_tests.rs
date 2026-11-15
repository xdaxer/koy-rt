use std::sync::Arc;

use super::support::s;
use crate::notch::{NotchController, NotchPanel, NotchPanels, NotchState};
use crate::settings::{AppSettings, SettingsStore};
use crate::time::testing::StepTimeProvider;
use crate::time::{TimeProvider, TimeSpan};

use NotchPanel::{Calendar, Clipboard, ControlCenter, Focus, Home, Shelf, System};

fn parse(keys: &[&str]) -> Vec<NotchPanel> {
    NotchPanels::parse_enabled(Some(&s(keys)), true)
}

fn keys_of(settings_keys: &[String]) -> Vec<String> {
    settings_keys.to_vec()
}

#[test]
fn default_settings_enable_all_panels_in_tab_order() {
    let keys = keys_of(&AppSettings::default().notch.enabled_panels.iter().cloned().collect::<Vec<String>>());
    assert_eq!(NotchPanels::order().to_vec(), NotchPanels::parse_enabled(Some(&keys), true));
}

#[test]
fn parse_enabled_always_includes_home() {
    assert_eq!(vec![Home], NotchPanels::parse_enabled(None, true));
    assert_eq!(vec![Home], NotchPanels::parse_enabled(Some(&[]), true));
    assert_eq!(vec![Home, Shelf], parse(&["shelf"]));
}

#[test]
fn parse_enabled_ignores_order_in_settings() {
    assert_eq!(vec![Home, Focus, Shelf, ControlCenter], parse(&["controlcenter", "shelf", "focus", "home"]));
}

#[test]
fn parse_enabled_migrates_old_keys() {
    // Eski varsayılan: music, controlcenter, status, calendar, shelf. H öncesinde olmayan Pano ve Sistem açık gelir.
    assert_eq!(
        vec![Home, Focus, Calendar, Shelf, Clipboard, System, ControlCenter],
        parse(&["music", "controlcenter", "status", "calendar", "shelf"])
    );
}

#[test]
fn parse_enabled_calendar_key_goes_to_calendar_and_only_legacy_files_also_get_focus() {
    // H dosyası: "calendar" yalnız Takvim sekmesini açar, Odak kendiliğinden gelmez.
    assert_eq!(vec![Home, Calendar], parse(&["home", "calendar"]));
    // H öncesi dosya ("music"/"status" var): eski takvim paneli Pomodoro da taşıdığı için Odak da açılır.
    assert_eq!(vec![Home, Focus, Calendar, Clipboard, System], parse(&["music", "calendar"]));
    assert_eq!(vec![Home, Clipboard, System], parse(&["music", "status"]));
}

#[test]
fn parse_enabled_h_files_without_new_keys_do_not_get_new_tabs() {
    // H anahtarlarıyla yazılmış dosyada Pano / Sistem yoksa kullanıcı kapatmıştır, açılmaz.
    let panels = parse(&["home", "focus", "shelf", "controlcenter"]);
    assert!(!panels.contains(&Clipboard));
    assert!(!panels.contains(&System));
}

#[test]
fn parse_enabled_reads_clipboard_and_system_keys_in_tab_order() {
    assert_eq!(vec![Home, Clipboard, System, ControlCenter], parse(&["controlcenter", "system", "clipboard"]));
}

#[test]
fn parse_enabled_clipboard_history_off_hides_clipboard_tab_only() {
    assert_eq!(vec![Home, System], NotchPanels::parse_enabled(Some(&s(&["clipboard", "system"])), false));
    assert_eq!(vec![Home, Clipboard, System], NotchPanels::parse_enabled(Some(&s(&["clipboard", "system"])), true));
}

#[test]
fn default_settings_include_clipboard_and_system() {
    let defaults = AppSettings::default();
    let keys: Vec<String> = defaults.notch.enabled_panels.iter().cloned().collect();
    assert!(keys.contains(&"clipboard".to_string()));
    assert!(keys.contains(&"system".to_string()));
    assert!(defaults.clipboard.history_enabled);
}

#[test]
fn settings_file_without_new_panel_keys_loads_and_keeps_its_list() {
    let dir = std::env::temp_dir().join(format!("koy-panels-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(&path, r#"{ "EnabledPanels": ["home", "focus", "shelf", "controlcenter"] }"#).unwrap();

    let loaded = SettingsStore::new(path).load();

    let keys: Vec<String> = loaded.notch.enabled_panels.iter().cloned().collect();
    assert_eq!(s(&["home", "focus", "shelf", "controlcenter"]), keys);
    assert!(loaded.clipboard.history_enabled);
    assert_eq!(vec![Home, Focus, Shelf, ControlCenter], NotchPanels::parse_enabled(Some(&keys), loaded.clipboard.history_enabled));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parse_enabled_legacy_file_new_tabs_respect_clipboard_setting() {
    // H öncesi dosyada Pano açık gelir ama pano geçmişi kapalıysa yine gizlenir.
    let panels = NotchPanels::parse_enabled(Some(&s(&["music", "status"])), false);
    assert!(!panels.contains(&Clipboard));
    assert!(panels.contains(&System));
}

#[test]
fn parse_enabled_skips_unknown_keys_and_ignores_case() {
    assert_eq!(vec![Home, ControlCenter], parse(&["home", "pano", "", "ControlCenter"]));
}

#[test]
fn to_key_round_trips() {
    for panel in NotchPanels::order() {
        assert_eq!(Some(*panel), NotchPanels::try_parse(Some(NotchPanels::to_key(*panel))));
    }
}

#[test]
fn expand_to_disabled_panel_keeps_tab_order() {
    let controller = NotchController::new(None);
    controller.set_enabled_panels(&[Home, ControlCenter]);

    controller.expand(Some(Shelf));

    assert_eq!(vec![Home, Shelf, ControlCenter], controller.enabled_panels());
}

// ---- NotchPinTests ----

fn step_controller() -> (Arc<StepTimeProvider>, NotchController) {
    let time = Arc::new(StepTimeProvider::new());
    let controller = NotchController::new(Some(Arc::clone(&time) as Arc<dyn TimeProvider>));
    (time, controller)
}

#[test]
fn pinned_expanded_does_not_collapse_on_mouse_leave() {
    let (time, controller) = step_controller();
    controller.expand(None);
    controller.on_mouse_enter();
    controller.set_pinned(true);

    controller.on_mouse_leave();
    time.advance(TimeSpan::from_secs(2));

    assert_eq!(NotchState::Expanded, controller.state());
    assert!(controller.is_pinned());
}

#[test]
fn unpinning_with_mouse_outside_collapses_after_the_usual_delay() {
    let (time, controller) = step_controller();
    controller.expand(None);
    controller.set_pinned(true);
    controller.on_mouse_leave();

    controller.set_pinned(false);
    time.advance(TimeSpan::from_millis(500));

    assert_eq!(NotchState::Collapsed, controller.state());
}

#[test]
fn collapsing_clears_pin() {
    let controller = NotchController::new(None);
    controller.expand(None);
    controller.set_pinned(true);

    controller.on_click(); // Expanded -> Collapsed

    assert_eq!(NotchState::Collapsed, controller.state());
    assert!(!controller.is_pinned());
}

#[test]
fn cannot_pin_when_not_expanded() {
    let controller = NotchController::new(None);

    controller.toggle_pinned();

    assert!(!controller.is_pinned());
}

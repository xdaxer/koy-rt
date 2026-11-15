//! `Settings/GoldenV3ToV4Tests.cs`: v0.2.6'nın yazdığı v3 dosyasındaki HER alan, v4'te beklenen bölüm ve alana gelmeli.
//! Bir alan göçte kaybolursa ya da yanlış bölüme düşerse bu test kırılır.

use super::{FIXTURE_V3, load_json};
use crate::appearance::{AnimationSpeed, AppTheme, MaterialLevel};
use crate::display::NewScreenMode;
use crate::dock::{
    DockFolder, DockIconAlignment, DockIndicatorStyle, DockProximityMode, DockStartIcon, DockStyleMode, MiddleClickAction,
};
use crate::input::CornerAction;
use crate::media::MusicCardStyle;
use crate::notch::{IdleDisplayMode, NotchOpenBehavior, NotchStyleMode, WheelAction};
use crate::search::SearchHistoryMode;
use crate::settings::*;
use crate::visibility::VisibilityMode;
use crate::voice::{VoiceClaudePermission, VoiceHarnessType};
use crate::weather::{WeatherLocationSource, WeatherRefreshInterval};

fn s() -> AppSettings {
    load_json(FIXTURE_V3)
}

#[test]
fn version_is_upgraded_to_current() {
    assert_eq!(AppSettings::CURRENT_VERSION, s().version);
}

#[test]
fn general_section() {
    let s = s();
    assert_eq!("tr", s.general.language);
    assert!(s.general.first_run_completed);
    assert_eq!("0.2.5", s.general.last_seen_version);
    assert!(!s.general.auto_check_updates);
    assert_eq!(UpdateChannel::Preview, s.general.update_channel);
}

#[test]
fn appearance_section() {
    let s = s();
    assert_eq!(AppTheme::Light, s.appearance.theme);
    assert_eq!(Some("#FF8800".to_string()), s.appearance.accent_color);
    assert!(s.appearance.use_windows_accent);
    assert_eq!(17.0, s.appearance.corner_radius);
    assert_eq!(AnimationSpeed::Fast, s.appearance.animation_speed);
    assert!(!s.appearance.respect_reduce_motion);
    assert!(!s.appearance.battery_saver_reduce_motion);
    assert_eq!(MaterialLevel::Glass, s.appearance.material);
    assert!(!s.appearance.material_readability_guard);
}

#[test]
fn dock_section() {
    let s = s();
    assert!(!s.dock.enabled);
    assert!(!s.dock.hide_windows_taskbar);
    assert_eq!(VisibilityMode::Smart, s.dock.visibility);
    assert_eq!(DockStyleMode::FullWidth, s.dock.style);
    assert_eq!(DockIconAlignment::Right, s.dock.icon_alignment);
    assert_eq!(DockProximityMode::Dodge, s.dock.proximity);
    assert!(!s.dock.hide_in_fullscreen);
    assert!(!s.dock.edge_reveal);
    assert!(!s.dock.hover_magnify);
    assert!(!s.dock.show_start_button);
    assert_eq!(DockStartIcon::Custom, s.dock.start_icon);
    assert_eq!(r"C:\Icons\start.png", s.dock.start_icon_path);
    assert!(!s.dock.show_folders);
    assert!(!s.dock.show_recents);
    assert!(!s.dock.show_tray);
    assert!(!s.dock.small_tray);
    assert!(!s.dock.show_badges);
    assert!(!s.dock.thumbnail_preview);
    assert_eq!(650, s.dock.thumbnail_delay_ms);
    assert_eq!(275, s.dock.dodge_delay_ms);
    assert_eq!(36.0, s.dock.icon_size);
    assert_eq!(DockIndicatorStyle::Line, s.dock.indicator_style);
    assert_eq!(MiddleClickAction::Close, s.dock.middle_click_action);
}

#[test]
fn dock_items_section_gets_pinned_order_and_folders() {
    let s = s();
    assert_eq!(["Obsidian.lnk", "Firefox.lnk", "Terminal.lnk"], s.dock_items.pinned_order);
    assert_eq!(2, s.dock_items.folders.len());
    assert_eq!(
        DockFolder::new("f1a2b3c4", "Araçlar", vec!["calc.lnk".to_string(), "notepad.lnk".to_string()]),
        s.dock_items.folders[0]
    );
    assert_eq!(DockFolder::new("d5e6f7a8", "Oyunlar", vec!["steam.lnk".to_string()]), s.dock_items.folders[1]);
}

#[test]
fn notch_section() {
    let s = s();
    assert!(!s.notch.enabled);
    assert_eq!(VisibilityMode::Peek, s.notch.visibility);
    assert_eq!(NotchStyleMode::Floating, s.notch.style);
    assert_eq!(NotchOpenBehavior::Click, s.notch.open_behavior);
    assert_eq!(IdleDisplayMode::RingClockDay, s.notch.idle_display);
    assert_eq!(WheelAction::Volume, s.notch.wheel_action);
    assert!(!s.notch.hide_in_fullscreen);
    assert!(!s.notch.follows_material);
    assert!(!s.notch.show_volume_osd);
    assert!(!s.notch.show_brightness_osd);
    assert!(!s.notch.show_privacy_indicator);
    assert!(!s.notch.show_notifications);
    assert!(s.notch.notifications_hide_body);
    assert_eq!(["home", "calendar", "system"], s.notch.enabled_panels);
    assert_eq!(["bluetooth", "wifi"], s.notch.control_center_layout);
}

#[test]
fn music_section() {
    let s = s();
    assert!(!s.music.show_waveform);
    assert_eq!(MusicCardStyle::Artwork, s.music.card_style);
    assert!(s.music.lyrics_enabled);
}

#[test]
fn displays_section_keeps_monitor_records_and_default() {
    let s = s();
    assert_eq!(NewScreenMode::NotchOnly, s.displays.new_screen_default);
    assert_eq!(2, s.displays.monitors.len());

    let office = &s.displays.monitors["kimlik-1"];
    assert!(office.notch_enabled);
    assert!(!office.dock_enabled);
    assert_eq!(Some("Ofis".to_string()), office.name);

    let lounge = &s.displays.monitors["kimlik-2"];
    assert!(!lounge.notch_enabled);
    assert!(lounge.dock_enabled);
    assert_eq!(Some("Salon".to_string()), lounge.name);
}

#[test]
fn shortcuts_section() {
    let s = s();
    assert_eq!("Ctrl+Alt+1", s.shortcuts.toggle_notch);
    assert_eq!("Ctrl+Alt+2", s.shortcuts.open_shelf);
    assert_eq!("Ctrl+Alt+3", s.shortcuts.toggle_dock);
    assert_eq!("Ctrl+Alt+4", s.shortcuts.open_menu);
    assert_eq!("Ctrl+Alt+5", s.shortcuts.play_pause);
    assert_eq!("Ctrl+Alt+6", s.shortcuts.open_search);
    assert_eq!("Ctrl+Alt+7", s.shortcuts.open_search_clipboard);
    assert_eq!("Ctrl+Alt+8", s.shortcuts.voice_command);
}

#[test]
fn voice_section() {
    let s = s();
    assert_eq!(VoiceHarnessType::Codex, s.voice.harness);
    assert_eq!(VoiceClaudePermission::AcceptEdits, s.voice.claude_permission);
    assert_eq!("opus", s.voice.model);
    assert_eq!("my-ai {prompt}", s.voice.custom_command);
    assert_eq!(r"C:\Workspace", s.voice.working_directory);
    assert_eq!(120, s.voice.timeout_seconds);
}

#[test]
fn search_section() {
    let s = s();
    assert!(!s.search.retain_prior);
    assert_eq!(SearchHistoryMode::ImmediateCompletion, s.search.history_mode);
    assert_eq!(["bookmarks", "files"], s.search.disabled_sources);
    assert!(s.search.pinned);
}

#[test]
fn focus_weather_and_clipboard_sections() {
    let s = s();
    assert_eq!(40, s.focus.pomodoro_work_minutes);
    assert_eq!(7, s.focus.pomodoro_short_break_minutes);
    assert_eq!(20, s.focus.pomodoro_long_break_minutes);

    assert_eq!(WeatherRefreshInterval::Hourly, s.weather.refresh_interval);
    assert_eq!(41.0082, s.weather.latitude);
    assert_eq!(28.9784, s.weather.longitude);
    assert_eq!("İstanbul", s.weather.city);
    assert_eq!(WeatherLocationSource::City, s.weather.location_source);
    assert!(s.weather.use_location);
    assert_eq!(1762800000i64, s.weather.location_updated_unix);

    assert!(!s.clipboard.history_enabled);
}

#[test]
fn hot_corners_section() {
    let s = s();
    assert_eq!(CornerAction::OpenNotch, s.hot_corners.top_left);
    assert_eq!(CornerAction::OpenSearch, s.hot_corners.top_right);
    assert_eq!(CornerAction::OpenKoyWindow, s.hot_corners.bottom_left);
    assert_eq!(CornerAction::ToggleDock, s.hot_corners.bottom_right);
    assert_eq!(700, s.hot_corners.dwell_ms);
    assert!(!s.hot_corners.disable_in_fullscreen);
}

#[test]
fn quick_launch_section() {
    assert_eq!([r"C:\Apps\a.exe", r"C:\Docs"], s().quick_launch.items);
}

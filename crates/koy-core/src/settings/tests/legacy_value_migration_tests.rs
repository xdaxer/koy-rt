//! `Settings/LegacyValueMigrationTests.cs`: v3 → v4: sayı ve dile bağlı metin değerleri enum adına çevrilir;
//! tanınmayan değer yalnız kendi alanını bozar.

use super::{FIXTURE_V3, load_json};
use crate::appearance::{AnimationSpeed, AppTheme, MaterialLevel};
use crate::dock::{DockIndicatorStyle, MiddleClickAction};
use crate::media::MusicCardStyle;
use crate::notch::{IdleDisplayMode, NotchOpenBehavior, NotchStyleMode, WheelAction};
use crate::settings::*;
use crate::weather::WeatherRefreshInterval;

fn load_with(key: &str, json_value: &str) -> AppSettings {
    load_json(&format!("{{ \"Version\": 3, \"{key}\": {json_value} }}"))
}

fn text(stored: &str) -> String {
    format!("\"{stored}\"")
}

#[test]
fn golden_v3_file_lands_every_converted_value_on_the_right_enum() {
    let s = load_json(FIXTURE_V3);

    assert_eq!(AppSettings::CURRENT_VERSION, s.version);
    assert_eq!(AppTheme::Light, s.appearance.theme); // "Light"
    assert_eq!(AnimationSpeed::Fast, s.appearance.animation_speed); // "Hızlı"
    assert_eq!(UpdateChannel::Preview, s.general.update_channel); // "Önizleme"
    assert_eq!(WeatherRefreshInterval::Hourly, s.weather.refresh_interval); // "1h"
    assert_eq!(MaterialLevel::Glass, s.appearance.material); // "Cam"
    assert_eq!(NotchStyleMode::Floating, s.notch.style); // "Yüzen"
    assert_eq!(MusicCardStyle::Artwork, s.music.card_style); // "Artwork"
    assert_eq!(IdleDisplayMode::RingClockDay, s.notch.idle_display); // 3
    assert_eq!(NotchOpenBehavior::Click, s.notch.open_behavior); // 1
    assert_eq!(WheelAction::Volume, s.notch.wheel_action); // 1
    assert_eq!(MiddleClickAction::Close, s.dock.middle_click_action); // 1
    assert_eq!(DockIndicatorStyle::Line, s.dock.indicator_style); // 1

    // Dönüştürülmeyen alanlar olduğu gibi kalır.
    assert_eq!(Some("#FF8800".to_string()), s.appearance.accent_color);
    assert_eq!(36.0, s.dock.icon_size);
    assert_eq!(["bookmarks", "files"], s.search.disabled_sources);
}

#[test]
fn theme_legacy_values() {
    let cases = [
        ("System", AppTheme::System),
        ("sistem", AppTheme::System),
        ("Dark", AppTheme::Dark),
        ("Koyu", AppTheme::Dark),
        ("Light", AppTheme::Light),
        ("Açık", AppTheme::Light),
        ("acik", AppTheme::Light),
    ];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("Theme", &text(stored)).appearance.theme, "{stored}");
    }
}

#[test]
fn material_legacy_values() {
    let cases = [
        ("Katı", MaterialLevel::Solid),
        ("Buzlu", MaterialLevel::Frosted),
        ("Yarı saydam", MaterialLevel::Frosted), // iki seviyeli dönemden kalan
        ("Translucent", MaterialLevel::Frosted),
        ("acrylic", MaterialLevel::Frosted),
        ("Cam", MaterialLevel::Glass),
        (" glass ", MaterialLevel::Glass),
    ];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("Material", &text(stored)).appearance.material, "{stored}");
    }
}

#[test]
fn animation_speed_legacy_values() {
    let cases = [
        ("Sakin", AnimationSpeed::Calm),
        ("Normal", AnimationSpeed::Normal),
        ("Hızlı", AnimationSpeed::Fast),
        ("fast", AnimationSpeed::Fast),
        ("Kapalı", AnimationSpeed::Off),
    ];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("AnimationSpeed", &text(stored)).appearance.animation_speed, "{stored}");
    }
}

#[test]
fn update_channel_legacy_values() {
    let cases = [("Kararlı", UpdateChannel::Stable), ("Önizleme", UpdateChannel::Preview)];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("UpdateChannel", &text(stored)).general.update_channel, "{stored}");
    }
}

#[test]
fn notch_style_legacy_values() {
    let cases = [
        ("Yapışık", NotchStyleMode::Pinned),
        ("Yüzen", NotchStyleMode::Floating),
        ("yuzen", NotchStyleMode::Floating),
        ("Floating", NotchStyleMode::Floating),
    ];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("NotchStyle", &text(stored)).notch.style, "{stored}");
    }
}

#[test]
fn music_card_style_legacy_values() {
    let cases = [
        ("Neutral", MusicCardStyle::Neutral),
        ("Düz", MusicCardStyle::Neutral),
        ("Tinted", MusicCardStyle::Tinted),
        ("Artwork", MusicCardStyle::Artwork),
        ("Albüm kapağı", MusicCardStyle::Artwork),
    ];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("MusicCardStyle", &text(stored)).music.card_style, "{stored}");
    }
}

#[test]
fn weather_refresh_legacy_values() {
    let cases = [
        ("30m", WeatherRefreshInterval::Every30Minutes),
        ("1h", WeatherRefreshInterval::Hourly),
        ("startup", WeatherRefreshInterval::OnStartup),
    ];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("WeatherRefreshInterval", &text(stored)).weather.refresh_interval, "{stored}");
    }
}

#[test]
fn idle_display_ints() {
    let cases = [
        (0, IdleDisplayMode::ClockAndBattery),
        (1, IdleDisplayMode::ClockOnly),
        (2, IdleDisplayMode::Nothing),
        (3, IdleDisplayMode::RingClockDay),
    ];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("IdleDisplayMode", &stored.to_string()).notch.idle_display, "{stored}");
    }
}

#[test]
fn open_behavior_ints() {
    let cases = [(0, NotchOpenBehavior::Hover), (1, NotchOpenBehavior::Click)];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("NotchOpenBehavior", &stored.to_string()).notch.open_behavior, "{stored}");
    }
}

#[test]
fn wheel_action_ints() {
    let cases = [(0, WheelAction::SwitchPanel), (1, WheelAction::Volume)];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("WheelAction", &stored.to_string()).notch.wheel_action, "{stored}");
    }
}

#[test]
fn middle_click_ints() {
    let cases = [(0, MiddleClickAction::NewWindow), (1, MiddleClickAction::Close)];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("MiddleClickAction", &stored.to_string()).dock.middle_click_action, "{stored}");
    }
}

#[test]
fn dock_indicator_ints() {
    let cases = [(0, DockIndicatorStyle::Dot), (1, DockIndicatorStyle::Line)];
    for (stored, expected) in cases {
        assert_eq!(expected, load_with("DockIndicatorStyle", &stored.to_string()).dock.indicator_style, "{stored}");
    }
}

#[test]
fn an_unknown_value_resets_only_its_own_field_and_keeps_the_rest_of_the_file() {
    let s = load_json(
        r##"
        {
          "Version": 3,
          "Theme": "saçma",
          "Material": "bilinmeyen",
          "UpdateChannel": 42,
          "IdleDisplayMode": 99,
          "WheelAction": "yok",
          "AnimationSpeed": null,
          "DockIconSize": 55,
          "AccentColor": "#112233",
          "Material2": "yok"
        }
        "##,
    );

    let defaults = AppSettings::default();
    assert_eq!(defaults.appearance.theme, s.appearance.theme);
    assert_eq!(defaults.appearance.material, s.appearance.material);
    assert_eq!(defaults.general.update_channel, s.general.update_channel);
    assert_eq!(defaults.notch.idle_display, s.notch.idle_display);
    assert_eq!(defaults.notch.wheel_action, s.notch.wheel_action);
    assert_eq!(defaults.appearance.animation_speed, s.appearance.animation_speed);
    assert_eq!(55.0, s.dock.icon_size);
    assert_eq!(Some("#112233".to_string()), s.appearance.accent_color);
}

#[test]
fn a_v4_file_is_not_translated_again() {
    // Sürüm 4 dosyasında enum adları zaten yazılıdır; çeviri yalnız eski sürümlere uygulanır.
    let s = load_json(r#"{ "Version": 4, "Appearance": { "Material": "Glass" }, "Notch": { "IdleDisplay": "Nothing" } }"#);

    assert_eq!(MaterialLevel::Glass, s.appearance.material);
    assert_eq!(IdleDisplayMode::Nothing, s.notch.idle_display);
}

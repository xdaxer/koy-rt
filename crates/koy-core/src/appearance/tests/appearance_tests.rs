use std::collections::HashMap;

use regex::Regex;

use super::{assert_eq_precision, resources_dir};
use crate::appearance::{
    AccentPalette, AnimationSpeed, AppTheme, ColorMath, CornerSetting, CornerStyle, MotionPolicy, ResolvedTheme, ThemeSetting,
};

#[test]
fn theme_setting_resolves_system_and_fixed_values() {
    let rows = [
        (AppTheme::Dark, false, ResolvedTheme::Dark),
        (AppTheme::Dark, true, ResolvedTheme::Dark),
        (AppTheme::Light, false, ResolvedTheme::Light),
        (AppTheme::System, true, ResolvedTheme::Light),
        (AppTheme::System, false, ResolvedTheme::Dark),
    ];
    for (value, system_light, expected) in rows {
        assert_eq!(expected, ThemeSetting::resolve(value, system_light), "{value:?} {system_light}");
    }
}

#[test]
fn speed_factor_follows_setting() {
    let rows = [(AnimationSpeed::Calm, 1.5), (AnimationSpeed::Normal, 1.0), (AnimationSpeed::Fast, 0.65), (AnimationSpeed::Off, 0.0)];
    for (speed, expected) in rows {
        assert_eq!(expected, MotionPolicy::speed_factor(speed), "{speed:?}");
    }
}

#[test]
fn reduce_motion_and_battery_saver_turn_motion_off() {
    assert_eq!(1.0, MotionPolicy::scale(AnimationSpeed::Normal, true, true, false));
    assert_eq!(0.0, MotionPolicy::scale(AnimationSpeed::Normal, true, false, false));
    assert_eq!(1.0, MotionPolicy::scale(AnimationSpeed::Normal, false, false, false));
    assert_eq!(0.0, MotionPolicy::scale(AnimationSpeed::Calm, false, true, true));
    assert_eq!(0.0, MotionPolicy::scale(AnimationSpeed::Off, false, true, false));
}

#[test]
fn scaled_milliseconds_rounds_and_zero_scale_is_instant() {
    assert_eq!(270, MotionPolicy::scaled_milliseconds(180.0, 1.5));
    assert_eq!(0, MotionPolicy::scaled_milliseconds(180.0, 0.0));
}

#[test]
fn corner_setting_maps_stored_value() {
    let rows = [
        (0.0, CornerStyle::Standard),
        (6.0, CornerStyle::Sharp),
        (12.0, CornerStyle::Standard),
        (18.0, CornerStyle::Standard), // eski varsayılan: seçim sayılmaz
        (17.0, CornerStyle::Round),
    ];
    for (stored, expected) in rows {
        assert_eq!(expected, CornerSetting::resolve(stored), "{stored}");
        assert_eq!(expected, CornerSetting::resolve(CornerSetting::to_stored(expected)), "{stored}");
    }
}

#[test]
fn contrast_matches_known_values() {
    assert_eq_precision(21.0, ColorMath::contrast_hex("#000000", "#FFFFFF"), 1);
    assert_eq_precision(1.0, ColorMath::contrast_hex("#3B82F6", "#3B82F6"), 3);
    let c = ColorMath::contrast_hex("#3B82F6", "#0A0A0B");
    assert!((5.3..=5.5).contains(&c), "{c}");
}

#[test]
fn accent_palette_text_and_on_accent_meet_contrast() {
    let rows = [
        ("#3B82F6", ResolvedTheme::Dark, "#18181B"),
        ("#3B82F6", ResolvedTheme::Light, "#FFFFFF"),
        ("#8B5CF6", ResolvedTheme::Dark, "#18181B"),
        ("#8B5CF6", ResolvedTheme::Light, "#FFFFFF"),
        ("#F97316", ResolvedTheme::Light, "#FFFFFF"),
        ("#22C55E", ResolvedTheme::Light, "#FFFFFF"),
    ];
    for (accent, theme, surface) in rows {
        let p = AccentPalette::derive(Some(accent), theme, surface);
        assert!(ColorMath::contrast_hex(&p.text, surface) >= 4.5, "Text {}", p.text);
        assert!(ColorMath::contrast_hex(&p.on_accent, &p.base) >= 4.5, "OnAccent {}", p.on_accent);
    }
}

#[test]
fn invalid_accent_falls_back_to_koy_blue() {
    assert_eq!("#3B82F6", AccentPalette::derive(Some("yok"), ResolvedTheme::Dark, "#18181B").base);
}

// --- Tema sözlüklerini (XAML) doğrudan okuyan denetim: renk anahtarları ve kontrast ---

fn palette(file: &str) -> HashMap<String, String> {
    let text = std::fs::read_to_string(resources_dir().join(file)).expect("XAML dosyası okunamadı");
    let re = Regex::new(r#"x:Key="(\w+)"\s+Color="(#[0-9A-Fa-f]{6,8})""#).unwrap();
    let mut map = HashMap::new();
    for m in re.captures_iter(&text) {
        // `ToDictionary` yinelenen anahtarda fırlatır.
        let previous = map.insert(m[1].to_string(), m[2].to_string());
        assert!(previous.is_none(), "{file}: yinelenen anahtar {}", &m[1]);
    }
    map
}

fn sorted_keys(map: &HashMap<String, String>) -> Vec<&String> {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    keys
}

#[test]
fn dark_and_light_palettes_define_the_same_keys() {
    let dark = palette("Theme.Dark.xaml");
    let light = palette("Theme.Light.xaml");
    assert!(!dark.is_empty());
    assert_eq!(sorted_keys(&dark), sorted_keys(&light));
}

#[test]
fn text_tokens_meet_aa_on_surfaces() {
    for file in ["Theme.Dark.xaml", "Theme.Light.xaml"] {
        let p = palette(file);
        for surface in ["KoyBgWindow", "KoyBgRow", "KoyBgShell"] {
            for text in ["KoyTextPrimary", "KoyTextSecondary", "KoyTextMuted"] {
                assert!(ColorMath::contrast_hex(&p[text], &p[surface]) >= 4.5, "{file}: {text} / {surface}");
            }
        }

        assert!(ColorMath::contrast_hex(&p["KoyDangerText"], &p["KoyBgRow"]) >= 4.5, "{file}: danger");
        assert!(ColorMath::contrast_hex(&p["KoyFgInfo"], &p["KoyBgInfo"]) >= 4.5, "{file}: info");
        assert!(ColorMath::contrast_hex(&p["KoyAccentText"], &p["KoyBgRow"]) >= 4.5, "{file}: accent text");
        assert!(ColorMath::contrast_hex(&p["KoyOnAccent"], &p["KoyAccent"]) >= 4.5, "{file}: on accent");
    }
}

#[test]
fn control_boundaries_meet_non_text_contrast() {
    for file in ["Theme.Dark.xaml", "Theme.Light.xaml"] {
        let p = palette(file);
        // Kapalı anahtarın kenarı ve odak halkası, satır zemininde en az 3:1 olmalı (WCAG 1.4.11).
        assert!(ColorMath::contrast_hex(&p["KoyBorderControl"], &p["KoyBgRow"]) >= 3.0, "{file}: border");
        assert!(ColorMath::contrast_hex(&p["KoyFocusRing"], &p["KoyBgRow"]) >= 3.0, "{file}: focus");
        assert!(ColorMath::contrast_hex(&p["KoyToggleKnobOff"], &p["KoyToggleTrackOff"]) >= 3.0, "{file}: knob");
        assert!(ColorMath::contrast_hex(&p["KoyToggleKnobOn"], &p["KoyAccent"]) >= 3.0, "{file}: knob on");
    }
}

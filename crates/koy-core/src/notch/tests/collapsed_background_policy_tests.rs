//! `Notch/CollapsedBackgroundPolicyTests.cs`.

use crate::appearance::{ColorMath, Rgb};
use crate::notch::{CollapsedBackgroundPolicy, LiveBackgroundPresets};
use crate::settings::SettingsKeyMap;

const LIGHT_CLOCK_TEXT: Rgb = Rgb::new(0xF4, 0xF4, 0xF5);

#[test]
fn all_presets_resolve_to_readable_dark_stops() {
    for preset in LiveBackgroundPresets::all() {
        let stops = CollapsedBackgroundPolicy::resolve_ambient_stops(Some(&preset.id));
        assert!(!stops.is_empty());
        for stop in stops {
            let contrast = ColorMath::contrast(stop, LIGHT_CLOCK_TEXT);
            assert!(
                contrast >= CollapsedBackgroundPolicy::MIN_TEXT_CONTRAST,
                "Preset '{}' stop {:?} had contrast {contrast:.2}",
                preset.id,
                stop
            );
        }
    }
}

#[test]
fn unknown_or_empty_preset_falls_back_cleanly() {
    for id in [None, Some(""), Some("non-existent-preset")] {
        let stops = CollapsedBackgroundPolicy::resolve_ambient_stops(id);
        assert!(!stops.is_empty(), "{id:?}");
        for s in stops {
            assert!(ColorMath::contrast(s, LIGHT_CLOCK_TEXT) >= CollapsedBackgroundPolicy::MIN_TEXT_CONTRAST, "{id:?}");
        }
    }
}

#[test]
fn ensure_readable_darkens_bright_stops_while_preserving_dark_stops() {
    let bright_white = Rgb::new(0xFF, 0xFF, 0xFF);
    let dark_base = Rgb::new(0x0A, 0x0A, 0x0B);
    let darkened = CollapsedBackgroundPolicy::ensure_readable(bright_white);
    let kept = CollapsedBackgroundPolicy::ensure_readable(dark_base);
    assert!(ColorMath::contrast(darkened, LIGHT_CLOCK_TEXT) >= CollapsedBackgroundPolicy::MIN_TEXT_CONTRAST);
    assert_eq!(dark_base, kept);
}

#[test]
fn settings_key_map_maps_notch_collapsed_background() {
    let key = SettingsKeyMap::v3_to_v4().get("NotchCollapsedBackground").expect("eşleme");
    assert_eq!(("Notch".to_string(), "CollapsedBackground".to_string()), *key);
}

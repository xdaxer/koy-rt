use crate::notch::*;
use crate::settings::NotchSettings;

const BOTH: LiveBackgroundTarget = LiveBackgroundTarget::Both;

const ALL_TARGETS: [LiveBackgroundTarget; 4] =
    [LiveBackgroundTarget::None, LiveBackgroundTarget::Notch, LiveBackgroundTarget::MusicCard, LiveBackgroundTarget::Both];
const ALL_SURFACES: [LiveBackgroundSurface; 2] = [LiveBackgroundSurface::Notch, LiveBackgroundSurface::MusicCard];
const BOOLS: [bool; 2] = [false, true];

// ---- Oynatma kararı: tüm oynat/durdur kombinasyonları ----

#[test]
fn should_animate_truth_table_for_active_target() {
    // (açık, görünür, hareketi azalt, pil, beklenen)
    let rows = [
        (true, true, false, false, true),
        (false, true, false, false, false), // çentik kapalı
        (true, false, false, false, false), // pencere gizli
        (false, false, false, false, false),
        (true, true, true, false, false), // hareketi azalt
        (true, true, false, true, false), // pil tasarrufu
        (true, true, true, true, false),
        (false, true, true, true, false),
    ];
    for (expanded, visible, reduce, battery, expected) in rows {
        assert_eq!(
            expected,
            LiveBackgroundPlaybackPolicy::should_animate(expanded, visible, reduce, battery, BOTH),
            "{expanded} {visible} {reduce} {battery}"
        );
    }
}

#[test]
fn should_animate_all_combinations_only_one_plays() {
    let mut playing = 0;
    for expanded in BOOLS {
        for visible in BOOLS {
            for reduce in BOOLS {
                for battery in BOOLS {
                    if LiveBackgroundPlaybackPolicy::should_animate(expanded, visible, reduce, battery, BOTH) {
                        playing += 1;
                        assert!(expanded && visible && !reduce && !battery);
                    }
                }
            }
        }
    }

    assert_eq!(1, playing);
}

#[test]
fn should_animate_target_none_never_animates() {
    assert!(!LiveBackgroundPlaybackPolicy::should_animate(true, true, false, false, LiveBackgroundTarget::None));
}

// `ShouldAnimate_undefined_target_never_animates` ((LiveBackgroundTarget)99): Rust enum'una tanımsız değer girmez,
// taşınamaz.

#[test]
fn should_animate_active_targets_animate_when_expanded() {
    for target in [LiveBackgroundTarget::Notch, LiveBackgroundTarget::MusicCard, LiveBackgroundTarget::Both] {
        assert!(LiveBackgroundPlaybackPolicy::should_animate(true, true, false, false, target), "{target:?}");
    }
}

#[test]
fn applies_to_matches_target_and_surface() {
    use LiveBackgroundSurface::{MusicCard as MusicSurface, Notch as NotchSurf};
    use LiveBackgroundTarget::{Both as B, MusicCard, None as N, Notch};
    let rows = [
        (N, NotchSurf, false),
        (N, MusicSurface, false),
        (Notch, NotchSurf, true),
        (Notch, MusicSurface, false),
        (MusicCard, NotchSurf, false),
        (MusicCard, MusicSurface, true),
        (B, NotchSurf, true),
        (B, MusicSurface, true),
    ];
    for (target, surface, expected) in rows {
        assert_eq!(expected, LiveBackgroundPlaybackPolicy::applies_to(target, surface), "{target:?} {surface:?}");
    }
}

// ---- Üç durumlu karar: Off / StaticFirstFrame / Animate ----

#[test]
fn decide_collapsed_is_off_so_resources_are_released() {
    assert_eq!(
        LiveBackgroundPlaybackMode::Off,
        LiveBackgroundPlaybackPolicy::decide(false, true, false, false, BOTH, LiveBackgroundSurface::Notch)
    );
}

#[test]
fn decide_hidden_window_is_off() {
    assert_eq!(
        LiveBackgroundPlaybackMode::Off,
        LiveBackgroundPlaybackPolicy::decide(true, false, false, false, BOTH, LiveBackgroundSurface::MusicCard)
    );
}

#[test]
fn decide_surface_not_targeted_is_off_even_when_expanded() {
    assert_eq!(
        LiveBackgroundPlaybackMode::Off,
        LiveBackgroundPlaybackPolicy::decide(true, true, false, false, LiveBackgroundTarget::Notch, LiveBackgroundSurface::MusicCard)
    );
}

#[test]
fn decide_target_none_is_off() {
    assert_eq!(
        LiveBackgroundPlaybackMode::Off,
        LiveBackgroundPlaybackPolicy::decide(true, true, false, false, LiveBackgroundTarget::None, LiveBackgroundSurface::Notch)
    );
}

#[test]
fn decide_expanded_animates() {
    assert_eq!(
        LiveBackgroundPlaybackMode::Animate,
        LiveBackgroundPlaybackPolicy::decide(true, true, false, false, LiveBackgroundTarget::Notch, LiveBackgroundSurface::Notch)
    );
}

#[test]
fn decide_reduce_motion_shows_static_first_frame() {
    assert_eq!(
        LiveBackgroundPlaybackMode::StaticFirstFrame,
        LiveBackgroundPlaybackPolicy::decide(true, true, true, false, BOTH, LiveBackgroundSurface::Notch)
    );
}

#[test]
fn decide_battery_saver_shows_static_first_frame() {
    assert_eq!(
        LiveBackgroundPlaybackMode::StaticFirstFrame,
        LiveBackgroundPlaybackPolicy::decide(true, true, false, true, BOTH, LiveBackgroundSurface::MusicCard)
    );
}

#[test]
fn decide_never_animates_unless_should_animate_agrees() {
    for target in ALL_TARGETS {
        for surface in ALL_SURFACES {
            for expanded in BOOLS {
                for visible in BOOLS {
                    for reduce in BOOLS {
                        for battery in BOOLS {
                            let mode = LiveBackgroundPlaybackPolicy::decide(expanded, visible, reduce, battery, target, surface);
                            let animate = LiveBackgroundPlaybackPolicy::should_animate(expanded, visible, reduce, battery, target);
                            if mode == LiveBackgroundPlaybackMode::Animate {
                                assert!(animate && LiveBackgroundPlaybackPolicy::applies_to(target, surface));
                            }
                            if !expanded || !visible {
                                assert_eq!(LiveBackgroundPlaybackMode::Off, mode);
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---- Yerel dosya doğrulama ----

fn validate(path: &str, size: i64) -> LiveBackgroundFileError {
    LiveBackgroundFileValidator::validate(Some(path), size)
}

#[test]
fn validate_accepts_allowed_local_files() {
    for path in [r"C:\Videos\loop.gif", r"C:\Videos\loop.mp4", r"D:\a b\loop.webm", r"C:\Videos\still.webp", r"C:\Videos\LOOP.GIF"] {
        assert_eq!(LiveBackgroundFileError::None, validate(path, 1024), "{path}");
    }
}

#[test]
fn allowed_extensions_are_exactly_the_four_formats() {
    assert_eq!(&[".gif", ".mp4", ".webm", ".webp"][..], LiveBackgroundFileValidator::allowed_extensions());
    assert_eq!("*.gif;*.mp4;*.webm;*.webp", LiveBackgroundFileValidator::dialog_pattern());
}

#[test]
fn validate_rejects_missing_path() {
    assert_eq!(LiveBackgroundFileError::MissingPath, LiveBackgroundFileValidator::validate(None, 1024));
    for path in ["", "   "] {
        assert_eq!(LiveBackgroundFileError::MissingPath, validate(path, 1024), "{path:?}");
    }
}

#[test]
fn validate_rejects_unsupported_extensions() {
    for path in [r"C:\a\x.exe", r"C:\a\x.png", r"C:\a\x.avi", r"C:\a\x.lnk", r"C:\a\noext", r"C:\a\x.gif.", r"C:\dir.gif\file"] {
        assert_eq!(LiveBackgroundFileError::UnsupportedExtension, validate(path, 1024), "{path}");
    }
}

#[test]
fn validate_rejects_non_local_or_relative_paths() {
    let paths = [
        r"\\server\share\x.gif",
        r"\\?\C:\x.gif",
        "https://example.com/x.gif",
        "http://example.com/x.mp4",
        "file:///C:/x.gif",
        r"x.gif",
        r"..\x.gif",
        r".\x.gif",
        r"C:x.gif",
        "C:/Videos/x.gif",
    ];
    for path in paths {
        assert_eq!(LiveBackgroundFileError::NotLocalPath, validate(path, 1024), "{path}");
    }
}

#[test]
fn validate_rejects_unsafe_paths() {
    for path in [r"C:\a\..\b\x.gif", r"C:\a\x.gif:stream.gif", r"C:\nul\x.gif", "C:\\a\\x\n.gif", r"C:\a\*.gif"] {
        assert_eq!(LiveBackgroundFileError::InvalidPath, validate(path, 1024), "{path:?}");
    }
}

#[test]
fn validate_rejects_empty_file() {
    assert_eq!(LiveBackgroundFileError::Empty, validate(r"C:\a\x.gif", 0));
    assert_eq!(LiveBackgroundFileError::Empty, validate(r"C:\a\x.gif", -1));
}

#[test]
fn validate_enforces_size_cap_inclusive() {
    assert_eq!(LiveBackgroundFileError::None, validate(r"C:\a\x.mp4", LiveBackgroundLimits::MAX_FILE_SIZE_BYTES));
    assert_eq!(LiveBackgroundFileError::TooLarge, validate(r"C:\a\x.mp4", LiveBackgroundLimits::MAX_FILE_SIZE_BYTES + 1));
}

#[test]
fn validate_rejects_too_long_path() {
    let path = format!(r"C:\{}.gif", "a".repeat(300));
    assert_eq!(LiveBackgroundFileError::InvalidPath, validate(&path, 1024));
}

// ---- Çözümleyici ----

#[test]
fn resolve_target_none_returns_none_even_with_valid_file() {
    let result = LiveBackgroundResolver::resolve(LiveBackgroundTarget::None, LiveBackgroundSourceKind::LocalFile, None, Some(r"C:\a\x.gif"), 10);
    assert_eq!(LiveBackgroundSelection::none(), result);
}

#[test]
fn resolve_ambient_uses_preset_and_falls_back_to_default() {
    let known = LiveBackgroundResolver::resolve(BOTH, LiveBackgroundSourceKind::BuiltInAmbient, Some("ocean"), None, 0);
    assert_eq!(LiveBackgroundContentKind::Ambient, known.kind);
    assert_eq!("ocean", known.preset.unwrap().id);

    let unknown = LiveBackgroundResolver::resolve(BOTH, LiveBackgroundSourceKind::BuiltInAmbient, Some("nope"), None, 0);
    assert_eq!(LiveBackgroundPresets::DEFAULT_ID, unknown.preset.unwrap().id);
}

#[test]
fn resolve_local_file_maps_extension_to_content_kind() {
    let rows = [
        (r"C:\a\x.gif", LiveBackgroundContentKind::AnimatedImage),
        (r"C:\a\x.mp4", LiveBackgroundContentKind::Video),
        (r"C:\a\x.webm", LiveBackgroundContentKind::Video),
        (r"C:\a\x.webp", LiveBackgroundContentKind::StillImage),
    ];
    for (path, expected) in rows {
        let result = LiveBackgroundResolver::resolve(BOTH, LiveBackgroundSourceKind::LocalFile, None, Some(path), 1024);
        assert_eq!(expected, result.kind, "{path}");
        assert_eq!(Some(path.to_string()), result.file_path);
        assert_eq!(LiveBackgroundFileError::None, result.error);
    }
}

#[test]
fn resolve_invalid_local_file_reports_error_and_draws_nothing() {
    let result = LiveBackgroundResolver::resolve(BOTH, LiveBackgroundSourceKind::LocalFile, None, Some(r"\\server\x.gif"), 1024);
    assert_eq!(LiveBackgroundContentKind::None, result.kind);
    assert_eq!(LiveBackgroundFileError::NotLocalPath, result.error);
    assert_eq!(None, result.file_path);
}

// `Resolve_undefined_source_returns_none` ((LiveBackgroundSourceKind)42): Rust enum'una tanımsız değer girmez,
// taşınamaz.

// ---- Önayar kataloğu ----

fn is_hex_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[test]
fn presets_have_unique_ids_and_valid_dark_colors() {
    let all = LiveBackgroundPresets::all();
    assert!(!all.is_empty());
    let mut ids: Vec<&str> = all.iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(all.len(), ids.len());
    assert!(all.iter().any(|p| p.id == LiveBackgroundPresets::DEFAULT_ID));

    // Video döngülerin paleti yalnız önizleme içindir; koyuluk ve döngü kuralı renk geçişlerinde.
    for preset in all.iter().filter(|p| p.video_file.is_none()) {
        assert!(!preset.name_key.trim().is_empty());
        assert!((8..=60).contains(&preset.cycle_seconds), "{}", preset.id);
        assert!(preset.colors.len() >= 2);
        for color in &preset.colors {
            assert!(is_hex_color(color), "{color}");
            let r = i32::from_str_radix(&color[1..3], 16).unwrap();
            let g = i32::from_str_radix(&color[3..5], 16).unwrap();
            let b = i32::from_str_radix(&color[5..7], 16).unwrap();
            assert!(r + g + b < 3 * 128, "{} {color} yeterince koyu değil", preset.id);
        }
    }
}

#[test]
fn find_unknown_preset_returns_default() {
    for id in [None, Some(""), Some("missing")] {
        assert_eq!(LiveBackgroundPresets::DEFAULT_ID, LiveBackgroundPresets::find(id).id, "{id:?}");
    }
}

#[test]
fn find_is_case_insensitive() {
    assert_eq!("ember", LiveBackgroundPresets::find(Some("EMBER")).id);
}

// ---- Sınırlar: çözümleme boyutu, kare planı, perde ----

#[test]
fn decode_size_clamps_width_to_620_and_keeps_ratio() {
    // küçük kaynak büyütülmez
    let rows = [(1240, 704, 620, 352), (1920, 1080, 620, 349), (300, 200, 300, 200), (620, 352, 620, 352)];
    for (w, h, ew, eh) in rows {
        assert_eq!((ew, eh), LiveBackgroundLimits::decode_size(w, h), "{w}x{h}");
    }
}

#[test]
fn decode_size_invalid_source_is_zero() {
    for (w, h) in [(0, 10), (10, 0), (-5, -5)] {
        assert_eq!((0, 0), LiveBackgroundLimits::decode_size(w, h), "{w}x{h}");
    }
}

#[test]
fn decode_size_extreme_aspect_never_returns_zero_height() {
    assert_eq!(1, LiveBackgroundLimits::decode_size(10000, 1).1);
}

#[test]
fn decode_pixel_width_matches_notch_canvas_width() {
    assert_eq!(620, LiveBackgroundLimits::DECODE_PIXEL_WIDTH);
}

#[test]
fn is_source_within_budget_limits_raw_decoded_size() {
    let rows = [
        (480, 270, 60, true),    // ~31 MB
        (1920, 1080, 15, true),  // ~124 MB
        (1920, 1080, 17, false), // ~141 MB
        (4000, 4000, 1, true),   // tek büyük kare (64 MB)
        (4000, 4000, 3, false),  // 192 MB
        (0, 100, 1, false),
        (100, 0, 1, false),
        (100, 100, 0, false),
        (100, 100, -1, false),
    ];
    for (w, h, frames, expected) in rows {
        assert_eq!(expected, LiveBackgroundLimits::is_source_within_budget(w, h, frames), "{w}x{h} {frames}");
    }
}

#[test]
fn is_source_within_budget_does_not_overflow_on_huge_input() {
    assert!(!LiveBackgroundLimits::is_source_within_budget(i32::MAX, i32::MAX, i32::MAX));
}

#[test]
fn frame_plan_small_gif_keeps_every_frame() {
    assert_eq!(1, GifFramePlan::stride(30, 620, 352));
    assert_eq!(30, GifFramePlan::kept_frames(30, 620, 352));
}

#[test]
fn frame_plan_caps_by_frame_count_for_tiny_frames() {
    // 100x100 kare 40 KB: bayt sınırı gevşek, 120 kare sınırı belirler.
    assert_eq!(1, GifFramePlan::stride(120, 100, 100));
    assert_eq!(2, GifFramePlan::stride(121, 100, 100));
    assert_eq!(120, GifFramePlan::kept_frames(240, 100, 100));
}

#[test]
fn frame_plan_caps_by_decoded_bytes_for_full_size_frames() {
    let stride = GifFramePlan::stride(120, 620, 352);
    let kept = GifFramePlan::kept_frames(120, 620, 352);

    assert!(stride > 1);
    assert!(kept as i64 * 620 * 352 * 4 <= LiveBackgroundLimits::MAX_DECODED_BYTES);
}

#[test]
fn frame_plan_never_exceeds_caps() {
    for (frames, w, h) in [(1, 620, 352), (500, 620, 352), (5000, 100, 100), (300, 4000, 4000)] {
        let kept = GifFramePlan::kept_frames(frames, w, h);

        assert!((1..=i32::max(1, LiveBackgroundLimits::MAX_GIF_FRAMES)).contains(&kept), "{frames} {w}x{h}: {kept}");
        assert!(kept == 1 || kept as i64 * w as i64 * h as i64 * 4 <= LiveBackgroundLimits::MAX_DECODED_BYTES, "{frames} {w}x{h}");
    }
}

#[test]
fn frame_plan_invalid_input_is_safe() {
    assert_eq!(1, GifFramePlan::stride(0, 620, 352));
    assert_eq!(1, GifFramePlan::stride(10, 0, 352));
    assert_eq!(0, GifFramePlan::kept_frames(0, 620, 352));
}

#[test]
fn frame_plan_groups_cover_every_frame_once() {
    const FRAMES: i32 = 250;
    let stride = GifFramePlan::stride(FRAMES, 100, 100);
    let ends = (0..FRAMES).filter(|i| GifFramePlan::is_group_end(*i, FRAMES, stride)).count() as i32;

    assert_eq!(GifFramePlan::kept_frames(FRAMES, 100, 100), ends);
    assert!(GifFramePlan::is_group_end(FRAMES - 1, FRAMES, stride));
}

#[test]
fn normalize_delay_matches_browser_behaviour() {
    for (centiseconds, expected_ms) in [(0, 100), (1, 100), (2, 20), (10, 100), (25, 250)] {
        assert_eq!(expected_ms, GifFramePlan::normalize_delay_ms(centiseconds), "{centiseconds}");
    }
}

#[test]
fn scrim_percent_is_clamped() {
    for (input, expected) in [(0, 30), (29, 30), (65, 65), (100, 90), (-10, 30)] {
        assert_eq!(expected, LiveBackgroundLimits::clamp_scrim_percent(input), "{input}");
        assert!((expected as f64 / 100.0 - LiveBackgroundLimits::scrim_opacity(input)).abs() < 1e-5, "{input}");
    }
}

// ---- Ayar varsayılanları ----

#[test]
fn defaults_keep_feature_off_for_existing_users() {
    let notch = NotchSettings::default();

    assert_eq!(LiveBackgroundTarget::None, notch.live_background_target);
    assert_eq!(LiveBackgroundSourceKind::BuiltInAmbient, notch.live_background_source);
    assert_eq!(LiveBackgroundPresets::DEFAULT_ID, notch.live_background_preset);
    assert_eq!("", notch.live_background_file_path);
    assert_eq!(LiveBackgroundLimits::DEFAULT_SCRIM_PERCENT, notch.live_background_scrim_percent);
    assert_eq!(
        LiveBackgroundPlaybackMode::Off,
        LiveBackgroundPlaybackPolicy::decide(true, true, false, false, notch.live_background_target, LiveBackgroundSurface::Notch)
    );
}

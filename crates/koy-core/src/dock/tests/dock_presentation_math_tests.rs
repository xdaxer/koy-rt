use crate::dock::{
    DockBadgeCorner, DockBadgeStyle, DockColorSource, DockIndicatorStyle, DockPresentationMath, DockRevealTrigger, EdgeRevealTracker,
};
use crate::settings::DockSettings;
use crate::time::TimeSpan;

#[test]
fn delay_is_clamped_to_zero_and_two_seconds() {
    for (input, expected) in [(-5, 0), (0, 0), (300, 300), (2000, 2000), (9999, 2000)] {
        assert_eq!(expected, DockPresentationMath::clamp_delay(input), "{input}");
    }
}

#[test]
fn indicator_is_none_without_windows() {
    assert_eq!(0, DockPresentationMath::indicator_count(0, DockIndicatorStyle::Dot, true));
    assert_eq!(0, DockPresentationMath::indicator_count(-1, DockIndicatorStyle::Line, false));
}

#[test]
fn one_dot_per_window_is_capped_at_three() {
    assert_eq!(1, DockPresentationMath::indicator_count(1, DockIndicatorStyle::Dot, true));
    assert_eq!(2, DockPresentationMath::indicator_count(2, DockIndicatorStyle::Dot, true));
    assert_eq!(3, DockPresentationMath::indicator_count(3, DockIndicatorStyle::Dot, true));
    assert_eq!(3, DockPresentationMath::indicator_count(7, DockIndicatorStyle::Dot, true));
}

#[test]
fn without_one_per_window_a_single_indicator_is_drawn() {
    for style in [DockIndicatorStyle::Dot, DockIndicatorStyle::Line, DockIndicatorStyle::Highlight] {
        assert_eq!(1, DockPresentationMath::indicator_count(5, style, false), "{style:?}");
    }
}

#[test]
fn per_window_mode_only_applies_to_dots() {
    for style in [DockIndicatorStyle::Line, DockIndicatorStyle::Highlight] {
        assert_eq!(1, DockPresentationMath::indicator_count(4, style, true), "{style:?}");
    }
}

#[test]
fn number_badge_text_caps_at_nine_plus() {
    for (count, expected) in [(0, ""), (-2, ""), (1, "1"), (9, "9"), (10, "9+"), (250, "9+")] {
        assert_eq!(expected, DockPresentationMath::badge_text(count, DockBadgeStyle::Number), "{count}");
    }
}

#[test]
fn dot_only_badge_has_no_text_but_shows_when_there_is_a_count() {
    for (count, shows) in [(0, false), (1, true), (40, true)] {
        assert_eq!("", DockPresentationMath::badge_text(count, DockBadgeStyle::DotOnly), "{count}");
        assert_eq!(shows, DockPresentationMath::shows_badge(count, DockBadgeStyle::DotOnly), "{count}");
    }
}

#[test]
fn number_badge_is_hidden_when_count_is_zero() {
    assert!(!DockPresentationMath::shows_badge(0, DockBadgeStyle::Number));
    assert!(DockPresentationMath::shows_badge(3, DockBadgeStyle::Number));
}

#[test]
fn edge_touch_reveals_without_waiting() {
    assert!(DockPresentationMath::should_reveal(DockRevealTrigger::EdgeTouch, true, false, false, TimeSpan::ZERO, 300));
    assert!(!DockPresentationMath::should_reveal(DockRevealTrigger::EdgeTouch, false, false, false, TimeSpan::ZERO, 300));
}

#[test]
fn edge_hold_waits_for_the_configured_delay() {
    let hold = |held: TimeSpan| DockPresentationMath::should_reveal(DockRevealTrigger::EdgeHold, true, false, false, held, 300);

    assert!(!hold(TimeSpan::from_millis(299)));
    assert!(hold(TimeSpan::from_millis(300)));
    assert!(hold(TimeSpan::from_secs(2)));
}

#[test]
fn edge_hold_delay_is_clamped_before_comparison() {
    // Dosyada negatif gecikme: sıfıra kıstırılır, hemen çıkar.
    assert!(DockPresentationMath::should_reveal(DockRevealTrigger::EdgeHold, true, false, false, TimeSpan::ZERO, -100));
}

#[test]
fn shortcut_only_ignores_the_edge() {
    assert!(!DockPresentationMath::should_reveal(DockRevealTrigger::ShortcutOnly, true, false, false, TimeSpan::from_secs(5), 0));
    assert!(DockPresentationMath::should_reveal(DockRevealTrigger::ShortcutOnly, false, false, true, TimeSpan::ZERO, 0));
}

#[test]
fn undefined_trigger_behaves_like_edge_hold_so_the_dock_stays_reachable() {
    // C# `(DockRevealTrigger)42` Rust'ta kurulamaz (enum yalnız tanımlı değerleri taşır; serde bilinmeyen adı zaten
    // reddeder). Karar kolu `EdgeHold` ile paylaşıldığından davranış EdgeHold üzerinden sınanır.
    let undefined = DockRevealTrigger::EdgeHold;

    assert!(!DockPresentationMath::should_reveal(undefined, true, false, false, TimeSpan::from_millis(100), 300));
    assert!(DockPresentationMath::should_reveal(undefined, true, false, false, TimeSpan::from_millis(300), 300));
}

#[test]
fn no_trigger_reveals_while_a_mouse_button_is_held() {
    for trigger in [DockRevealTrigger::EdgeTouch, DockRevealTrigger::EdgeHold, DockRevealTrigger::ShortcutOnly] {
        assert!(!DockPresentationMath::should_reveal(trigger, true, true, true, TimeSpan::from_secs(5), 0), "{trigger:?}");
    }
}

#[test]
fn hidden_edge_line_needs_setting_hidden_dock_and_no_fullscreen() {
    // (line, hidden, fullscreen, expected)
    for (line, hidden, fullscreen, expected) in [
        (true, true, false, true),
        (true, false, false, false),
        (false, true, false, false),
        (true, true, true, false),
    ] {
        assert_eq!(expected, DockPresentationMath::shows_hidden_edge_line(line, hidden, fullscreen), "({line}, {hidden}, {fullscreen})");
    }
}

#[test]
fn hex_color_must_be_six_digits_with_hash() {
    let cases: [(Option<&str>, bool); 7] = [
        (Some("#1E90FF"), true),
        (Some("#abcdef"), true),
        (Some("#ABC"), false),
        (Some("1E90FF"), false),
        (Some("#GG0000"), false),
        (Some(""), false),
        (None, false),
    ];
    for (text, valid) in cases {
        assert_eq!(valid, DockPresentationMath::is_valid_hex_color(text), "{text:?}");
    }
}

#[test]
fn custom_color_with_invalid_text_falls_back_to_accent() {
    assert_eq!(DockColorSource::Accent, DockPresentationMath::effective_color_source(DockColorSource::Custom, Some("not-a-color")));
    assert_eq!(DockColorSource::Custom, DockPresentationMath::effective_color_source(DockColorSource::Custom, Some("#FF8800")));
    assert_eq!(DockColorSource::Neutral, DockPresentationMath::effective_color_source(DockColorSource::Neutral, Some("")));
}

#[test]
fn new_dock_settings_have_defaults_that_keep_current_behavior() {
    let dock = DockSettings::default();

    assert_eq!(DockColorSource::Accent, dock.indicator_color);
    assert_eq!(DockColorSource::Accent, dock.badge_color);
    assert_eq!(DockBadgeStyle::Number, dock.badge_style);
    assert_eq!(DockBadgeCorner::TopRight, dock.badge_corner);
    // Bugünkü dock kenarda ~300 ms bekleyince açılır (EdgeRevealTracker); varsayılan bunu değiştirmemeli.
    assert_eq!(DockRevealTrigger::EdgeHold, dock.reveal_trigger);
    assert_eq!(EdgeRevealTracker::DEFAULT_DELAY.total_milliseconds() as i32, dock.reveal_delay_ms);
    assert!(!dock.hidden_edge_line);
    assert!(!dock.indicator_one_per_window);
}

#[test]
fn edge_reveal_delay_follows_trigger_and_is_clamped() {
    // (trigger, ms, expectedMs)
    let cases = [
        (DockRevealTrigger::EdgeTouch, 500, 0),
        (DockRevealTrigger::EdgeHold, 500, 500),
        (DockRevealTrigger::EdgeHold, 9999, 2000),
        (DockRevealTrigger::EdgeHold, -3, 0),
        (DockRevealTrigger::ShortcutOnly, 300, 300),
    ];
    for (trigger, ms, expected_ms) in cases {
        assert_eq!(TimeSpan::from_millis(expected_ms), DockPresentationMath::edge_reveal_delay(trigger, ms), "({trigger:?}, {ms})");
    }
}

#[test]
fn only_highlight_style_draws_no_mark() {
    for (style, expected) in [(DockIndicatorStyle::Dot, true), (DockIndicatorStyle::Line, true), (DockIndicatorStyle::Highlight, false)] {
        assert_eq!(expected, DockPresentationMath::draws_indicator_mark(style), "{style:?}");
    }
}

use crate::background::{
    BackgroundAnimationOptions, BackgroundAnimationPolicy, BackgroundKind, BackgroundPlaybackState,
};

const EXPANDED: BackgroundPlaybackState = BackgroundPlaybackState::new(false, false, false, true, 1.0);

fn all_opt_ins() -> BackgroundAnimationOptions {
    BackgroundAnimationOptions::new(true, true)
}

#[test]
fn static_kinds_never_animate() {
    for kind in [BackgroundKind::None, BackgroundKind::StillImage] {
        assert!(!BackgroundAnimationPolicy::should_animate(kind, EXPANDED, all_opt_ins()), "{kind:?}");
    }
}

#[test]
fn animated_kinds_animate_when_expanded_on_ac() {
    for kind in [BackgroundKind::AnimatedImage, BackgroundKind::Video] {
        assert!(BackgroundAnimationPolicy::should_animate_default(kind, EXPANDED), "{kind:?}");
    }
}

#[test]
fn idle_notch_does_not_animate_by_default() {
    for kind in [BackgroundKind::AnimatedImage, BackgroundKind::Video] {
        let idle = BackgroundPlaybackState { is_notch_idle: true, ..EXPANDED };

        assert!(!BackgroundAnimationPolicy::should_animate_default(kind, idle), "{kind:?}");
    }
}

#[test]
fn battery_saver_does_not_animate_by_default() {
    for kind in [BackgroundKind::AnimatedImage, BackgroundKind::Video] {
        let battery = BackgroundPlaybackState { battery_saver: true, ..EXPANDED };

        assert!(!BackgroundAnimationPolicy::should_animate_default(kind, battery), "{kind:?}");
    }
}

#[test]
fn idle_opt_in_allows_animation_while_idle() {
    let idle = BackgroundPlaybackState { is_notch_idle: true, ..EXPANDED };
    let options = BackgroundAnimationOptions { animate_when_idle: true, ..Default::default() };

    assert!(BackgroundAnimationPolicy::should_animate(BackgroundKind::Video, idle, options));
}

#[test]
fn battery_opt_in_allows_animation_on_battery() {
    let battery = BackgroundPlaybackState { battery_saver: true, ..EXPANDED };
    let options = BackgroundAnimationOptions { animate_on_battery: true, ..Default::default() };

    assert!(BackgroundAnimationPolicy::should_animate(BackgroundKind::AnimatedImage, battery, options));
}

#[test]
fn idle_opt_in_does_not_override_battery_without_battery_opt_in() {
    let idle_on_battery = BackgroundPlaybackState::new(true, true, false, true, 1.0);
    let options = BackgroundAnimationOptions { animate_when_idle: true, ..Default::default() };

    assert!(!BackgroundAnimationPolicy::should_animate(BackgroundKind::Video, idle_on_battery, options));
}

#[test]
fn fullscreen_stops_animation_even_with_all_opt_ins() {
    let fullscreen = BackgroundPlaybackState { fullscreen: true, ..EXPANDED };

    assert!(!BackgroundAnimationPolicy::should_animate(BackgroundKind::Video, fullscreen, all_opt_ins()));
}

#[test]
fn undefined_kind_never_animates() {
    for raw in [99, -1, 4] {
        assert!(!BackgroundAnimationPolicy::should_animate_raw(raw, EXPANDED, all_opt_ins()), "{raw}");
    }
}

#[test]
fn defined_raw_kinds_follow_the_enum() {
    // `(BackgroundKind)raw` dönüşümünün tanımlı değerleri enum ile aynı sonucu vermeli.
    for raw in 0..=3 {
        let kind = BackgroundKind::from_i32(raw).expect("tanımlı değer");
        assert_eq!(
            BackgroundAnimationPolicy::should_animate(kind, EXPANDED, all_opt_ins()),
            BackgroundAnimationPolicy::should_animate_raw(raw, EXPANDED, all_opt_ins()),
            "{raw}"
        );
    }
}

#[test]
fn hidden_notch_stops_animation_even_with_all_opt_ins() {
    // Gizli çentiği kimse görmez; "boşta da oynat" tercihi bunu aşamaz.
    let hidden = BackgroundPlaybackState { is_notch_visible: false, is_notch_idle: true, ..EXPANDED };

    assert!(!BackgroundAnimationPolicy::should_animate(BackgroundKind::Video, hidden, all_opt_ins()));
}

#[test]
fn no_motion_scale_stops_animation_even_with_all_opt_ins() {
    for scale in [0.0, -1.0, f64::NAN] {
        let reduced = BackgroundPlaybackState { motion_scale: scale, ..EXPANDED };

        assert!(!BackgroundAnimationPolicy::should_animate(BackgroundKind::AnimatedImage, reduced, all_opt_ins()), "{scale}");
    }
}

// `MotionPolicy` Appearance modülündedir: `background` özelliği tek başına açıkken bu iki test derlenmez.
#[cfg(feature = "appearance")]
#[test]
fn reduced_motion_from_motion_policy_stops_animation() {
    use crate::appearance::{AnimationSpeed, MotionPolicy};

    // Windows "animasyonları azalt" kapalıyken MotionPolicy 0 verir; politika bunu aynen izler.
    let scale = MotionPolicy::scale(AnimationSpeed::Normal, true, false, false);
    let state = BackgroundPlaybackState { motion_scale: scale, ..EXPANDED };

    assert!(!BackgroundAnimationPolicy::should_animate_default(BackgroundKind::Video, state));
}

#[cfg(feature = "appearance")]
#[test]
fn animation_speed_off_stops_animation() {
    use crate::appearance::{AnimationSpeed, MotionPolicy};

    let scale = MotionPolicy::scale(AnimationSpeed::Off, false, true, false);
    let state = BackgroundPlaybackState { motion_scale: scale, ..EXPANDED };

    assert!(!BackgroundAnimationPolicy::should_animate_default(BackgroundKind::Video, state));
}

#[test]
fn default_state_does_not_animate() {
    // Doldurulmayan durum güvenli tarafta kalır: görünmez ve hareketsiz.
    assert!(!BackgroundAnimationPolicy::should_animate_default(BackgroundKind::Video, BackgroundPlaybackState::default()));
}

#[test]
fn default_options_are_both_off() {
    let options = BackgroundAnimationOptions::default();

    assert!(!options.animate_when_idle);
    assert!(!options.animate_on_battery);
}

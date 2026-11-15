use crate::dock::{DockProximityMapping, DockProximityMode, DockStyleMode};

const ALL_MODES: [DockProximityMode; 5] = [
    DockProximityMode::DockDown,
    DockProximityMode::ReserveSpace,
    DockProximityMode::Dodge,
    DockProximityMode::AlwaysHidden,
    DockProximityMode::AlwaysFloating,
];

#[test]
fn capsule_keeps_every_mode() {
    for mode in ALL_MODES {
        assert!(DockProximityMapping::is_available(DockStyleMode::FloatingCapsule, mode), "{mode:?}");
        assert_eq!(mode, DockProximityMapping::effective(DockStyleMode::FloatingCapsule, mode), "{mode:?}");
    }
}

#[test]
fn full_width_offers_only_always_show_and_auto_hide() {
    // (mode, available, effective)
    let cases = [
        (DockProximityMode::ReserveSpace, true, DockProximityMode::ReserveSpace),
        (DockProximityMode::AlwaysHidden, true, DockProximityMode::AlwaysHidden),
        (DockProximityMode::DockDown, false, DockProximityMode::ReserveSpace),
        (DockProximityMode::Dodge, false, DockProximityMode::AlwaysHidden),
        (DockProximityMode::AlwaysFloating, false, DockProximityMode::ReserveSpace),
    ];
    for (mode, available, effective) in cases {
        assert_eq!(available, DockProximityMapping::is_available(DockStyleMode::FullWidth, mode), "{mode:?}");
        assert_eq!(effective, DockProximityMapping::effective(DockStyleMode::FullWidth, mode), "{mode:?}");
    }
}

#[test]
fn effective_mode_is_always_available_in_its_style() {
    for style in [DockStyleMode::FloatingCapsule, DockStyleMode::FullWidth] {
        for mode in ALL_MODES {
            assert!(
                DockProximityMapping::is_available(style, DockProximityMapping::effective(style, mode)),
                "({style:?}, {mode:?})"
            );
        }
    }
}

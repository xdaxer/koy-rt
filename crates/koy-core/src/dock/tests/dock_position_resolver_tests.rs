use crate::display::DisplayMonitor;
use crate::dock::{DockEdge, DockLayoutLimits, DockPositionResolver, DockProximityMode, DockResolvedState, DockStyleMode};
use crate::shell::WindowRect;

fn primary_1080p() -> DisplayMonitor {
    DisplayMonitor::new("MONITOR_1", "Primary 1080p", WindowRect::new(0, 0, 1920, 1080), WindowRect::new(0, 0, 1920, 1080), true)
        .with_dpi(1.0, 1.0)
        .with_handle(100)
}

fn secondary_1440p() -> DisplayMonitor {
    DisplayMonitor::new("MONITOR_2", "Secondary 1440p", WindowRect::new(1920, 0, 4480, 1440), WindowRect::new(1920, 0, 4480, 1440), false)
        .with_dpi(1.0, 1.0)
        .with_handle(200)
}

/// `DockPositionResolver.Resolve(...)` çağrısının adlandırılmış bağımsız değişkenleri (C# varsayılanlarıyla).
/// Test yazımı `Args { maximized: true, ..Args::new(...) }` biçimindedir.
#[derive(Clone)]
pub(super) struct Args<'a> {
    pub monitor: Option<&'a DisplayMonitor>,
    pub style: DockStyleMode,
    pub mode: DockProximityMode,
    pub content_width: f64,
    pub foreground: WindowRect,
    pub maximized: bool,
    pub mouse_at_edge: bool,
    pub mouse_over_dock: bool,
    pub fullscreen: bool,
    pub hide_in_fullscreen: bool,
    pub edge_reveal: bool,
    pub edge: DockEdge,
    pub length_percent: i32,
    pub edge_offset: f64,
    pub inner_padding: f64,
}

impl<'a> Args<'a> {
    pub fn new(
        monitor: Option<&'a DisplayMonitor>,
        style: DockStyleMode,
        mode: DockProximityMode,
        content_width: f64,
        foreground: WindowRect,
    ) -> Self {
        Args {
            monitor,
            style,
            mode,
            content_width,
            foreground,
            maximized: false,
            mouse_at_edge: false,
            mouse_over_dock: false,
            fullscreen: false,
            hide_in_fullscreen: true,
            edge_reveal: true,
            edge: DockEdge::Bottom,
            length_percent: DockLayoutLimits::DEFAULT_LENGTH_PERCENT,
            edge_offset: DockLayoutLimits::DEFAULT_EDGE_OFFSET_DIP,
            inner_padding: DockLayoutLimits::DEFAULT_INNER_PADDING_DIP,
        }
    }

    pub fn resolve(&self) -> DockResolvedState {
        DockPositionResolver::resolve(
            self.monitor,
            self.style,
            self.mode,
            self.content_width,
            self.foreground,
            self.maximized,
            self.mouse_at_edge,
            self.mouse_over_dock,
            self.fullscreen,
            self.hide_in_fullscreen,
            self.edge_reveal,
            self.edge,
            self.length_percent,
            self.edge_offset,
            self.inner_padding,
        )
    }
}

/// `Resolve(monitor, style, mode, contentWidth, foregroundBounds)`: geri kalan her şey varsayılan.
pub(super) fn resolve_default(
    monitor: &DisplayMonitor,
    style: DockStyleMode,
    mode: DockProximityMode,
    content_width: f64,
    foreground: WindowRect,
) -> DockResolvedState {
    Args::new(Some(monitor), style, mode, content_width, foreground).resolve()
}

const CAPSULE: DockStyleMode = DockStyleMode::FloatingCapsule;
const FULL: DockStyleMode = DockStyleMode::FullWidth;

#[test]
fn floating_capsule_when_no_window_intersects_floats_with_10px_margin_and_18px_radius() {
    let monitor = primary_1080p();
    let result = Args {
        mouse_at_edge: false,
        mouse_over_dock: false,
        maximized: false,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::DockDown, 500.0, WindowRect::new(100, 100, 900, 700)) // Far from dock
    }
    .resolve();

    assert_eq!(60.0, result.height_dip);
    // Top should be 1080 - 60 - 10 = 1010
    assert_eq!(1010.0, result.top_dip);
    // Center: (1920 - 500) / 2 = 710
    assert_eq!(710.0, result.left_dip);
    assert_eq!(500.0, result.width_dip);
    assert_eq!(18.0, result.corner_radius_top_left);
    assert_eq!(18.0, result.corner_radius_top_right);
    assert_eq!(18.0, result.corner_radius_bottom_right);
    assert_eq!(18.0, result.corner_radius_bottom_left);
    assert!(result.has_uniform_radius());
    assert!(!result.is_snapped);
    assert!(!result.is_handle_visible);
    assert!(!result.is_hidden);
    assert!(!result.is_full_width);
    assert_eq!(0, result.app_bar_reserved_height_dip);
}

#[test]
fn dock_down_when_window_approaches_dock_snaps_to_bottom_and_shrinks_to_46px_with_14px_top_corners() {
    // Window extends into dock proximity area (bottom >= 1080 - 80 = 1000)
    let approaching_window = WindowRect::new(200, 200, 1200, 1020);

    let result = resolve_default(&primary_1080p(), CAPSULE, DockProximityMode::DockDown, 520.0, approaching_window);

    assert!(result.is_snapped);
    assert_eq!(46.0, result.height_dip);
    // Top should be exactly at bottom: 1080 - 46 = 1034
    assert_eq!(1034.0, result.top_dip);
    assert_eq!(14.0, result.corner_radius_top_left);
    assert_eq!(14.0, result.corner_radius_top_right);
    assert_eq!(0.0, result.corner_radius_bottom_right);
    assert_eq!(0.0, result.corner_radius_bottom_left);
    assert!(!result.has_uniform_radius());
    assert!(!result.is_handle_visible);
    assert!(!result.is_hidden);
}

#[test]
fn dock_down_when_window_leaves_dock_area_springs_back_to_floating_capsule() {
    let distant_window = WindowRect::new(100, 50, 800, 600);

    let result = resolve_default(&primary_1080p(), CAPSULE, DockProximityMode::DockDown, 520.0, distant_window);

    assert!(!result.is_snapped);
    assert_eq!(60.0, result.height_dip);
    assert_eq!(1010.0, result.top_dip);
    assert_eq!(18.0, result.corner_radius_top_left);
    assert_eq!(18.0, result.corner_radius_bottom_left);
}

#[test]
fn reserve_space_always_snaps_to_bottom_and_reserves_46px_work_area() {
    let result = resolve_default(&primary_1080p(), CAPSULE, DockProximityMode::ReserveSpace, 480.0, WindowRect::EMPTY);

    assert!(result.is_snapped);
    assert_eq!(46.0, result.height_dip);
    assert_eq!(1034.0, result.top_dip);
    assert_eq!(14.0, result.corner_radius_top_left);
    assert_eq!(14.0, result.corner_radius_top_right);
    assert_eq!(0.0, result.corner_radius_bottom_left);
    assert_eq!(46, result.app_bar_reserved_height_dip);
}

#[test]
fn dodge_when_maximized_window_covers_dock_slides_offscreen_and_shows_handle() {
    let monitor = primary_1080p();
    let result = Args {
        maximized: true,
        mouse_at_edge: false,
        mouse_over_dock: false,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::Dodge, 500.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(result.is_hidden);
    assert!(result.is_handle_visible);
    assert!(result.translation_y > 60.0);
    assert_eq!(0.0, result.opacity);
}

#[test]
fn dodge_when_maximized_window_covers_dock_and_mouse_hovers_edge_slides_back_up() {
    let monitor = primary_1080p();
    let result = Args {
        maximized: true,
        mouse_at_edge: true, // Mouse at bottom edge (150ms hover)
        mouse_over_dock: false,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::Dodge, 500.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(!result.is_hidden);
    assert!(!result.is_handle_visible);
    assert_eq!(0.0, result.translation_y);
    assert_eq!(1.0, result.opacity);
    assert_eq!(60.0, result.height_dip);
}

#[test]
fn always_hidden_when_mouse_not_at_edge_remains_hidden() {
    let monitor = primary_1080p();
    let result = Args {
        mouse_at_edge: false,
        mouse_over_dock: false,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::AlwaysHidden, 500.0, WindowRect::EMPTY)
    }
    .resolve();

    assert!(result.is_hidden);
    assert_eq!(0.0, result.opacity);
    assert!(result.translation_y > 0.0);
    assert!(!result.is_handle_visible);
}

#[test]
fn always_hidden_when_mouse_touches_bottom_edge_becomes_visible() {
    let monitor = primary_1080p();
    let result = Args {
        mouse_at_edge: true,
        mouse_over_dock: false,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::AlwaysHidden, 500.0, WindowRect::EMPTY)
    }
    .resolve();

    assert!(!result.is_hidden);
    assert_eq!(1.0, result.opacity);
    assert_eq!(0.0, result.translation_y);
}

#[test]
fn full_width_snaps_to_bottom_with_48px_height_and_full_screen_width() {
    let result = resolve_default(&primary_1080p(), FULL, DockProximityMode::DockDown, 600.0, WindowRect::EMPTY);

    assert!(result.is_full_width);
    assert!(result.is_snapped);
    assert_eq!(0.0, result.left_dip);
    assert_eq!(1920.0, result.width_dip);
    assert_eq!(48.0, result.height_dip);
    assert_eq!(DockPositionResolver::FULL_WIDTH_HEIGHT_DIP, result.height_dip);
    // Top = 1080 - 48 = 1032
    assert_eq!(1032.0, result.top_dip);
    assert_eq!(0.0, result.corner_radius_top_left);
    assert_eq!(0.0, result.corner_radius_top_right);
    assert_eq!(0.0, result.corner_radius_bottom_right);
    assert_eq!(0.0, result.corner_radius_bottom_left);
}

#[test]
fn multi_monitor_secondary_screen_coordinates_calculates_correct_offsets() {
    // Left: 1920, Width: 2560, Height: 1440
    let result = resolve_default(&secondary_1440p(), CAPSULE, DockProximityMode::DockDown, 600.0, WindowRect::EMPTY);

    // Center on secondary: 1920 + (2560 - 600) / 2 = 1920 + 980 = 2900
    assert_eq!(2900.0, result.left_dip);
    // Top: 1440 - 70 = 1370
    assert_eq!(1370.0, result.top_dip);
    assert_eq!(600.0, result.width_dip);
    assert_eq!(60.0, result.height_dip);
}

#[test]
fn full_width_when_reserve_space_reserves_48px_work_area() {
    let result = resolve_default(&primary_1080p(), FULL, DockProximityMode::ReserveSpace, 600.0, WindowRect::EMPTY);

    assert!(result.is_full_width);
    assert_eq!(48, result.app_bar_reserved_height_dip);
    assert_eq!(48.0, result.height_dip);
}

#[test]
fn dodge_when_window_does_not_intersect_dock_floats_normally_without_handle() {
    let distant_window = WindowRect::new(100, 100, 500, 500);

    let result = resolve_default(&primary_1080p(), CAPSULE, DockProximityMode::Dodge, 500.0, distant_window);

    assert!(!result.is_hidden);
    assert!(!result.is_handle_visible);
    assert_eq!(0.0, result.translation_y);
    assert_eq!(1.0, result.opacity);
    assert_eq!(60.0, result.height_dip);
}

#[test]
fn resolve_with_null_monitor_uses_default_resolution_gracefully() {
    let result = Args::new(None, CAPSULE, DockProximityMode::DockDown, 520.0, WindowRect::EMPTY).resolve();

    assert_eq!(60.0, result.height_dip);
    // Default 1080 - 70 = 1010
    assert_eq!(1010.0, result.top_dip);
    assert!(!result.is_snapped);
}

#[test]
fn dock_down_when_window_is_on_different_monitor_does_not_snap() {
    // Window is located completely on secondary monitor (left: 1920 to 2500)
    let window_on_secondary = WindowRect::new(2000, 800, 2500, 1050);

    // primary monitor (left: 0 to 1920)
    let result = resolve_default(&primary_1080p(), CAPSULE, DockProximityMode::DockDown, 520.0, window_on_secondary);

    // Should not snap because window is not on this monitor!
    assert!(!result.is_snapped);
    assert_eq!(60.0, result.height_dip);
    assert_eq!(1010.0, result.top_dip);
}

#[test]
fn full_width_when_fullscreen_hides_dock() {
    let monitor = primary_1080p();
    let result = Args {
        fullscreen: true,
        hide_in_fullscreen: true,
        ..Args::new(Some(&monitor), FULL, DockProximityMode::Dodge, 600.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(result.is_hidden);
    assert_eq!(0.0, result.opacity);
    assert!(result.translation_y > 40.0);
    assert!(!result.is_handle_visible);
}

#[test]
fn full_width_when_fullscreen_and_hide_in_fullscreen_false_does_not_hide() {
    let monitor = primary_1080p();
    let result = Args {
        fullscreen: true,
        hide_in_fullscreen: false,
        ..Args::new(Some(&monitor), FULL, DockProximityMode::Dodge, 600.0, WindowRect::EMPTY)
    }
    .resolve();

    assert!(!result.is_hidden);
    assert_eq!(1.0, result.opacity);
    assert_eq!(0.0, result.translation_y);
}

#[test]
fn full_width_when_dodge_and_maximized_slides_offscreen() {
    let monitor = primary_1080p();
    let result = Args {
        maximized: true,
        mouse_at_edge: false,
        mouse_over_dock: false,
        ..Args::new(Some(&monitor), FULL, DockProximityMode::Dodge, 600.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(result.is_hidden);
    assert_eq!(0.0, result.opacity);
    assert!(result.translation_y > 40.0);
    assert!(!result.is_handle_visible);
}

#[test]
fn full_width_when_dodge_and_maximized_and_mouse_at_edge_slides_back_up() {
    let monitor = primary_1080p();
    let result = Args {
        maximized: true,
        mouse_at_edge: true,
        mouse_over_dock: false,
        ..Args::new(Some(&monitor), FULL, DockProximityMode::Dodge, 600.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(!result.is_hidden);
    assert_eq!(1.0, result.opacity);
    assert_eq!(0.0, result.translation_y);
}

#[test]
fn floating_capsule_when_fullscreen_hides_dock_without_handle() {
    let monitor = primary_1080p();
    let result = Args {
        fullscreen: true,
        hide_in_fullscreen: true,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::Dodge, 520.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(result.is_hidden);
    assert_eq!(0.0, result.opacity);
    assert!(result.translation_y > 60.0);
    assert!(!result.is_handle_visible);
}

#[test]
fn floating_capsule_when_maximized_and_hide_in_fullscreen_hides_dock() {
    let monitor = primary_1080p();
    // Çekil büyütülmüş pencerede gizler (Aşağı yapış gizlemez)
    let result = Args {
        maximized: true,
        hide_in_fullscreen: true,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::Dodge, 520.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(result.is_hidden);
    assert_eq!(0.0, result.opacity);
    assert!(result.translation_y > 60.0);
}

#[test]
fn floating_capsule_when_maximized_and_hide_in_fullscreen_and_mouse_at_edge_peeks() {
    let monitor = primary_1080p();
    let result = Args {
        maximized: true,
        mouse_at_edge: true,
        hide_in_fullscreen: true,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::DockDown, 520.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(!result.is_hidden);
    assert_eq!(1.0, result.opacity);
    assert_eq!(0.0, result.translation_y);
}

#[test]
fn floating_capsule_always_floating_never_snaps_hides_or_reserves() {
    // (maximized, l, t, r, b)
    let cases = [
        (false, 0, 0, 0, 0),         // pencere yok
        (true, 100, 100, 900, 700),  // uzakta, büyütülmüş
        (false, 100, 960, 900, 1075), // dock bölgesine binmiş
        (true, 0, 0, 1920, 1080),    // tam büyütülmüş
    ];
    let monitor = primary_1080p();
    for (maximized, l, t, r, b) in cases {
        let bounds = if l == 0 && t == 0 && r == 0 && b == 0 { WindowRect::default() } else { WindowRect::new(l, t, r, b) };
        let result = Args {
            maximized,
            hide_in_fullscreen: true,
            ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::AlwaysFloating, 500.0, bounds)
        }
        .resolve();

        let ctx = format!("({maximized}, {l}, {t}, {r}, {b})");
        assert_eq!(60.0, result.height_dip, "{ctx}");
        assert_eq!(1010.0, result.top_dip, "{ctx}"); // 1080 - 60 - 10
        assert_eq!(710.0, result.left_dip, "{ctx}");
        assert_eq!(500.0, result.width_dip, "{ctx}");
        assert_eq!(18.0, result.corner_radius_top_left, "{ctx}");
        assert_eq!(18.0, result.corner_radius_bottom_right, "{ctx}");
        assert!(result.has_uniform_radius(), "{ctx}");
        assert_eq!(0.0, result.translation_y, "{ctx}");
        assert_eq!(1.0, result.opacity, "{ctx}");
        assert!(!result.is_snapped, "{ctx}");
        assert!(!result.is_hidden, "{ctx}");
        assert!(!result.is_handle_visible, "{ctx}");
        assert_eq!(0, result.app_bar_reserved_height_dip, "{ctx}");
    }
}

#[test]
fn floating_capsule_always_floating_is_hidden_in_real_fullscreen() {
    let monitor = primary_1080p();
    let result = Args {
        maximized: true,
        fullscreen: true,
        hide_in_fullscreen: true,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::AlwaysFloating, 500.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(result.is_hidden);
    assert_eq!(0.0, result.opacity);
    assert!(result.translation_y > 60.0);
}

#[test]
fn floating_capsule_always_floating_stays_when_fullscreen_hiding_is_off() {
    let monitor = primary_1080p();
    let result = Args {
        maximized: true,
        fullscreen: true,
        hide_in_fullscreen: false,
        ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::AlwaysFloating, 500.0, WindowRect::new(0, 0, 1920, 1080))
    }
    .resolve();

    assert!(!result.is_hidden);
    assert_eq!(1010.0, result.top_dip);
    assert!(!result.is_snapped);
}

#[test]
fn floating_capsule_always_floating_ignores_mouse_at_edge() {
    let monitor = primary_1080p();
    let base = Args { maximized: true, ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::AlwaysFloating, 500.0, WindowRect::new(0, 0, 1920, 1080)) };
    let with_mouse = Args { mouse_at_edge: true, mouse_over_dock: true, ..base.clone() }.resolve();
    let without = base.resolve();

    assert_eq!(without, with_mouse);
}

#[test]
fn top_edge_floating_capsule_snaps_to_top_with_correct_corners() {
    // Approaching window at top (top <= 80)
    let approaching = WindowRect::new(100, 20, 800, 500);
    let monitor = primary_1080p();
    let result = Args { edge: DockEdge::Top, ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::DockDown, 500.0, approaching) }.resolve();

    assert!(result.is_snapped);
    assert_eq!(0.0, result.top_dip);
    assert_eq!(46.0, result.height_dip);
    assert_eq!(0.0, result.corner_radius_top_left);
    assert_eq!(0.0, result.corner_radius_top_right);
    assert_eq!(14.0, result.corner_radius_bottom_right);
    assert_eq!(14.0, result.corner_radius_bottom_left);
}

#[test]
fn left_edge_floating_capsule_snaps_to_left_with_correct_corners() {
    // Approaching window at left (left <= 80)
    let approaching = WindowRect::new(30, 200, 800, 700);
    let monitor = primary_1080p();
    let result = Args { edge: DockEdge::Left, ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::DockDown, 500.0, approaching) }.resolve();

    assert!(result.is_snapped);
    assert_eq!(0.0, result.left_dip);
    assert_eq!(46.0, result.width_dip);
    assert_eq!(500.0, result.height_dip);
    assert_eq!(0.0, result.corner_radius_top_left);
    assert_eq!(14.0, result.corner_radius_top_right);
    assert_eq!(14.0, result.corner_radius_bottom_right);
    assert_eq!(0.0, result.corner_radius_bottom_left);
}

#[test]
fn right_edge_floating_capsule_snaps_to_right_with_correct_corners() {
    // Approaching window at right (right >= 1920 - 80 = 1840)
    let approaching = WindowRect::new(1500, 200, 1900, 700);
    let monitor = primary_1080p();
    let result = Args { edge: DockEdge::Right, ..Args::new(Some(&monitor), CAPSULE, DockProximityMode::DockDown, 500.0, approaching) }.resolve();

    assert!(result.is_snapped);
    assert_eq!(1920.0 - 46.0, result.left_dip);
    assert_eq!(46.0, result.width_dip);
    assert_eq!(500.0, result.height_dip);
    assert_eq!(14.0, result.corner_radius_top_left);
    assert_eq!(0.0, result.corner_radius_top_right);
    assert_eq!(0.0, result.corner_radius_bottom_right);
    assert_eq!(14.0, result.corner_radius_bottom_left);
}

#[test]
fn full_width_hidden_translation_directs_away_from_edge() {
    // (edge, expectedTx, expectedTy)
    let cases = [
        (DockEdge::Bottom, 0.0, 58.0),
        (DockEdge::Top, 0.0, -58.0),
        (DockEdge::Left, -58.0, 0.0),
        (DockEdge::Right, 58.0, 0.0),
    ];
    let monitor = primary_1080p();
    for (edge, expected_tx, expected_ty) in cases {
        let result = Args {
            mouse_at_edge: false,
            mouse_over_dock: false,
            edge,
            ..Args::new(Some(&monitor), FULL, DockProximityMode::AlwaysHidden, 500.0, WindowRect::EMPTY)
        }
        .resolve();

        assert!(result.is_hidden, "{edge:?}");
        assert_eq!(expected_tx, result.translation_x, "{edge:?}");
        assert_eq!(expected_ty, result.translation_y, "{edge:?}");
    }
}

#[test]
fn full_width_length_percent_and_edge_offset_applied_correctly() {
    let monitor = primary_1080p();
    let result = Args {
        edge: DockEdge::Bottom,
        length_percent: 50,
        edge_offset: 12.0,
        ..Args::new(Some(&monitor), FULL, DockProximityMode::ReserveSpace, 0.0, WindowRect::EMPTY)
    }
    .resolve();

    assert_eq!(960.0, result.width_dip);
    assert_eq!(480.0, result.left_dip);
    assert_eq!(1080.0 - 12.0 - 48.0, result.top_dip);
    assert_eq!(48.0, result.height_dip);
}

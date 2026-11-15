//! `EdgeRevealAndFullscreenTests.cs`: `EdgeRevealTrackerTests` ve `DockEdgeRevealResolverTests`.
//! `FullscreenRuleStateTests` Shell modülünün testidir (`shell/tests/fullscreen_rule_tests.rs`).

use super::dock_position_resolver_tests::Args;
use crate::display::DisplayMonitor;
use crate::dock::{DockEdge, DockPositionResolver, DockProximityMode, DockStyleMode, EdgeRevealTracker};
use crate::shell::WindowRect;
use crate::time::TimeSpan;

fn ms(ms: i64) -> TimeSpan {
    TimeSpan::from_millis(ms)
}

#[test]
fn kenarda_beklemeden_acmaz_gecikme_sonra_acar() {
    let mut tracker = EdgeRevealTracker::new(ms(300));

    assert!(!tracker.update(true, false, ms(0)));
    assert!(!tracker.update(true, false, ms(100)));
    assert!(!tracker.update(true, false, ms(299)));
    assert!(tracker.update(true, false, ms(300)));
    assert!(tracker.is_armed());
}

#[test]
fn kenardan_ayrilinca_sifirlanir_yeniden_beklenir() {
    let mut tracker = EdgeRevealTracker::new(ms(300));
    tracker.update(true, false, ms(0));
    tracker.update(true, false, ms(400));
    assert!(tracker.is_armed());

    assert!(!tracker.update(false, false, ms(450)));
    assert!(!tracker.is_armed());

    assert!(!tracker.update(true, false, ms(500)));
    assert!(!tracker.update(true, false, ms(799)));
    assert!(tracker.update(true, false, ms(800)));
}

#[test]
fn fare_tusu_basiliyken_cikmaz_ve_sayaci_sifirlar() {
    let mut tracker = EdgeRevealTracker::new(ms(300));
    tracker.update(true, false, ms(0));

    // Pencere sürüklerken kenara inmek dock'u açmamalı.
    assert!(!tracker.update(true, true, ms(350)));
    assert!(!tracker.update(true, true, ms(900)));

    // Tuş bırakıldıktan sonra bekleme baştan başlar.
    assert!(!tracker.update(true, false, ms(1000)));
    assert!(tracker.update(true, false, ms(1300)));
}

#[test]
fn acikken_tusa_basilirsa_kapanir() {
    let mut tracker = EdgeRevealTracker::new(ms(300));
    tracker.update(true, false, ms(0));
    tracker.update(true, false, ms(300));

    assert!(!tracker.update(true, true, ms(350)));
}

#[test]
fn reset_silahi_bosaltir() {
    let mut tracker = EdgeRevealTracker::new(ms(0));
    assert!(tracker.update(true, false, ms(5)));

    tracker.reset();

    assert!(!tracker.is_armed());
}

#[test]
fn is_in_zone_monitor_basina_kenar_bolgesi() {
    // (x, y, left, right, bottom, revealed, dpi, expected)
    let cases = [
        (500, 1079, 0, 1920, 1080, false, 1.0, true),
        (500, 1076, 0, 1920, 1080, false, 1.0, true),
        (500, 1070, 0, 1920, 1080, false, 1.0, false),
        (500, 1005, 0, 1920, 1080, true, 1.0, true),
        (500, 990, 0, 1920, 1080, true, 1.0, false),
        (500, 970, 0, 1920, 1080, true, 1.5, true),
        (2000, 1079, 0, 1920, 1080, false, 1.0, false),
        (-1, 1079, 0, 1920, 1080, false, 1.0, false),
    ];
    for (x, y, left, right, bottom, revealed, dpi, expected) in cases {
        assert_eq!(
            expected,
            EdgeRevealTracker::is_in_zone(x, y, left, right, bottom, revealed, dpi),
            "({x}, {y}, {left}, {right}, {bottom}, {revealed}, {dpi})"
        );
    }
}

#[test]
fn is_in_zone_four_edges() {
    // (edge, x, y, revealed, expected)
    let cases = [
        (DockEdge::Top, 500, 2, false, true),
        (DockEdge::Top, 500, 10, false, false),
        (DockEdge::Top, 500, 50, true, true),
        (DockEdge::Left, 2, 500, false, true),
        (DockEdge::Left, 10, 500, false, false),
        (DockEdge::Left, 50, 500, true, true),
        (DockEdge::Right, 1918, 500, false, true),
        (DockEdge::Right, 1900, 500, false, false),
        (DockEdge::Right, 1850, 500, true, true),
        (DockEdge::Bottom, 500, 1078, false, true),
        (DockEdge::Bottom, 500, 1050, false, false),
        (DockEdge::Bottom, 500, 1010, true, true),
    ];
    let bounds = WindowRect::new(0, 0, 1920, 1080);
    for (edge, x, y, revealed, expected) in cases {
        assert_eq!(expected, EdgeRevealTracker::is_in_zone_with_edge(edge, x, y, bounds, revealed, 1.0, 1.0), "({edge:?}, {x}, {y}, {revealed})");
    }
}

// --- DockEdgeRevealResolverTests ---

fn monitor() -> DisplayMonitor {
    DisplayMonitor::new("m1", "M", WindowRect::new(0, 0, 1920, 1080), WindowRect::new(0, 0, 1920, 1080), true)
        .with_dpi(1.0, 1.0)
        .with_handle(1)
}

fn args(monitor: &DisplayMonitor, style: DockStyleMode, mode: DockProximityMode) -> Args<'_> {
    Args::new(Some(monitor), style, mode, 500.0, WindowRect::EMPTY)
}

#[test]
fn buyutulmus_pencere_dock_gizli_kenarda_acilir() {
    let m = monitor();
    let hidden = Args { maximized: true, mouse_at_edge: false, edge_reveal: true, ..args(&m, DockStyleMode::FloatingCapsule, DockProximityMode::Dodge) }.resolve();
    let revealed = Args { maximized: true, mouse_at_edge: true, edge_reveal: true, ..args(&m, DockStyleMode::FloatingCapsule, DockProximityMode::Dodge) }.resolve();

    assert!(hidden.is_hidden);
    assert!(!revealed.is_hidden);
}

#[test]
fn kenardan_acma_kapaliysa_buyutulmus_pencerede_fare_kenarda_olsa_da_gizli_kalir() {
    let m = monitor();
    let state = Args { maximized: true, mouse_at_edge: true, edge_reveal: false, ..args(&m, DockStyleMode::FloatingCapsule, DockProximityMode::Dodge) }.resolve();

    assert!(state.is_hidden);
}

#[test]
fn kapsul_asagi_yapis_buyutulmus_pencerede_gizlenmez_alta_yapisir_yer_ayirmaz() {
    let m = monitor();
    let state = Args { maximized: true, mouse_at_edge: false, edge_reveal: true, ..args(&m, DockStyleMode::FloatingCapsule, DockProximityMode::DockDown) }.resolve();

    assert!(!state.is_hidden);
    assert_eq!(1.0, state.opacity);
    assert_eq!(0.0, state.translation_y);
    assert!(state.is_snapped);
    assert!(!state.is_handle_visible);
    assert_eq!(DockPositionResolver::SNAPPED_HEIGHT_DIP, state.height_dip);
    assert_eq!(DockPositionResolver::SNAPPED_CORNER_RADIUS_DIP, state.corner_radius_top_left);
    assert_eq!(0.0, state.corner_radius_bottom_left);
    assert_eq!(1080.0 - DockPositionResolver::SNAPPED_HEIGHT_DIP, state.top_dip);
    assert_eq!(0, state.app_bar_reserved_height_dip);
}

#[test]
fn kapsul_asagi_yapis_tam_ekranda_yine_gizlenir() {
    let m = monitor();
    let state = Args { maximized: true, fullscreen: true, hide_in_fullscreen: true, ..args(&m, DockStyleMode::FloatingCapsule, DockProximityMode::DockDown) }.resolve();

    assert!(state.is_hidden);
}

#[test]
fn kapsul_cekil_ve_hep_gizli_buyutulmus_pencerede_gizli_kalir() {
    let m = monitor();
    for mode in [DockProximityMode::Dodge, DockProximityMode::AlwaysHidden] {
        let state = Args { maximized: true, mouse_at_edge: false, edge_reveal: true, ..args(&m, DockStyleMode::FloatingCapsule, mode) }.resolve();

        assert!(state.is_hidden, "{mode:?}");
    }
}

#[test]
fn tam_ekranda_kenarda_fare_olsa_da_dock_cikmaz() {
    let m = monitor();
    let modes = [
        DockProximityMode::DockDown,
        DockProximityMode::ReserveSpace,
        DockProximityMode::Dodge,
        DockProximityMode::AlwaysHidden,
        DockProximityMode::AlwaysFloating,
    ];
    for style in [DockStyleMode::FloatingCapsule, DockStyleMode::FullWidth] {
        for mode in modes {
            let state = Args {
                maximized: true,
                mouse_at_edge: true,
                mouse_over_dock: false,
                fullscreen: true,
                hide_in_fullscreen: true,
                edge_reveal: true,
                ..args(&m, style, mode)
            }
            .resolve();

            assert!(state.is_hidden, "{style:?}/{mode:?}");
        }
    }
}

#[test]
fn full_width_buyutulmus_pencere_kenarda_acilir() {
    let m = monitor();
    let state = Args { maximized: true, mouse_at_edge: true, edge_reveal: true, ..args(&m, DockStyleMode::FullWidth, DockProximityMode::DockDown) }.resolve();

    assert!(!state.is_hidden);
}

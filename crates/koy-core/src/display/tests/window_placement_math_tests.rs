use crate::display::*;
use crate::dock::{DockEdge, DockPositionResolver};
use crate::shell::WindowRect;

// Ana ekran %100, sağdaki ikinci ekran %150 ve farklı çözünürlükte; ikincisi sanal masaüstünde x=1920'den başlar.
fn main_mon() -> DisplayMonitor {
    DisplayMonitor::new("a", "Ana", WindowRect::new(0, 0, 1920, 1080), WindowRect::new(0, 0, 1920, 1040), true).with_dpi(1.0, 1.0)
}

fn side_mon() -> DisplayMonitor {
    DisplayMonitor::new("b", "Yan", WindowRect::new(1920, -200, 5760, 1960), WindowRect::new(1920, -200, 5760, 1920), false).with_dpi(1.5, 1.5)
}

#[test]
fn notch_is_centered_on_its_own_monitor_with_its_own_scale() {
    let rect = WindowPlacementMath::top_center(&side_mon(), 500.0, 260.0);

    assert_eq!(750, rect.width()); // 500 DIP * 1.5
    assert_eq!(390, rect.height()); // 260 DIP * 1.5
    assert_eq!(1920 + (3840 - 750) / 2, rect.left);
    assert_eq!(-200, rect.top);
}

#[test]
fn center_places_the_window_in_the_middle_of_its_own_monitor() {
    let rect = WindowPlacementMath::center(&side_mon(), 200.0, 100.0);

    assert_eq!(300, rect.width());
    assert_eq!(150, rect.height());
    assert_eq!(1920 + (3840 - 300) / 2, rect.left);
    assert_eq!(-200 + (2160 - 150) / 2, rect.top);
}

#[test]
fn notch_on_main_monitor_uses_scale_one() {
    let rect = WindowPlacementMath::top_center(&main_mon(), 500.0, 260.0);

    assert_eq!(WindowRect::new(710, 0, 1210, 260), rect);
}

#[test]
fn capsule_dock_sits_at_bottom_center_with_scaled_margin() {
    let rect = WindowPlacementMath::bottom_center(&side_mon(), 600.0, 60.0, 10.0);

    assert_eq!(1960 - 15, rect.bottom);
    assert_eq!(90, rect.height());
    assert_eq!(900, rect.width());
    assert_eq!(1920 + (3840 - 900) / 2, rect.left);
}

#[test]
fn capsule_dock_never_exceeds_the_monitor_width() {
    let rect = WindowPlacementMath::bottom_center(&main_mon(), 5000.0, 60.0, 0.0);

    assert_eq!(1920, rect.width());
    assert_eq!(0, rect.left);
}

#[test]
fn full_width_dock_spans_the_monitor_and_hugs_the_bottom() {
    let rect = WindowPlacementMath::bottom_full_width(&side_mon(), DockPositionResolver::FULL_WIDTH_HEIGHT_DIP);

    assert_eq!(1920, rect.left);
    assert_eq!(5760, rect.right);
    assert_eq!(1960, rect.bottom);
    assert_eq!(72, rect.height()); // 48 DIP * 1.5
}

#[test]
fn reserved_pixels_equal_the_window_height_at_every_scale() {
    let rows = [(1.0, 48), (1.25, 60), (1.5, 72), (1.75, 84), (2.0, 96), (1.1, 53), (1.05, 50)];
    for (scale, expected) in rows {
        let monitor = DisplayMonitor::new("m", "M", WindowRect::new(0, 0, 2560, 1440), WindowRect::new(0, 0, 2560, 1440), true).with_dpi(scale, scale);
        let window = WindowPlacementMath::bottom_full_width(&monitor, DockPositionResolver::FULL_WIDTH_HEIGHT_DIP);

        assert_eq!(expected, window.height(), "ölçek {scale}");
        // AppBar aynı fonksiyondan geçer: ayrılan pay pencere yüksekliğine tam eşittir.
        assert_eq!(window.height(), WindowPlacementMath::to_pixels(DockPositionResolver::FULL_WIDTH_HEIGHT_DIP, scale), "ölçek {scale}");
    }
}

#[test]
fn to_pixels_rounds_half_up_and_guards_invalid_scale() {
    // 80.5 yukarı yuvarlanır (eski kesme 80, banker 80 verirdi); 57.5; geçersiz ölçek 1 sayılır.
    let rows = [(46.0, 1.75, 81), (46.0, 1.25, 58), (48.0, 0.0, 48)];
    for (dip, scale, expected) in rows {
        assert_eq!(expected, WindowPlacementMath::to_pixels(dip, scale), "{dip} {scale}");
    }
}

#[test]
fn top_center_offset_is_scaled_by_the_target_monitor_and_keeps_the_size() {
    // (ölçek, ofset DIP, beklenen ofset piksel); 1.5 ve 9 DIP: 13.5 yukarı yuvarlanır.
    let rows = [(1.0, 0.0, 0), (1.0, 9.0, 9), (1.5, 0.0, 0), (1.5, 9.0, 14), (2.0, 9.0, 18)];
    for (scale, offset_dip, expected_offset_px) in rows {
        let monitor = DisplayMonitor::new("m", "M", WindowRect::new(100, -50, 3940, 2110), WindowRect::new(100, -50, 3940, 2070), false).with_dpi(scale, scale);

        let rect = WindowPlacementMath::top_center_with_offset(&monitor, 640.0, 364.0, offset_dip);

        assert_eq!(-50 + expected_offset_px, rect.top, "{scale} {offset_dip}");
        assert_eq!(WindowPlacementMath::to_pixels(364.0, scale), rect.height());
        assert_eq!(WindowPlacementMath::to_pixels(640.0, scale), rect.width());
        assert_eq!(WindowPlacementMath::top_center(&monitor, 640.0, 364.0).left, rect.left);
    }
}

#[test]
fn top_center_offset_uses_the_second_monitors_own_scale_not_the_main_one() {
    let on_main = WindowPlacementMath::top_center_with_offset(&main_mon(), 640.0, 364.0, 9.0);
    let on_side = WindowPlacementMath::top_center_with_offset(&side_mon(), 640.0, 364.0, 9.0);

    assert_eq!(9, on_main.top - main_mon().bounds.top);
    assert_eq!(14, on_side.top - side_mon().bounds.top);
}

#[test]
fn edge_bar_full_length_places_on_all_edges() {
    let rows = [
        (DockEdge::Bottom, 0, 1032, 1920, 48),
        (DockEdge::Top, 0, 0, 1920, 48),
        (DockEdge::Left, 0, 0, 48, 1080),
        (DockEdge::Right, 1872, 0, 48, 1080),
    ];
    for (edge, exp_x, exp_y, exp_w, exp_h) in rows {
        let rect = WindowPlacementMath::edge_bar(&main_mon(), edge, 48.0, 100, 0.0);

        assert_eq!(exp_x, rect.left, "{edge:?}");
        assert_eq!(exp_y, rect.top, "{edge:?}");
        assert_eq!(exp_w, rect.width(), "{edge:?}");
        assert_eq!(exp_h, rect.height(), "{edge:?}");
    }
}

#[test]
fn edge_bar_with_length_percent_and_offset() {
    let rect = WindowPlacementMath::edge_bar(&main_mon(), DockEdge::Top, 48.0, 50, 10.0);

    assert_eq!(480, rect.left);
    assert_eq!(960, rect.width());
    assert_eq!(10, rect.top);
    assert_eq!(48, rect.height());
}

#[test]
fn edge_capsule_left_edge_places_vertically_centered() {
    let rect = WindowPlacementMath::edge_capsule(&main_mon(), DockEdge::Left, 500.0, 60.0, 10.0);

    assert_eq!(10, rect.left);
    assert_eq!(60, rect.width());
    assert_eq!(500, rect.height());
    assert_eq!((1080 - 500) / 2, rect.top);
}

#[test]
fn edge_capsule_right_edge_places_vertically_centered() {
    let rect = WindowPlacementMath::edge_capsule(&main_mon(), DockEdge::Right, 500.0, 60.0, 10.0);

    assert_eq!(1920 - 10 - 60, rect.left);
    assert_eq!(60, rect.width());
    assert_eq!(500, rect.height());
    assert_eq!((1080 - 500) / 2, rect.top);
}

#[test]
fn edge_capsule_top_edge_places_horizontally_centered() {
    let rect = WindowPlacementMath::edge_capsule(&main_mon(), DockEdge::Top, 500.0, 60.0, 10.0);

    assert_eq!((1920 - 500) / 2, rect.left);
    assert_eq!(10, rect.top);
    assert_eq!(500, rect.width());
    assert_eq!(60, rect.height());
}

#[test]
fn edge_capsule_bottom_edge_places_horizontally_centered() {
    let rect = WindowPlacementMath::edge_capsule(&main_mon(), DockEdge::Bottom, 500.0, 60.0, 10.0);

    assert_eq!((1920 - 500) / 2, rect.left);
    assert_eq!(1080 - 10 - 60, rect.top);
    assert_eq!(500, rect.width());
    assert_eq!(60, rect.height());
}

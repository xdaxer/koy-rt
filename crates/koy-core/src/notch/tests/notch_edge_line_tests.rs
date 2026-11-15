use crate::display::DisplayMonitor;
use crate::notch::{NotchEdgeLine, NotchEdgeLineColor};
use crate::shell::WindowRect;

fn monitor(bounds: WindowRect, dpi: f64) -> DisplayMonitor {
    DisplayMonitor::new("id", "Ekran", bounds, bounds, true).with_dpi(dpi, dpi)
}

#[test]
fn spans_full_monitor_width_at_top_edge() {
    let rect = NotchEdgeLine::bounds(&monitor(WindowRect::new(0, 0, 1920, 1080), 1.0), NotchEdgeLine::DEFAULT_THICKNESS);

    assert_eq!(WindowRect::new(0, 0, 1920, 2), rect);
}

#[test]
fn thickness_scales_with_dpi_and_follows_monitor_offset() {
    let rect = NotchEdgeLine::bounds(&monitor(WindowRect::new(-2560, -200, 0, 1240), 1.5), 2);

    assert_eq!(WindowRect::new(-2560, -200, 0, -197), rect);
}

#[test]
fn thickness_is_clamped_to_range() {
    let rows = [(1, 1), (5, 5), (20, 20), (0, 1), (-3, 1), (99, 20)];
    for (requested, expected_height) in rows {
        assert_eq!(expected_height, NotchEdgeLine::bounds(&monitor(WindowRect::new(0, 0, 100, 100), 1.0), requested).height(), "{requested}");
    }
}

#[test]
fn thin_line_never_collapses_below_one_pixel() {
    assert_eq!(1, NotchEdgeLine::bounds(&monitor(WindowRect::new(0, 0, 100, 100), 0.5), 1).height());
}

#[test]
fn color_reads_theme_resource() {
    assert_eq!("KoyNotchSurface", NotchEdgeLine::resource_key(NotchEdgeLineColor::NotchSurface));
    assert_eq!("KoyAccent", NotchEdgeLine::resource_key(NotchEdgeLineColor::Accent));
}

#[test]
fn hides_only_when_notch_hides_in_fullscreen() {
    let rows = [(false, true, true), (true, true, false), (true, false, true)];
    for (fullscreen, hide_in_fullscreen, expected) in rows {
        assert_eq!(expected, NotchEdgeLine::is_visible(fullscreen, hide_in_fullscreen), "{fullscreen} {hide_in_fullscreen}");
    }
}

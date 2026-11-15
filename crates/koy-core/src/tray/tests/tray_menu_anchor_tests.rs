use crate::shell::WindowRect;
use crate::tray::TrayMenuAnchor;

const DOCK: WindowRect = WindowRect::new(500, 1000, 1400, 1080);

#[test]
fn point_above_dock_is_unchanged() {
    assert_eq!((800, 900), TrayMenuAnchor::clamp_above_dock(800, 900, DOCK));
}

#[test]
fn point_inside_dock_moves_above_top_edge() {
    assert_eq!((800, 999), TrayMenuAnchor::clamp_above_dock(800, 1040, DOCK));
}

#[test]
fn point_beside_dock_is_unchanged() {
    assert_eq!((200, 1040), TrayMenuAnchor::clamp_above_dock(200, 1040, DOCK));
}

#[test]
fn empty_dock_rect_is_ignored() {
    assert_eq!((800, 1040), TrayMenuAnchor::clamp_above_dock(800, 1040, WindowRect::EMPTY));
}

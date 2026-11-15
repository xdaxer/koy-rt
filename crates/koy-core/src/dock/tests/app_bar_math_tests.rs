use crate::dock::{AppBarMath, AppBarOwnership, DockEdge};
use crate::shell::WindowRect;

const MONITOR: WindowRect = WindowRect::new(0, 0, 1920, 1080);

#[test]
fn owned_edge_ignores_the_upward_push_of_the_explorer_taskbar() {
    // QUERYPOS Gezgin'in 48 px'ini sayıp alt sınırı 1032'ye çekti: 46 px'lik pay 986..1032 olurdu (94 px toplam).
    let pushed = WindowRect::new(0, 986, 1920, 1032);

    let r = AppBarMath::final_rect(MONITOR, 46, pushed, true);

    assert_eq!(WindowRect::new(0, 1034, 1920, 1080), r);
}

#[test]
fn owned_edge_always_hugs_the_monitor_bottom() {
    for (height, expected_top) in [(46, 1034), (48, 1032), (60, 1020)] {
        let r = AppBarMath::final_rect(MONITOR, height, WindowRect::new(0, 0, 1920, 900), true);

        assert_eq!(1080, r.bottom, "yükseklik {height}");
        assert_eq!(expected_top, r.top, "yükseklik {height}");
        assert_eq!(0, r.left, "yükseklik {height}");
        assert_eq!(1920, r.right, "yükseklik {height}");
    }
}

#[test]
fn visible_taskbar_stacks_above_what_windows_proposed() {
    let queried = WindowRect::new(0, 0, 1920, 1032);

    let r = AppBarMath::final_rect(MONITOR, 46, queried, false);

    assert_eq!(WindowRect::new(0, 986, 1920, 1032), r);
}

#[test]
fn secondary_monitor_uses_its_own_bounds() {
    let second = WindowRect::new(1920, -200, 3840, 880);

    let r = AppBarMath::final_rect(second, 48, WindowRect::new(1920, 0, 3840, 832), true);

    assert_eq!(WindowRect::new(1920, 832, 3840, 880), r);
}

#[test]
fn top_edge_allocates_from_queried_top() {
    let queried = WindowRect::new(0, 0, 1920, 1080);
    let r = AppBarMath::final_rect_with_edge(DockEdge::Top, MONITOR, 48, queried, false);
    assert_eq!(WindowRect::new(0, 0, 1920, 48), r);
}

#[test]
fn left_edge_allocates_from_queried_left() {
    let queried = WindowRect::new(0, 0, 1920, 1080);
    let r = AppBarMath::final_rect_with_edge(DockEdge::Left, MONITOR, 48, queried, false);
    assert_eq!(WindowRect::new(0, 0, 48, 1080), r);
}

#[test]
fn right_edge_allocates_from_queried_right() {
    let queried = WindowRect::new(0, 0, 1920, 1080);
    let r = AppBarMath::final_rect_with_edge(DockEdge::Right, MONITOR, 48, queried, false);
    assert_eq!(WindowRect::new(1920 - 48, 0, 1920, 1080), r);
}

#[test]
fn needs_reassert_works_on_all_edges() {
    // Top edge: expected work.Top is 48. If work.Top < 48 (e.g. 0), needs reassert
    assert!(AppBarOwnership::needs_reassert_with_edge(DockEdge::Top, MONITOR, WindowRect::new(0, 0, 1920, 1080), 48, false, None));
    assert!(!AppBarOwnership::needs_reassert_with_edge(DockEdge::Top, MONITOR, WindowRect::new(0, 48, 1920, 1080), 48, false, None));

    // Left edge: expected work.Left is 48. If work.Left < 48 (e.g. 0), needs reassert
    assert!(AppBarOwnership::needs_reassert_with_edge(DockEdge::Left, MONITOR, WindowRect::new(0, 0, 1920, 1080), 48, false, None));
    assert!(!AppBarOwnership::needs_reassert_with_edge(DockEdge::Left, MONITOR, WindowRect::new(48, 0, 1920, 1080), 48, false, None));

    // Right edge: expected work.Right is 1920 - 48 = 1872. If work.Right > 1872 (e.g. 1920), needs reassert
    assert!(AppBarOwnership::needs_reassert_with_edge(DockEdge::Right, MONITOR, WindowRect::new(0, 0, 1920, 1080), 48, false, None));
    assert!(!AppBarOwnership::needs_reassert_with_edge(DockEdge::Right, MONITOR, WindowRect::new(0, 0, 1872, 1080), 48, false, None));
}

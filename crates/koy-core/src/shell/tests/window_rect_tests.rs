//! `VisibilityControllerTests.WindowRect_GeometryMethodsWorkCorrectly`: `WindowRect` Shell'e ait olduğu için burada.

use crate::shell::WindowRect;

#[test]
fn window_rect_geometry_methods_work_correctly() {
    let rect = WindowRect::new(10, 20, 110, 120);
    assert_eq!(100, rect.width());
    assert_eq!(100, rect.height());
    assert!(!rect.is_empty());

    assert!(rect.contains(50, 50));
    assert!(!rect.contains(5, 50));
    assert!(!rect.contains(50, 150));

    let overlapping = WindowRect::new(50, 50, 150, 150);
    assert!(rect.intersects(overlapping));

    let disjoint = WindowRect::new(200, 200, 300, 300);
    assert!(!rect.intersects(disjoint));

    let empty = WindowRect::EMPTY;
    assert_eq!(0, empty.width());
    assert_eq!(0, empty.height());
    assert!(empty.is_empty());
}

use crate::dock::ThumbnailFit;
use crate::shell::WindowRect;

#[test]
fn fit_landscape_source_in_portrait_box_fills_width_and_centers_vertically() {
    let box_rect = WindowRect::new(0, 0, 100, 200);

    let result = ThumbnailFit::fit(1600, 900, box_rect);

    assert_eq!(WindowRect::new(0, 72, 100, 128), result);
}

#[test]
fn fit_portrait_source_in_landscape_box_fills_height_and_centers_horizontally() {
    let box_rect = WindowRect::new(10, 20, 474, 284); // 464 × 264

    let result = ThumbnailFit::fit(600, 1200, box_rect);

    assert_eq!(264, result.height());
    assert_eq!(132, result.width());
    assert_eq!(10 + (464 - 132) / 2, result.left);
    assert_eq!(20, result.top);
}

#[test]
fn fit_same_aspect_ratio_fills_box_exactly() {
    let box_rect = WindowRect::new(16, 40, 480, 301); // 464 × 261

    let result = ThumbnailFit::fit(1920, 1080, box_rect);

    assert_eq!(box_rect.width(), result.width());
    assert!((260..=261).contains(&result.height()));
    assert_eq!(16, result.left);
}

#[test]
fn fit_small_source_is_not_upscaled() {
    let box_rect = WindowRect::new(0, 0, 400, 300);

    let result = ThumbnailFit::fit(200, 100, box_rect);

    assert_eq!(WindowRect::new(100, 100, 300, 200), result);
}

#[test]
fn fit_unknown_source_size_returns_box() {
    for (width, height) in [(0, 900), (1600, 0), (-5, -5)] {
        let box_rect = WindowRect::new(5, 5, 105, 65);

        assert_eq!(box_rect, ThumbnailFit::fit(width, height, box_rect), "({width}, {height})");
    }
}

#[test]
fn fit_empty_box_returns_empty() {
    assert_eq!(WindowRect::EMPTY, ThumbnailFit::fit(1920, 1080, WindowRect::new(10, 10, 10, 50)));
}

#[test]
fn fit_result_always_inside_box() {
    let box_rect = WindowRect::new(3, 7, 143, 77);

    for (w, h) in [(1, 1), (3840, 160), (160, 3840), (1366, 768), (141, 71)] {
        let r = ThumbnailFit::fit(w, h, box_rect);
        assert!(
            r.left >= box_rect.left && r.top >= box_rect.top && r.right <= box_rect.right && r.bottom <= box_rect.bottom,
            "{w}x{h} -> {r:?}"
        );
        assert!(!r.is_empty());
    }
}

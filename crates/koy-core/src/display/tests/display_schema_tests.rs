use crate::display::*;
use crate::shell::WindowRect;

fn mon(id: &str, l: i32, t: i32, r: i32, b: i32, primary: bool) -> DisplayMonitor {
    DisplayMonitor::new(id, id, WindowRect::new(l, t, r, b), WindowRect::new(l, t, r, b), primary)
}

fn assert_close(expected: f64, actual: f64) {
    assert!((expected - actual).abs() < 1e-6, "beklenen {expected}, gelen {actual}");
}

#[test]
fn empty_or_zero_area_gives_empty_layout() {
    // C#'ta Assert.Same: Rust'ta paylaşılan örnek yok, eşitlikle denetlenir.
    assert_eq!(SchemaLayout::empty(), DisplaySchema::compute(&[], 500.0, 120.0, 8.0));
    assert_eq!(SchemaLayout::empty(), DisplaySchema::compute(&[mon("a", 0, 0, 100, 100, false)], 0.0, 120.0, 8.0));
}

#[test]
fn two_side_by_side_monitors_keep_proportions_and_fit() {
    let monitors = [mon("a", 0, 0, 1920, 1080, true), mon("b", 1920, 0, 3840, 1080, false)];

    let layout = DisplaySchema::compute(&monitors, 400.0, 200.0, 0.0);

    assert_close(400.0, layout.width);
    assert_close(112.5, layout.height);
    assert_eq!(2, layout.tiles.len());
    assert_close(0.0, layout.tiles[0].left);
    assert_close(200.0, layout.tiles[1].left);
    assert_close(200.0, layout.tiles[0].width);
    assert_close(1920.0 / 1080.0, layout.tiles[0].width / layout.tiles[0].height);
}

#[test]
fn layout_is_never_scaled_up() {
    let layout = DisplaySchema::compute(&[mon("a", 0, 0, 100, 50, true)], 5000.0, 5000.0, 0.0);

    assert_close(100.0, layout.width);
    assert_close(50.0, layout.height);
}

#[test]
fn negative_coordinates_are_normalized_to_the_top_left() {
    let monitors = [mon("main", 0, 0, 1920, 1080, true), mon("left", -1920, -300, 0, 780, false)];

    let layout = DisplaySchema::compute(&monitors, 384.0, 216.0, 0.0);

    let left = layout.tiles.iter().find(|t| t.id == "left").unwrap();
    let main = layout.tiles.iter().find(|t| t.id == "main").unwrap();
    assert_close(0.0, left.left);
    assert_close(0.0, left.top);
    assert_close(192.0, main.left);
    assert_close(30.0, main.top);
    for t in &layout.tiles {
        let right = t.left + t.width;
        assert!((0.0..=layout.width + 1e-6).contains(&right), "{right}");
    }
}

#[test]
fn stacked_monitors_are_scaled_by_height() {
    let monitors = [mon("a", 0, 0, 1000, 1000, true), mon("b", 0, 1000, 1000, 2000, false)];

    let layout = DisplaySchema::compute(&monitors, 500.0, 100.0, 0.0);

    assert_close(100.0, layout.height);
    assert_close(50.0, layout.width);
}

#[test]
fn gap_shrinks_tiles_so_adjacent_monitors_do_not_touch() {
    let monitors = [mon("a", 0, 0, 1000, 500, true), mon("b", 1000, 0, 2000, 500, false)];

    let layout = DisplaySchema::compute(&monitors, 200.0, 200.0, 8.0);

    assert!(layout.tiles[1].left - (layout.tiles[0].left + layout.tiles[0].width) >= 7.99);
}

#[test]
fn numbering_puts_primary_first_then_left_to_right_then_top_to_bottom() {
    let monitors = [
        mon("sag", 1920, 0, 3840, 1080, false),
        mon("ana", 0, 0, 1920, 1080, true),
        mon("sol-alt", -1920, 500, 0, 1580, false),
        mon("sol-ust", -1920, 0, 0, 500, false),
    ];

    let ids: Vec<String> = DisplaySchema::order(&monitors).into_iter().map(|m| m.id).collect();

    assert_eq!(vec!["ana", "sol-ust", "sol-alt", "sag"], ids);
    assert_eq!(1, DisplaySchema::number_of(&monitors, "ana"));
    assert_eq!(4, DisplaySchema::number_of(&monitors, "SAG"));
    assert_eq!(0, DisplaySchema::number_of(&monitors, "yok"));
}

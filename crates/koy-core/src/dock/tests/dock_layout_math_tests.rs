use super::dock_position_resolver_tests::resolve_default;
use super::TempDir;
use crate::display::DisplayMonitor;
use crate::dock::{DockEdge, DockLayoutLimits, DockLayoutMath, DockPositionResolver, DockProximityMode, DockRectDip, DockStyleMode};
use crate::settings::{AppSettings, DockSettings, SettingsStore};
use crate::shell::WindowRect;

const SCREEN_1080P: DockRectDip = DockRectDip::new(0.0, 0.0, 1920.0, 1080.0);

const ALL_EDGES: [DockEdge; 4] = [DockEdge::Bottom, DockEdge::Top, DockEdge::Left, DockEdge::Right];

#[test]
fn default_bottom_layout_matches_full_width_dock_geometry() {
    // Varsayılan ayarlar (alt, %100, ofset 0, dolgu 0) bugünkü tam genişlik dock ile aynı kutuyu vermeli.
    let monitor = DisplayMonitor::new("M1", "Primary", WindowRect::new(0, 0, 1920, 1080), WindowRect::new(0, 0, 1920, 1080), true);

    let resolved = resolve_default(&monitor, DockStyleMode::FullWidth, DockProximityMode::ReserveSpace, 1920.0, WindowRect::EMPTY);

    let layout = DockLayoutMath::compute(
        DockEdge::Bottom,
        DockLayoutLimits::DEFAULT_LENGTH_PERCENT,
        DockLayoutLimits::DEFAULT_EDGE_OFFSET_DIP,
        DockLayoutLimits::DEFAULT_INNER_PADDING_DIP,
        DockPositionResolver::FULL_WIDTH_HEIGHT_DIP,
        SCREEN_1080P,
    );

    assert_eq!(resolved.left_dip, layout.outer.left);
    assert_eq!(resolved.top_dip, layout.outer.top);
    assert_eq!(resolved.width_dip, layout.outer.width);
    assert_eq!(resolved.height_dip, layout.outer.height);
    assert_eq!(layout.outer, layout.content);
}

#[test]
fn bottom_edge_sits_on_work_area_bottom_minus_offset_and_thickness() {
    let result = DockLayoutMath::compute(DockEdge::Bottom, 100, 12.0, 0.0, 48.0, SCREEN_1080P);

    assert_eq!(DockRectDip::new(0.0, 1080.0 - 12.0 - 48.0, 1920.0, 48.0), result.outer);
}

#[test]
fn top_edge_sits_on_work_area_top_plus_offset() {
    let result = DockLayoutMath::compute(DockEdge::Top, 100, 8.0, 0.0, 48.0, SCREEN_1080P);

    assert_eq!(DockRectDip::new(0.0, 8.0, 1920.0, 48.0), result.outer);
}

#[test]
fn left_edge_is_full_height_and_thickness_wide() {
    let result = DockLayoutMath::compute(DockEdge::Left, 100, 0.0, 0.0, 60.0, SCREEN_1080P);

    assert_eq!(DockRectDip::new(0.0, 0.0, 60.0, 1080.0), result.outer);
}

#[test]
fn right_edge_sits_on_work_area_right_minus_offset_and_thickness() {
    let result = DockLayoutMath::compute(DockEdge::Right, 100, 10.0, 0.0, 60.0, SCREEN_1080P);

    assert_eq!(DockRectDip::new(1920.0 - 10.0 - 60.0, 0.0, 60.0, 1080.0), result.outer);
}

#[test]
fn length_is_a_centered_percentage_of_the_parallel_side() {
    let bottom = DockLayoutMath::compute(DockEdge::Bottom, 50, 0.0, 0.0, 48.0, SCREEN_1080P);
    assert_eq!(DockRectDip::new(480.0, 1032.0, 960.0, 48.0), bottom.outer);

    let left = DockLayoutMath::compute(DockEdge::Left, 50, 0.0, 0.0, 48.0, SCREEN_1080P);
    assert_eq!(DockRectDip::new(0.0, 270.0, 48.0, 540.0), left.outer);
}

#[test]
fn work_area_with_nonzero_origin_is_respected() {
    // İkinci monitör: çalışma alanı x=1920'den başlar, üstten 40 px görev çubuğu var.
    let secondary = DockRectDip::new(1920.0, 40.0, 1440.0, 1400.0);

    let bottom = DockLayoutMath::compute(DockEdge::Bottom, 100, 0.0, 0.0, 48.0, secondary);
    assert_eq!(DockRectDip::new(1920.0, 40.0 + 1400.0 - 48.0, 1440.0, 48.0), bottom.outer);

    let right = DockLayoutMath::compute(DockEdge::Right, 100, 0.0, 0.0, 48.0, secondary);
    assert_eq!(DockRectDip::new(1920.0 + 1440.0 - 48.0, 40.0, 48.0, 1400.0), right.outer);
}

#[test]
fn inner_padding_insets_the_content_rectangle_only() {
    let result = DockLayoutMath::compute(DockEdge::Bottom, 100, 0.0, 6.0, 48.0, SCREEN_1080P);

    assert_eq!(DockRectDip::new(0.0, 1032.0, 1920.0, 48.0), result.outer);
    assert_eq!(DockRectDip::new(6.0, 1038.0, 1908.0, 36.0), result.content);
}

#[test]
fn inset_never_produces_negative_size() {
    let content = DockLayoutMath::compute(DockEdge::Bottom, 100, 0.0, 24.0, 20.0, SCREEN_1080P).content;

    assert_eq!(0.0, content.height);
    assert!(content.width >= 0.0);
}

#[test]
fn content_stays_inside_outer_when_padding_exceeds_half_the_thickness() {
    let result = DockLayoutMath::compute(DockEdge::Bottom, 100, 0.0, 24.0, 20.0, SCREEN_1080P);

    assert!(result.content.top >= result.outer.top);
    assert!(result.content.bottom() <= result.outer.bottom());
    assert_eq!(result.outer.top + 10.0, result.content.top);
}

#[test]
fn monitor_with_negative_coordinates_is_respected() {
    // Birincil monitörün solundaki ve üstündeki ekran negatif koordinatta durur.
    let left = DockRectDip::new(-1920.0, -200.0, 1920.0, 1080.0);

    let bottom = DockLayoutMath::compute(DockEdge::Bottom, 50, 4.0, 0.0, 48.0, left);
    assert_eq!(DockRectDip::new(-1920.0 + 480.0, -200.0 + 1080.0 - 4.0 - 48.0, 960.0, 48.0), bottom.outer);

    let right = DockLayoutMath::compute(DockEdge::Right, 100, 0.0, 0.0, 48.0, left);
    assert_eq!(DockRectDip::new(-48.0, -200.0, 48.0, 1080.0), right.outer);
}

#[test]
fn degenerate_work_area_never_gives_negative_or_non_finite_sizes() {
    let cases = [(0.0, 0.0), (-100.0, -50.0), (f64::NAN, f64::NAN), (f64::INFINITY, 100.0)];
    for (width, height) in cases {
        for edge in ALL_EDGES {
            let r = DockLayoutMath::compute(edge, 100, 10.0, 6.0, 48.0, DockRectDip::new(0.0, 0.0, width, height));

            for v in [r.outer.left, r.outer.top, r.outer.width, r.outer.height, r.content.width, r.content.height] {
                assert!(v.is_finite(), "{edge:?} ({width}, {height}): sonlu olmalı");
            }
            assert!(r.outer.width >= 0.0 && r.outer.height >= 0.0 && r.content.width >= 0.0 && r.content.height >= 0.0);
        }
    }
}

#[test]
fn invalid_thickness_gives_zero_thickness_not_nan() {
    for thickness in [f64::NAN, -30.0] {
        let r = DockLayoutMath::compute(DockEdge::Bottom, 100, 0.0, 0.0, thickness, SCREEN_1080P);

        assert_eq!(0.0, r.outer.height, "{thickness}");
        assert_eq!(1080.0, r.outer.top, "{thickness}");
    }
}

#[test]
#[ignore = "C# `(DockEdge)7` / `(DockEdge)(-1)` Rust'ta kurulamaz: enum yalnız tanımlı değerleri taşır (settings.json yolu aşağıdaki testte sınanır)"]
fn undefined_edge_number_is_treated_as_bottom() {}

#[test]
fn undefined_edge_number_in_settings_json_loads_as_bottom() {
    let dir = TempDir::new("koy-dock-layout-");
    std::fs::write(dir.settings_path(), "{ \"Version\": 4, \"Dock\": { \"Edge\": 7, \"LengthPercent\": 40 } }").unwrap();

    let back = SettingsStore::new(dir.settings_path()).load().dock;

    assert_eq!(DockEdge::Bottom, back.edge);
    assert_eq!(40, back.length_percent);
}

#[test]
fn out_of_range_length_is_clamped_into_limits() {
    let too_short = DockLayoutMath::compute(DockEdge::Bottom, 0, 0.0, 0.0, 48.0, SCREEN_1080P);
    assert!((1920.0 * 0.10 - too_short.outer.width).abs() < 1e-6);

    let too_long = DockLayoutMath::compute(DockEdge::Bottom, 500, 0.0, 0.0, 48.0, SCREEN_1080P);
    assert_eq!(1920.0, too_long.outer.width);
}

#[test]
fn out_of_range_offset_and_padding_are_clamped() {
    let result = DockLayoutMath::compute(DockEdge::Top, 100, 10_000.0, 10_000.0, 48.0, SCREEN_1080P);

    assert_eq!(DockLayoutLimits::MAX_EDGE_OFFSET_DIP, result.outer.top);
    assert_eq!(DockLayoutLimits::MAX_INNER_PADDING_DIP, result.content.left);
}

#[test]
fn negative_offset_and_padding_fall_back_to_zero() {
    let result = DockLayoutMath::compute(DockEdge::Top, 100, -5.0, -3.0, 48.0, SCREEN_1080P);

    assert_eq!(0.0, result.outer.top);
    assert_eq!(0.0, result.content.left);
}

#[test]
fn non_finite_values_use_defaults() {
    assert_eq!(DockLayoutLimits::DEFAULT_EDGE_OFFSET_DIP, DockLayoutLimits::clamp_edge_offset_dip(f64::NAN));
    assert_eq!(DockLayoutLimits::DEFAULT_INNER_PADDING_DIP, DockLayoutLimits::clamp_inner_padding_dip(f64::INFINITY));
    assert_eq!(DockLayoutLimits::DEFAULT_ICON_SPACING_DIP, DockLayoutLimits::clamp_icon_spacing_dip(f64::NAN));
}

#[test]
fn icon_spacing_is_clamped_to_its_range() {
    assert_eq!(0.0, DockLayoutLimits::clamp_icon_spacing_dip(-1.0));
    assert_eq!(DockLayoutLimits::MAX_ICON_SPACING_DIP, DockLayoutLimits::clamp_icon_spacing_dip(99.0));
    assert_eq!(6.0, DockLayoutLimits::clamp_icon_spacing_dip(6.0));
}

#[test]
fn default_dock_settings_keep_bottom_edge_and_full_length() {
    let dock = DockSettings::default();

    assert_eq!(DockEdge::Bottom, dock.edge);
    assert_eq!(100, dock.length_percent);
    assert_eq!(0.0, dock.edge_offset_dip);
    assert_eq!(0.0, dock.inner_padding_dip);
    assert_eq!(2.0, dock.icon_spacing_dip);
}

#[test]
fn sanitize_clamps_every_geometry_field_of_dock_settings() {
    let mut dock = DockSettings {
        length_percent: 3,
        edge_offset_dip: 500.0,
        inner_padding_dip: -4.0,
        icon_spacing_dip: f64::NAN,
        ..Default::default()
    };

    DockLayoutLimits::sanitize(&mut dock);

    assert_eq!(DockLayoutLimits::MIN_LENGTH_PERCENT, dock.length_percent);
    assert_eq!(DockLayoutLimits::MAX_EDGE_OFFSET_DIP, dock.edge_offset_dip);
    assert_eq!(DockLayoutLimits::MIN_INNER_PADDING_DIP, dock.inner_padding_dip);
    assert_eq!(DockLayoutLimits::DEFAULT_ICON_SPACING_DIP, dock.icon_spacing_dip);
}

#[test]
fn edge_is_written_by_name_and_read_back() {
    let dir = TempDir::new("koy-dock-layout-");
    let store = SettingsStore::new(dir.settings_path());
    store
        .save(&AppSettings { dock: DockSettings { edge: DockEdge::Left, length_percent: 70, ..Default::default() }, ..Default::default() })
        .unwrap();

    let text = std::fs::read_to_string(store.path()).unwrap();
    let back = store.load().dock;

    assert!(text.contains("\"Left\""), "{text}");
    assert_eq!(DockEdge::Left, back.edge);
    assert_eq!(70, back.length_percent);
}

#[test]
fn settings_file_without_geometry_fields_loads_as_bottom_full_length() {
    let dir = TempDir::new("koy-dock-layout-");
    std::fs::write(dir.settings_path(), "{ \"Version\": 4, \"Dock\": { \"Enabled\": true } }").unwrap();

    let back = SettingsStore::new(dir.settings_path()).load().dock;

    assert_eq!(DockEdge::Bottom, back.edge);
    assert_eq!(100, back.length_percent);
}

#[test]
fn thickness_and_offset_stay_inside_a_small_work_area() {
    let small = DockRectDip::new(0.0, 0.0, 1280.0, 40.0);

    let result = DockLayoutMath::compute(DockEdge::Bottom, 100, 96.0, 0.0, 60.0, small);

    assert!(result.outer.top >= small.top, "üst kenar çalışma alanının dışına taşmamalı");
    assert!(result.outer.top + result.outer.height <= small.top + small.height + 1e-9);
    assert!(result.outer.height <= small.height);
}

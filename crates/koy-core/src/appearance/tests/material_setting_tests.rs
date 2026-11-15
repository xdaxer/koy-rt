use crate::appearance::{MaterialLevel, MaterialSetting};

const LEVELS: [MaterialLevel; 3] = [MaterialLevel::Solid, MaterialLevel::Frosted, MaterialLevel::Glass];

#[test]
fn resolve_window_material_returns_solid_when_backdrop_unsupported() {
    for requested in LEVELS {
        let result = MaterialSetting::resolve_window_material(requested, false);

        assert_eq!(MaterialLevel::Solid, result, "{requested:?}");
    }
}

#[test]
fn resolve_window_material_preserves_requested_when_backdrop_supported() {
    for requested in LEVELS {
        let result = MaterialSetting::resolve_window_material(requested, true);

        assert_eq!(requested, result, "{requested:?}");
    }
}

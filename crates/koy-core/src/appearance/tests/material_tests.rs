use crate::appearance::{MaterialLevel, MaterialSetting, ReadabilityGuard, Rgb};

#[test]
fn surface_opacity_matches_demo_levels() {
    let rows = [
        (MaterialLevel::Solid, 1.0, 255u8),
        (MaterialLevel::Frosted, 0.86, 219),
        (MaterialLevel::Glass, 0.72, 184),
    ];
    for (value, opacity, alpha) in rows {
        assert_eq!(opacity, MaterialSetting::surface_opacity(value), "{value:?}");
        assert_eq!(alpha, MaterialSetting::surface_alpha(value), "{value:?}");
        assert_eq!(value != MaterialLevel::Solid, MaterialSetting::is_translucent(value), "{value:?}");
    }
}

// Demodaki duvar kağıdı ortalamaları.
const DARK_WALL: Rgb = Rgb::new(24, 30, 44);
const LIGHT_WALL: Rgb = Rgb::new(236, 240, 246);

#[test]
fn guard_keeps_glass_on_dark_wallpaper() {
    assert_eq!(
        MaterialLevel::Glass,
        ReadabilityGuard::effective(MaterialLevel::Glass, true, Some(DARK_WALL), ReadabilityGuard::NOTCH)
    );
    assert_eq!(
        MaterialLevel::Glass,
        ReadabilityGuard::effective(MaterialLevel::Glass, true, Some(DARK_WALL), ReadabilityGuard::WINDOW_DARK)
    );
}

#[test]
fn guard_raises_glass_to_frosted_on_light_wallpaper_like_the_demo() {
    // Onay notu: açık duvar kağıdında Cam → Buzlu (demodaki %72 / %86, bugün Koy penceresinin değerleri).
    assert!(ReadabilityGuard::secondary_contrast(MaterialSetting::GLASS_OPACITY, LIGHT_WALL, ReadabilityGuard::WINDOW_DARK) < 4.5);
    assert!(ReadabilityGuard::secondary_contrast(MaterialSetting::FROSTED_OPACITY, LIGHT_WALL, ReadabilityGuard::WINDOW_DARK) >= 4.5);
    assert_eq!(
        MaterialLevel::Frosted,
        ReadabilityGuard::effective(MaterialLevel::Glass, true, Some(LIGHT_WALL), ReadabilityGuard::WINDOW_DARK)
    );
}

#[test]
fn layered_surfaces_are_more_opaque_than_the_window() {
    let rows = [
        (MaterialLevel::Solid, 1.0, 255u8),
        (MaterialLevel::Frosted, 0.94, 240),
        (MaterialLevel::Glass, 0.88, 224),
    ];
    // Çentik ve dock bulanıklık alamaz; elle sınamada %72 arkadaki metni okunur bırakıyordu.
    for (value, opacity, alpha) in rows {
        assert_eq!(opacity, MaterialSetting::layered_opacity(value), "{value:?}");
        assert_eq!(alpha, MaterialSetting::layered_alpha(value), "{value:?}");
        assert!(MaterialSetting::layered_opacity(value) >= MaterialSetting::surface_opacity(value), "{value:?}");
    }
}

#[test]
fn notch_guard_uses_layered_opacity() {
    assert!(ReadabilityGuard::NOTCH.layered);
    assert_eq!(MaterialSetting::LAYERED_GLASS_OPACITY, ReadabilityGuard::NOTCH.opacity_of(MaterialLevel::Glass));
    assert_eq!(MaterialSetting::GLASS_OPACITY, ReadabilityGuard::WINDOW_DARK.opacity_of(MaterialLevel::Glass));
    // Açık duvar kağıdında çentik yine eşiği geçen bir seviyede kalır.
    let level = ReadabilityGuard::effective(MaterialLevel::Glass, true, Some(LIGHT_WALL), ReadabilityGuard::NOTCH);
    assert!(
        level == MaterialLevel::Solid
            || ReadabilityGuard::secondary_contrast(ReadabilityGuard::NOTCH.opacity_of(level), LIGHT_WALL, ReadabilityGuard::NOTCH) >= 4.5
    );
}

#[test]
fn guard_result_is_solid_or_passes_threshold() {
    let white = Rgb::new(255, 255, 255);
    for surface in [ReadabilityGuard::NOTCH, ReadabilityGuard::WINDOW_DARK, ReadabilityGuard::WINDOW_LIGHT] {
        let effective = ReadabilityGuard::effective(MaterialLevel::Glass, true, Some(white), surface);
        assert!(
            effective == MaterialLevel::Solid
                || ReadabilityGuard::secondary_contrast(surface.opacity_of(effective), white, surface) >= 4.5
        );
    }
}

#[test]
fn guard_off_or_unknown_wallpaper_returns_requested() {
    assert_eq!(
        MaterialLevel::Glass,
        ReadabilityGuard::effective(MaterialLevel::Glass, false, Some(LIGHT_WALL), ReadabilityGuard::NOTCH)
    );
    assert_eq!(MaterialLevel::Glass, ReadabilityGuard::effective(MaterialLevel::Glass, true, None, ReadabilityGuard::NOTCH));
    assert_eq!(
        MaterialLevel::Solid,
        ReadabilityGuard::effective(MaterialLevel::Solid, true, Some(LIGHT_WALL), ReadabilityGuard::NOTCH)
    );
}

#[test]
fn guard_never_lowers_opacity() {
    assert_eq!(
        MaterialLevel::Frosted,
        ReadabilityGuard::effective(MaterialLevel::Frosted, true, Some(DARK_WALL), ReadabilityGuard::NOTCH)
    );
}

#[test]
fn average_bgra_averages_channels_and_ignores_alpha() {
    let pixels: [u8; 8] = [0, 0, 255, 255, 255, 0, 0, 0]; // kırmızı, ardından alfası 0 olan mavi
    assert_eq!(Some(Rgb::new(127, 0, 127)), ReadabilityGuard::average_bgra(&pixels));
    assert_eq!(None, ReadabilityGuard::average_bgra(&[]));
    assert_eq!(None, ReadabilityGuard::average_bgra(&[1, 2, 3]));
}

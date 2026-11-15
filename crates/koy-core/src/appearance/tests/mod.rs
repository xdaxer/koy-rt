//! `koy/tests/Core.Tests` altındaki Appearance testleri.

use std::path::PathBuf;

mod app_icon_override_tests;
mod appearance_tests;
mod font_scale_policy_tests;
mod icon_pack_files_tests;
mod material_setting_tests;
mod material_tests;
mod motion_toggles_tests;
mod user_style_overrides_tests;

/// xunit `Assert.Equal(expected, actual, precision)`: iki sayı da `precision` basamağa yuvarlanıp karşılaştırılır.
fn assert_eq_precision(expected: f64, actual: f64, precision: i32) {
    let e = crate::text::round_digits(expected, precision);
    let a = crate::text::round_digits(actual, precision);
    assert!(e == a, "beklenen {expected}, gelen {actual} ({precision} basamak)");
}

/// C# testlerinin `Koy.slnx`'ten yukarı yürüyerek bulduğu `src/App/Resources` klasörü. Rust çalışma alanı
/// `koy-rt/` kardeş klasörde durduğundan `CARGO_MANIFEST_DIR`'den göreli yol kullanılır.
fn resources_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../koy/src/App/Resources");
    assert!(dir.is_dir(), "XAML kaynak klasörü bulunamadı: {}", dir.display());
    dir
}

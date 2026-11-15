//! Atlananlar (Settings modülü taşınınca eklenecek; `AppearanceSettings`/`SettingsStore`/`SettingsFile` ister):
//! `Default_appearance_font_values_are_safe`, `Font_settings_survive_a_save_and_load_round_trip`,
//! `Missing_font_fields_in_file_read_as_safe_defaults`.

use super::assert_eq_precision;
use crate::appearance::FontScalePolicy;

const INSTALLED: [&str; 3] = ["Segoe UI", "Cascadia Mono", "Consolas"];

#[test]
fn fit_scale_keeps_window_on_the_work_area() {
    // (yüzde, çalışma alanı genişliği, yüksekliği, beklenen)
    let rows: [(i32, f64, f64, f64); 7] = [
        (100, 1920.0, 1040.0, 1.0), // sığıyor
        (150, 1920.0, 1040.0, 1.5), // 1320x1020 sığıyor
        (150, 1366.0, 728.0, 1.07), // 728/680 ≈ 1,07'ye iner
        (120, 800.0, 600.0, 0.88),  // 600/680 ≈ 0,88
        (120, 640.0, 480.0, 0.8),   // çok küçük ekranda alt sınır
        (400, 3840.0, 2100.0, 1.5), // önce 150'ye kısılır
        (130, 0.0, 0.0, 1.3),       // çalışma alanı bilinmiyor
    ];
    for (percent, work_w, work_h, expected) in rows {
        assert_eq_precision(expected, FontScalePolicy::fit_scale(percent, 880.0, 680.0, work_w, work_h), 2);
    }
}

#[test]
fn fit_scale_ignores_nan_work_area() {
    assert_eq_precision(1.2, FontScalePolicy::fit_scale(120, 880.0, 680.0, f64::NAN, 900.0), 3);
}

#[test]
fn font_tokens_scale_with_base_size() {
    let rows: [(f64, f64, f64); 5] = [
        (13.0, 13.0, 13.0),
        (20.0, 13.0, 20.0),
        (12.5, 16.0, 15.5), // 12,5 x 16/13 = 15,38 → 15,5
        (20.0, 10.0, 15.5), // 15,38 → 15,5
        (12.0, 99.0, 16.5), // boyut 18'e kısılır: 12 x 18/13 = 16,6 → 16,5
    ];
    for (token, base_size, expected) in rows {
        assert_eq!(expected, FontScalePolicy::scale_font_token(token, base_size), "{token} {base_size}");
    }
}

#[test]
fn ui_scale_is_clamped_to_80_150() {
    let rows = [(50, 80), (80, 80), (100, 100), (150, 150), (400, 150), (-5, 80)];
    for (requested, expected) in rows {
        assert_eq!(expected, FontScalePolicy::clamp_ui_scale(requested), "{requested}");
    }
}

#[test]
fn scaled_dip_applies_clamped_percent() {
    assert_eq!(28.0, FontScalePolicy::scale_dip(28.0, 100));
    assert_eq!(42.0, FontScalePolicy::scale_dip(28.0, 150));
    // Sınır dışı yüzde önce sınırlanır: 400 değil 150 uygulanır.
    assert_eq!(42.0, FontScalePolicy::scale_dip(28.0, 400));
    assert_eq_precision(22.4, FontScalePolicy::scale_dip(28.0, 80), 6);
}

#[test]
fn base_font_size_is_clamped_and_non_finite_falls_back() {
    let rows = [(4.0, 10.0), (13.0, 13.0), (40.0, 18.0), (f64::NAN, 13.0), (f64::INFINITY, 13.0)];
    for (requested, expected) in rows {
        assert_eq!(expected, FontScalePolicy::clamp_base_font_size(requested), "{requested}");
    }
}

#[test]
fn font_weight_is_clamped_and_snapped_to_hundreds() {
    let rows = [(0, 100), (450, 500), (449, 400), (400, 400), (1200, 900)];
    for (requested, expected) in rows {
        assert_eq!(expected, FontScalePolicy::clamp_font_weight(requested), "{requested}");
    }
}

#[test]
fn font_family_resolves_case_insensitively_to_installed_spelling() {
    assert_eq!("Cascadia Mono", FontScalePolicy::resolve_font_family(Some("cascadia mono"), INSTALLED));
    assert_eq!("Consolas", FontScalePolicy::resolve_font_family(Some("  CONSOLAS "), INSTALLED));
}

#[test]
fn missing_or_empty_font_family_falls_back_to_default() {
    for requested in [Some(""), Some("   "), None, Some("Kaldirilmis Font")] {
        assert_eq!(
            FontScalePolicy::DEFAULT_FONT_FAMILY,
            FontScalePolicy::resolve_font_family(requested, INSTALLED),
            "{requested:?}"
        );
    }
}

#[test]
fn overlong_font_family_falls_back_without_matching() {
    let huge = "a".repeat(FontScalePolicy::MAX_FONT_FAMILY_LENGTH as usize + 1);

    assert_eq!(FontScalePolicy::DEFAULT_FONT_FAMILY, FontScalePolicy::resolve_font_family(Some(&huge), [huge.as_str()]));
}

#[test]
fn non_finite_dip_scales_to_zero() {
    for dip in [f64::NAN, f64::INFINITY] {
        assert_eq!(0.0, FontScalePolicy::scale_dip(dip, 100), "{dip}");
    }
}

#[test]
fn custom_fallback_is_used_when_font_is_missing() {
    assert_eq!("Consolas", FontScalePolicy::resolve_font_family_with_fallback(Some("Yok"), INSTALLED, "Consolas"));
}

#[test]
fn clock_font_falls_back_to_ui_font_when_empty_or_missing() {
    assert_eq!("Segoe UI", FontScalePolicy::resolve_clock_font_family(Some(""), "Segoe UI", INSTALLED));
    assert_eq!("Segoe UI", FontScalePolicy::resolve_clock_font_family(Some("Yok"), "Segoe UI", INSTALLED));
    assert_eq!("Cascadia Mono", FontScalePolicy::resolve_clock_font_family(Some("cascadia mono"), "Segoe UI", INSTALLED));
}

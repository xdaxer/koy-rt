//! Karşılığı: `koy/tests/Core.Tests/Updates/UpdateFlowPolicyTests.cs`.

use crate::updates::UpdateFlowPolicy;

#[test]
fn compare_versions_dogru_karsilastirir() {
    let cases = [
        ("0.2.0", "0.1.1", 1),
        ("0.1.1", "0.2.0", -1),
        ("0.2.0", "0.2.0", 0),
        ("v0.2.0", "0.2.0", 0),
        ("0.2.0+abc", "0.2.0+def", 0),
        ("0.2.1", "0.2.0", 1),
        ("1.0.0", "0.9.9", 1),
        ("0.3.0", "0.3.0-rc.2", 1),
        ("0.3.0-rc.2", "0.3.0", -1),
        ("0.3.0-rc.10", "0.3.0-rc.2", 1),
        ("0.3.0-rc.1", "0.3.0-rc.1", 0),
        ("0.3.0-beta", "0.3.0-alpha", 1),
    ];
    for (a, b, expected_sign) in cases {
        let result = UpdateFlowPolicy::compare_versions(Some(a), Some(b));
        assert_eq!(expected_sign, result.signum(), "{a} <=> {b}");
    }
}

#[test]
fn is_newer_version_yalniz_daha_yuksek_surumlerde_true() {
    let cases = [("0.1.1", "0.2.0", true), ("0.2.0", "0.2.0", false), ("0.3.0", "0.2.0", false), ("v0.1.1", "0.2.0", true)];
    for (current, available, expected) in cases {
        assert_eq!(expected, UpdateFlowPolicy::is_newer_version(current, available), "{current} -> {available}");
    }
}

#[test]
fn should_show_whats_new_ilk_kurulumda_false() {
    // İlk açılış sihirbazı çalışır, Neler Geldi gösterilmez.
    assert!(!UpdateFlowPolicy::should_show_whats_new("0.2.0", Some(""), false));
    assert!(!UpdateFlowPolicy::should_show_whats_new("0.2.0", None, false));
}

#[test]
fn should_show_whats_new_eski_surumden_geciste_true() {
    // Daha önce sürüm kaydedilmemiş (v0.1.1/0.2.0'da bu ayar yoktu) ama firstRunCompleted=true
    assert!(UpdateFlowPolicy::should_show_whats_new("0.2.0", Some(""), true));
    assert!(UpdateFlowPolicy::should_show_whats_new("0.2.0", None, true));
}

#[test]
fn should_show_whats_new_yeni_surumde_true() {
    assert!(UpdateFlowPolicy::should_show_whats_new("0.2.1", Some("0.2.0"), true));
    assert!(UpdateFlowPolicy::should_show_whats_new("0.3.0", Some("0.2.0"), true));
}

#[test]
fn should_show_whats_new_ayni_veya_eski_surumde_false() {
    assert!(!UpdateFlowPolicy::should_show_whats_new("0.2.0", Some("0.2.0"), true));
    assert!(!UpdateFlowPolicy::should_show_whats_new("0.1.9", Some("0.2.0"), true));
}

#[test]
fn format_display_version_formatlar() {
    let cases = [("0.2.0", false, "0.2.0"), ("v0.2.0", false, "0.2.0"), ("0.2.0", true, "v0.2.0"), ("v0.2.0", true, "v0.2.0")];
    for (input, prefix_with_v, expected) in cases {
        assert_eq!(expected, UpdateFlowPolicy::format_display_version(Some(input), prefix_with_v), "{input} ({prefix_with_v})");
    }
}

#[test]
fn should_show_whats_new_on_surumden_son_surume_gecince_gosterir() {
    assert!(UpdateFlowPolicy::should_show_whats_new("0.3.0", Some("0.3.0-rc.2"), true));
    assert!(!UpdateFlowPolicy::should_show_whats_new("0.3.0-rc.2", Some("0.3.0"), true));
}

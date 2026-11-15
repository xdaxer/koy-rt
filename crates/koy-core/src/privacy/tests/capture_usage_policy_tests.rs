//! Karşılığı: `tests/Core.Tests/Privacy/CaptureUsagePolicyTests.cs`.

use crate::privacy::{CaptureUsagePolicy, PrivacyState};

#[test]
fn is_actively_using_zero_start_returns_false() {
    assert!(!CaptureUsagePolicy::is_actively_using(0, 0));
    assert!(!CaptureUsagePolicy::is_actively_using(0, 100));
}

#[test]
fn is_actively_using_start_greater_than_zero_stop_zero_returns_true() {
    assert!(CaptureUsagePolicy::is_actively_using(100, 0));
}

#[test]
fn is_actively_using_start_greater_than_stop_returns_true() {
    assert!(CaptureUsagePolicy::is_actively_using(200, 150));
}

#[test]
fn is_actively_using_stop_greater_than_or_equal_start_returns_false() {
    assert!(!CaptureUsagePolicy::is_actively_using(100, 100));
    assert!(!CaptureUsagePolicy::is_actively_using(100, 200));
}

#[test]
fn extract_app_name_from_key_extracts_expected_name() {
    let cases: [(Option<&str>, Option<&str>); 7] = [
        (Some("C:#Program Files#obs-studio#bin#64bit#obs64.exe"), Some("obs64")),
        (Some("C:#Program Files#Google#Chrome#Application#chrome.exe"), Some("chrome")),
        (Some("C:\\path\\to\\app.exe"), Some("app")),
        (Some("Microsoft.WindowsCamera_8wekyb3d8bbwe!App"), Some("WindowsCamera")),
        (Some("5319275A.WhatsAppDesktop_cv1g1gvanyjgm"), Some("WhatsAppDesktop")),
        (Some(""), None),
        (None, None),
    ];
    for (key, expected) in cases {
        let actual = CaptureUsagePolicy::extract_app_name_from_key(key);
        assert_eq!(expected.map(str::to_string), actual, "{key:?}");
    }
}

const BOTH: &str = "Cam: {0}, Mic: {1}";
const CAM_ONLY: &str = "Cam: {0}";
const MIC_ONLY: &str = "Mic: {0}";

#[test]
fn format_tool_tip_uses_templates_and_unknown_name() {
    let cases: [(bool, bool, Option<&str>, Option<&str>, &str); 6] = [
        (true, true, Some("CameraApp"), Some("MicApp"), "Cam: CameraApp, Mic: MicApp"),
        (true, false, Some("CameraApp"), None, "Cam: CameraApp"),
        (false, true, None, Some("MicApp"), "Mic: MicApp"),
        (true, true, None, None, "Cam: Unknown, Mic: Unknown"),
        (true, false, None, None, "Cam: Unknown"),
        (false, false, None, None, ""),
    ];
    for (cam, mic, cam_app, mic_app, expected) in cases {
        let state = PrivacyState::new(cam, mic, cam_app.map(str::to_string), mic_app.map(str::to_string));
        assert_eq!(
            expected,
            CaptureUsagePolicy::format_tool_tip(&state, BOTH, CAM_ONLY, MIC_ONLY, "Unknown"),
            "{cam} {mic} {cam_app:?} {mic_app:?}"
        );
    }
}

#[test]
fn privacy_state_empty_and_any_in_use() {
    assert!(!PrivacyState::EMPTY.any_in_use());
    assert!(PrivacyState::new(false, true, None, None).any_in_use());
}

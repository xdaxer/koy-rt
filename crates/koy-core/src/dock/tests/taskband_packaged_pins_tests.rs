use std::sync::Arc;

use super::fakes::{controller, FakePins, FakeShell, FakeTracker};
use super::strs;
use crate::dock::{PinnedApp, PinnedAppMatcher, PinnedOrder, TaskbandPackagedPins};
use crate::shell::{OpenWindowInfo, WindowRect};

const BOUNDS: WindowRect = WindowRect::new(0, 0, 800, 600);

#[test]
fn extract_aumids_returns_empty_for_null_or_short_input() {
    assert!(TaskbandPackagedPins::extract_aumids(None).is_empty());
    assert!(TaskbandPackagedPins::extract_aumids(Some(&[])).is_empty());
    assert!(TaskbandPackagedPins::extract_aumids(Some(&[1, 2, 3, 4])).is_empty());
}

#[test]
fn is_packaged_aumid_validates_format() {
    let cases = [
        ("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify", true),
        ("5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App", true),
        ("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", true),
        ("windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel", true),
        ("Visual Studio Code.lnk", false),
        ("Microsoft.Windows.Explorer", false),
        ("NoUnderscore!App", false),
        ("Pkg_short!App", false),
        ("Pkg_123456789012i!App", false), // 'i' Crockford Base32'de yok
        ("Pkg_zpdnekdrzrea0!", false),
        ("_zpdnekdrzrea0!App", false),
        (".BadPkg_zpdnekdrzrea0!App", false),
    ];
    for (value, expected) in cases {
        assert_eq!(expected, TaskbandPackagedPins::is_packaged_aumid(value), "{value}");
    }
}

fn write_utf16_with_nulls(ms: &mut Vec<u8>, text: &str) {
    ms.extend([0x00, 0x00]);
    ms.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    ms.extend([0x00, 0x00]);
}

#[test]
fn extract_aumids_extracts_distinct_packaged_apps_in_taskband_order_across_alignments() {
    // Gerçek Taskband\Favorites akışını taklit eder: çift ve tek hizalamada UTF-16LE dizgiler, tekrarlar ve .lnk girdileri.
    let mut ms: Vec<u8> = Vec::new();
    write_utf16_with_nulls(&mut ms, "Visual Studio Code.lnk");
    write_utf16_with_nulls(&mut ms, "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify");
    write_utf16_with_nulls(&mut ms, r"Assets\Square44x44Logo.png");
    write_utf16_with_nulls(&mut ms, "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"); // tekrar
    ms.push(0x01); // tek bayt kaydırarak hizalama 1'i sına
    write_utf16_with_nulls(&mut ms, "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App");
    write_utf16_with_nulls(&mut ms, "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App");

    let aumids = TaskbandPackagedPins::extract_aumids(Some(&ms));

    assert_eq!(
        strs(&[
            "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify",
            "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App",
            "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
        ]),
        aumids
    );
}

#[test]
fn seed_from_taskband_preserves_order_for_mixed_lnk_and_packaged_pins() {
    let mut ms: Vec<u8> = Vec::new();
    write_utf16_with_nulls(&mut ms, "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify");
    write_utf16_with_nulls(&mut ms, "Google Chrome.lnk");
    write_utf16_with_nulls(&mut ms, "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App");

    let folder_order = strs(&[
        "Google Chrome.lnk",
        "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App",
        "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify",
    ]);

    let seeded = PinnedOrder::seed_from_taskband(Some(&ms), &folder_order);

    assert_eq!(
        strs(&[
            "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify",
            "Google Chrome.lnk",
            "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App",
        ]),
        seeded
    );
}

#[test]
fn packaged_pin_matches_running_uwp_window_by_aumid_and_not_unrelated_title() {
    const AUMID: &str = "Microsoft.WindowsStore_8wekyb3d8bbwe!App";
    let shell_path = TaskbandPackagedPins::apps_folder_path(AUMID);
    let pin = PinnedApp::new("Microsoft Store", shell_path.clone(), Some(shell_path.clone()), None, Some(AUMID.to_string()));

    let store_window = OpenWindowInfo {
        icon_data: None,
        app_user_model_id: Some(AUMID.to_string()),
        class_name: Some("ApplicationFrameWindow".to_string()),
        ..OpenWindowInfo::new(1, "Microsoft Store", Some(shell_path), 42, BOUNDS)
    };
    let unrelated_window = OpenWindowInfo {
        class_name: Some("Notepad".to_string()),
        ..OpenWindowInfo::new(2, "Microsoft Belgesi - Not Defteri", Some(r"C:\Windows\notepad.exe".to_string()), 99, BOUNDS)
    };

    assert!(PinnedAppMatcher::is_match(&pin, &store_window));
    assert!(!PinnedAppMatcher::is_match(&pin, &unrelated_window));
}

#[test]
fn clicking_not_running_packaged_pin_launches_shell_apps_folder_path() {
    const AUMID: &str = "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App";
    let shell_path = TaskbandPackagedPins::apps_folder_path(AUMID);
    let shell = FakeShell::new();

    let controller = controller(
        FakePins::new(vec![PinnedApp::new("Calculator", shell_path.clone(), Some(shell_path.clone()), None, Some(AUMID.to_string()))]),
        FakeTracker::new(),
        Arc::clone(&shell),
    );

    controller.refresh();
    assert_eq!(1, controller.items().len());
    assert_eq!(Some(AUMID.to_string()), controller.items()[0].app_user_model_id);

    controller.handle_item_click(&controller.items()[0]);

    assert_eq!(vec![shell_path], shell.launched());
}

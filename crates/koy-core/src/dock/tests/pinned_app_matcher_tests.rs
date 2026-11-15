use crate::dock::{KnownAppUserModelIds, PinnedApp, PinnedAppMatcher};
use crate::shell::{OpenWindowInfo, WindowRect};
use crate::win_path;

const BOUNDS: WindowRect = WindowRect::new(0, 0, 800, 600);

fn win(title: &str, path: Option<&str>, aumid: Option<&str>) -> OpenWindowInfo {
    let class_name = match path {
        Some(p) if crate::text::ends_with_ignore_case(p, "explorer.exe") => Some("CabinetWClass".to_string()),
        _ => None,
    };
    OpenWindowInfo {
        icon_data: None,
        app_user_model_id: aumid.map(str::to_string),
        class_name,
        ..OpenWindowInfo::new(1, title, path.map(str::to_string), 10, BOUNDS)
    }
}

fn pin(title: &str, shortcut: &str, target: Option<&str>, aumid: Option<&str>) -> PinnedApp {
    PinnedApp::new(title, shortcut, target.map(str::to_string), None, aumid.map(str::to_string))
}

fn system_root() -> String {
    std::env::var("SystemRoot").unwrap()
}

#[test]
fn explorer_pin_without_target_matches_explorer_window_by_aumid() {
    let pin = pin("File Explorer", r"C:\Pins\File Explorer.lnk", None, Some("Microsoft.Windows.Explorer"));

    assert!(PinnedAppMatcher::is_match(&pin, &win("Downloads", Some(r"C:\WINDOWS\Explorer.EXE"), Some("Microsoft.Windows.Explorer"))));
}

#[test]
fn explorer_window_without_own_aumid_matches_pin_via_known_table() {
    let system_root = system_root();
    // Gerçek durum: kısayol AUMID'i dolu, hedef yerel ad ("Dosya Gezgini") yol değil, pencerenin AUMID'i yok.
    let pin = pin("Dosya Gezgini", r"C:\Pins\File Explorer.lnk", Some("Dosya Gezgini"), Some("Microsoft.Windows.Explorer"));

    assert!(PinnedAppMatcher::is_match(&pin, &win("İndirilenler - Dosya Gezgini", Some(&win_path::combine(&system_root, "Explorer.EXE")), None)));
    assert!(!PinnedAppMatcher::is_match(&pin, &win("Notlar", Some(r"C:\Windows\notepad.exe"), None)));
}

#[test]
fn known_table_only_knows_the_system_explorer() {
    let system_root = system_root();
    let explorer = win_path::combine(&system_root, "explorer.exe");
    assert_eq!(Some(KnownAppUserModelIds::EXPLORER.to_string()), KnownAppUserModelIds::for_window(Some(&explorer), Some("CabinetWClass")));
    assert_eq!(Some(KnownAppUserModelIds::EXPLORER.to_string()), KnownAppUserModelIds::for_window(Some(&explorer), Some("ExploreWClass")));
    assert_eq!(None, KnownAppUserModelIds::for_window(Some(r"C:\Tools\explorer.exe"), Some("CabinetWClass")));
    assert_eq!(None, KnownAppUserModelIds::for_window(None, Some("CabinetWClass")));
}

#[test]
fn other_explorer_process_windows_are_not_folder_windows() {
    let system_root = system_root();
    for cls in [Some("Progman"), Some("Shell_TrayWnd"), Some("#32770"), None] {
        assert_eq!(None, KnownAppUserModelIds::for_window(Some(&win_path::combine(&system_root, "explorer.exe")), cls), "{cls:?}");
    }
}

#[test]
fn explorer_pin_does_not_swallow_other_windows() {
    let pin = pin("File Explorer", r"C:\Pins\File Explorer.lnk", None, Some("Microsoft.Windows.Explorer"));

    assert!(!PinnedAppMatcher::is_match(&pin, &win("Notes", Some(r"C:\Windows\notepad.exe"), Some("Microsoft.Windows.Notepad"))));
}

#[test]
fn different_aumid_with_same_exe_stays_separate_chrome_profiles() {
    let pin = pin(
        "Google Chrome",
        r"C:\Pins\Chrome.lnk",
        Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
        Some("Chrome"),
    );

    assert!(PinnedAppMatcher::is_match(&pin, &win("a", Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe"), Some("Chrome"))));
    assert!(!PinnedAppMatcher::is_match(&pin, &win("b", Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe"), Some("Chrome.Profile2"))));
}

#[test]
fn aumid_comparison_ignores_case() {
    let pin = pin("X", r"C:\Pins\X.lnk", None, Some("Vendor.App"));
    assert!(PinnedAppMatcher::is_match(&pin, &win("t", None, Some("vendor.app"))));
}

#[test]
fn uwp_shell_appsfolder_window_matches_by_aumid() {
    let pin = pin("Spotify", r"C:\Pins\Spotify.lnk", None, Some("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"));
    let win = win(
        "Spotify",
        Some(r"shell:AppsFolder\SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
        Some("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
    );

    assert!(PinnedAppMatcher::is_match(&pin, &win));
}

#[test]
fn without_aumid_full_path_matches_case_insensitively() {
    let pin = pin("Code", r"C:\Pins\Code.lnk", Some(r"C:\Users\x\AppData\Local\Programs\Microsoft VS Code\Code.exe"), None);

    assert!(PinnedAppMatcher::is_match(&pin, &win("a", Some(r"c:\users\x\appdata\local\programs\microsoft vs code\code.exe"), None)));
}

#[test]
fn system32_and_syswow64_paths_are_the_same_program() {
    assert!(PinnedAppMatcher::same_path(r"C:\Windows\SysWOW64\notepad.exe", r"C:\WINDOWS\System32\NOTEPAD.EXE"));
    assert!(PinnedAppMatcher::same_path(r"C:\Windows\Sysnative\cmd.exe", r"C:\Windows\System32\cmd.exe"));
}

#[test]
fn environment_variables_in_target_are_expanded() {
    let root = system_root();
    assert!(PinnedAppMatcher::same_path(r"%SystemRoot%\explorer.exe", &win_path::combine(&root, "Explorer.EXE")));
}

#[test]
fn only_one_side_has_aumid_falls_back_to_path() {
    let pin = pin("Code", r"C:\Pins\Code.lnk", Some(r"C:\VSCode\code.exe"), Some("Microsoft.VisualStudioCode"));
    assert!(PinnedAppMatcher::is_match(&pin, &win("a", Some(r"C:\VSCode\code.exe"), None)));
    assert!(!PinnedAppMatcher::is_match(&pin, &win("a", Some(r"C:\Other\notepad.exe"), None)));
}

#[test]
fn title_is_the_last_resort_for_pins_without_target_or_aumid() {
    let pin = pin("Obsidian", r"C:\Pins\Obsidian.lnk", None, None);
    assert!(PinnedAppMatcher::is_match(&pin, &win("Notes - Obsidian", None, None)));
}

//! Dock öğesinin AppUserModelID'si (atlama listesi bu kimliğe bağlı).

use super::fakes::{controller, FakePins, FakeShell, FakeTracker};
use crate::dock::{KnownAppUserModelIds, PinnedApp};
use crate::shell::{OpenWindowInfo, WindowRect};

const RECT: WindowRect = WindowRect::new(0, 0, 100, 100);

fn pin(title: &str, shortcut: &str, target: &str, aumid: &str) -> PinnedApp {
    PinnedApp::new(title, shortcut, Some(target.to_string()), None, Some(aumid.to_string()))
}

fn window(path: &str, aumid: Option<&str>, class_name: Option<&str>) -> OpenWindowInfo {
    OpenWindowInfo {
        app_user_model_id: aumid.map(str::to_string),
        class_name: class_name.map(str::to_string),
        ..OpenWindowInfo::new(1, "t", Some(path.to_string()), 10, RECT)
    }
}

#[test]
fn pinned_not_running_uses_the_shortcut_aumid() {
    let controller = controller(
        FakePins::new(vec![pin("Code", r"C:\p\Code.lnk", r"C:\a\Code.exe", "Vendor.Code")]),
        FakeTracker::new(),
        FakeShell::new(),
    );
    controller.refresh();

    assert_eq!(Some("Vendor.Code".to_string()), controller.items()[0].app_user_model_id);
}

#[test]
fn running_window_aumid_wins_over_the_pin() {
    let controller = controller(
        FakePins::new(vec![pin("Chrome", r"C:\p\Chrome.lnk", r"C:\a\chrome.exe", "Chrome")]),
        FakeTracker::with_windows(vec![window(r"C:\a\chrome.exe", Some("Chrome"), None)]),
        FakeShell::new(),
    );
    controller.refresh();

    assert_eq!(Some("Chrome".to_string()), controller.items()[0].app_user_model_id);
}

#[test]
fn unpinned_window_carries_its_own_aumid() {
    let controller = controller(
        FakePins::new(vec![]),
        FakeTracker::with_windows(vec![window(r"C:\a\app.exe", Some("Vendor.App"), None)]),
        FakeShell::new(),
    );
    controller.refresh();

    assert_eq!(Some("Vendor.App".to_string()), controller.items()[0].app_user_model_id);
}

#[test]
fn window_without_aumid_gets_none_so_only_koys_own_items_show() {
    // Klasik uygulama: AUMID yok ve exe yolundan kimlik türetilmez.
    let controller = controller(
        FakePins::new(vec![]),
        FakeTracker::with_windows(vec![window(r"C:\a\app.exe", None, Some("SomeClass"))]),
        FakeShell::new(),
    );
    controller.refresh();

    assert_eq!(None, controller.items()[0].app_user_model_id);
}

#[test]
fn explorer_folder_window_maps_to_the_known_explorer_aumid() {
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let explorer = crate::win_path::combine(&system_root, "explorer.exe");
    let controller = controller(
        FakePins::new(vec![]),
        FakeTracker::with_windows(vec![window(&explorer, None, Some("CabinetWClass"))]),
        FakeShell::new(),
    );
    controller.refresh();

    assert_eq!(Some(KnownAppUserModelIds::EXPLORER.to_string()), controller.items()[0].app_user_model_id);
}

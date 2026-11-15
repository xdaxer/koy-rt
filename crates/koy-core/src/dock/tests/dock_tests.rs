use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::fakes::{controller, FakeFullscreen, FakePins, FakeShell, FakeTracker};
use super::refresh_debouncer_tests::ManualTimeProvider;
use super::strs;
use crate::dock::{DockController, DockGroupingMode, OrderedPinnedAppsService, PinnedApp};
use crate::shell::{OpenWindowInfo, WindowInfo, WindowRect};
use crate::time::TimeSpan;

fn pin(title: &str, shortcut: &str, target: Option<&str>) -> PinnedApp {
    PinnedApp::new(title, shortcut, target.map(str::to_string), None, None)
}

fn win(handle: isize, title: &str, path: &str, pid: i32, bounds: WindowRect) -> OpenWindowInfo {
    OpenWindowInfo::new(handle, title, Some(path.to_string()), pid, bounds)
}

#[test]
fn refresh_loads_pinned_apps_in_preserved_order() {
    let pinned = FakePins::new(vec![
        pin("Chrome", r"C:\Pins\Chrome.lnk", Some(r"C:\Program Files\Chrome\chrome.exe")),
        pin("Code", r"C:\Pins\Code.lnk", Some(r"C:\Program Files\VSCode\Code.exe")),
    ]);

    let controller = controller(pinned, FakeTracker::new(), FakeShell::new());
    controller.refresh();

    let items = controller.items();
    assert_eq!(2, items.len());
    assert_eq!("Chrome", items[0].title);
    assert!(items[0].is_pinned);
    assert!(!items[0].is_running);

    assert_eq!("Code", items[1].title);
    assert!(items[1].is_pinned);
    assert!(!items[1].is_running);
}

#[test]
fn refresh_matches_running_window_to_pinned_app() {
    let pinned = FakePins::new(vec![pin("Chrome", r"C:\Pins\Chrome.lnk", Some(r"C:\Program Files\Chrome\chrome.exe"))]);
    let tracker = FakeTracker::with_windows(vec![win(
        1001,
        "Google - Google Chrome",
        r"C:\Program Files\Chrome\chrome.exe",
        1234,
        WindowRect::new(0, 0, 800, 600),
    )]);

    let controller = controller(pinned, tracker, FakeShell::new());
    controller.refresh();

    let items = controller.items();
    assert_eq!(1, items.len());
    let item = &items[0];
    assert!(item.is_pinned);
    assert!(item.is_running);
    assert_eq!(1, item.window_handles.len());
    assert_eq!(1001, item.window_handles[0]);
    assert_eq!(1234, item.process_id);
}

#[test]
fn refresh_groups_multiple_windows_of_same_app() {
    let pinned = FakePins::new(vec![pin("Chrome", r"C:\Pins\Chrome.lnk", Some(r"C:\Program Files\Chrome\chrome.exe"))]);
    let tracker = FakeTracker::with_windows(vec![
        win(1001, "Google - Chrome", r"C:\Program Files\Chrome\chrome.exe", 1234, WindowRect::new(0, 0, 800, 600)),
        win(1002, "GitHub - Chrome", r"C:\Program Files\Chrome\chrome.exe", 1234, WindowRect::new(0, 0, 800, 600)),
    ]);

    let controller = controller(pinned, tracker, FakeShell::new());
    controller.refresh();

    let items = controller.items();
    assert_eq!(1, items.len());
    let item = &items[0];
    assert_eq!(2, item.window_handles.len());
    assert!(item.window_handles.contains(&1001));
    assert!(item.window_handles.contains(&1002));
}

#[test]
fn refresh_adds_unpinned_running_apps_after_pinned_apps() {
    let pinned = FakePins::new(vec![pin("Chrome", r"C:\Pins\Chrome.lnk", Some(r"C:\Program Files\Chrome\chrome.exe"))]);
    let tracker = FakeTracker::with_windows(vec![win(
        2001,
        "Untitled - Notepad",
        r"C:\Windows\System32\notepad.exe",
        5678,
        WindowRect::new(0, 0, 600, 400),
    )]);

    let controller = controller(pinned, tracker, FakeShell::new());
    controller.refresh();

    let items = controller.items();
    assert_eq!(2, items.len());
    assert_eq!("Chrome", items[0].title);
    assert!(items[0].is_pinned);
    assert!(!items[0].is_running);

    assert_eq!("Untitled - Notepad", items[1].title);
    assert!(!items[1].is_pinned);
    assert!(items[1].is_running);
    assert_eq!(1, items[1].window_handles.len());
    assert_eq!(2001, items[1].window_handles[0]);
}

#[test]
fn refresh_identifies_active_window() {
    let tracker = FakeTracker::with_windows(vec![
        win(1001, "Notepad", r"C:\Windows\notepad.exe", 100, WindowRect::new(0, 0, 400, 400)),
        win(2001, "Calculator", r"C:\Windows\calc.exe", 200, WindowRect::new(0, 0, 400, 400)),
    ]);
    tracker.set_foreground(Some(WindowInfo { handle: 2001, bounds: WindowRect::new(0, 0, 400, 400) }));

    let controller = controller(FakePins::new(vec![]), tracker, FakeShell::new());
    controller.refresh();

    let items = controller.items();
    assert_eq!(2, items.len());
    assert!(!items[0].is_active);
    assert!(items[1].is_active);
}

#[test]
fn handle_item_click_launches_when_not_running() {
    let pinned = FakePins::new(vec![pin("Code", r"C:\Pins\Code.lnk", Some(r"C:\VSCode\code.exe"))]);
    let shell = FakeShell::new();

    let controller = controller(pinned, FakeTracker::new(), shell.clone());
    controller.refresh();

    controller.handle_item_click(&controller.items()[0]);

    assert_eq!(1, shell.launched().len());
    assert_eq!(r"C:\VSCode\code.exe", shell.launched()[0]);
}

#[test]
fn handle_item_click_activates_when_running_and_not_active() {
    let pinned = FakePins::new(vec![pin("Code", r"C:\Pins\Code.lnk", Some(r"C:\VSCode\code.exe"))]);
    let tracker = FakeTracker::with_windows(vec![win(1001, "Code", r"C:\VSCode\code.exe", 100, WindowRect::new(0, 0, 500, 500))]);
    tracker.set_foreground(Some(WindowInfo { handle: 9999, bounds: WindowRect::EMPTY })); // another window is active

    let controller = controller(pinned, tracker.clone(), FakeShell::new());
    controller.refresh();

    assert!(!controller.items()[0].is_active);
    controller.handle_item_click(&controller.items()[0]);

    assert_eq!(1, tracker.activated().len());
    assert_eq!(1001, tracker.activated()[0]);
    assert!(tracker.minimized().is_empty());
}

#[test]
fn handle_item_click_minimizes_when_running_and_already_active() {
    let pinned = FakePins::new(vec![pin("Code", r"C:\Pins\Code.lnk", Some(r"C:\VSCode\code.exe"))]);
    let tracker = FakeTracker::with_windows(vec![win(1001, "Code", r"C:\VSCode\code.exe", 100, WindowRect::new(0, 0, 500, 500))]);
    tracker.set_foreground(Some(WindowInfo { handle: 1001, bounds: WindowRect::new(0, 0, 500, 500) })); // Code is active

    let controller = controller(pinned, tracker.clone(), FakeShell::new());
    controller.refresh();

    assert!(controller.items()[0].is_active);
    controller.handle_item_click(&controller.items()[0]);

    assert_eq!(1, tracker.minimized().len());
    assert_eq!(1001, tracker.minimized()[0]);
    assert!(tracker.activated().is_empty());
}

#[test]
fn handle_item_middle_click_launches_new_instance() {
    let pinned = FakePins::new(vec![pin("Terminal", r"C:\Pins\wt.lnk", Some(r"C:\Windows\wt.exe"))]);
    let tracker = FakeTracker::with_windows(vec![win(1001, "Terminal", r"C:\Windows\wt.exe", 50, WindowRect::new(0, 0, 500, 500))]);
    let shell = FakeShell::new();

    let controller = controller(pinned, tracker, shell.clone());
    controller.refresh();

    controller.handle_item_middle_click(&controller.items()[0]);

    assert_eq!(1, shell.launched().len());
    assert_eq!(r"C:\Windows\wt.exe", shell.launched()[0]);
}

#[test]
fn handle_item_close_closes_all_windows_of_item() {
    let tracker = FakeTracker::with_windows(vec![
        win(1001, "Doc 1", r"C:\App\app.exe", 10, WindowRect::new(0, 0, 500, 500)),
        win(1002, "Doc 2", r"C:\App\app.exe", 10, WindowRect::new(0, 0, 500, 500)),
    ]);

    let controller = controller(FakePins::new(vec![]), tracker.clone(), FakeShell::new());
    controller.refresh();

    controller.handle_item_close(&controller.items()[0]);

    assert_eq!(2, tracker.closed().len());
    assert!(tracker.closed().contains(&1001));
    assert!(tracker.closed().contains(&1002));
}

#[test]
fn handle_item_terminate_terminates_process_id() {
    let tracker = FakeTracker::with_windows(vec![win(1001, "App", r"C:\App\app.exe", 777, WindowRect::new(0, 0, 500, 500))]);

    let controller = controller(FakePins::new(vec![]), tracker.clone(), FakeShell::new());
    controller.refresh();

    controller.handle_item_terminate(&controller.items()[0]);

    assert_eq!(1, tracker.terminated().len());
    assert_eq!(777, tracker.terminated()[0]);
}

#[test]
fn fullscreen_detector_updates_is_hidden_by_fullscreen() {
    let fullscreen = Arc::new(FakeFullscreen::default());
    fullscreen.fullscreen.store(true, Ordering::SeqCst);

    let controller = DockController::new(
        FakePins::new(vec![]),
        FakeTracker::new(),
        FakeShell::new(),
        Some(fullscreen.clone()),
        None,
        None,
        None,
    );
    controller.refresh();

    assert!(controller.is_hidden_by_fullscreen());

    fullscreen.fullscreen.store(false, Ordering::SeqCst);
    controller.refresh();

    assert!(!controller.is_hidden_by_fullscreen());
}

#[test]
fn refresh_preserves_icon_data_for_unpinned_running_app() {
    let dummy_icon = vec![0x42u8, 0x4D, 0x01, 0x02];
    let tracker = FakeTracker::with_windows(vec![OpenWindowInfo {
        icon_data: Some(dummy_icon.clone()),
        ..win(3001, "Custom App", r"C:\App\custom.exe", 999, WindowRect::new(0, 0, 500, 500))
    }]);

    let controller = controller(FakePins::new(vec![]), tracker, FakeShell::new());
    controller.refresh();

    let items = controller.items();
    assert_eq!(1, items.len());
    assert_eq!(Some(dummy_icon), items[0].icon_data);
    assert!(!items[0].is_pinned);
    assert!(items[0].is_running);
}

#[test]
fn window_flashing_sets_is_flashing_on_matching_dock_item() {
    let pinned = FakePins::new(vec![pin("App", r"C:\Pins\app.lnk", Some(r"C:\app.exe"))]);
    let tracker = FakeTracker::with_windows(vec![win(1001, "App", r"C:\app.exe", 100, WindowRect::new(0, 0, 400, 400))]);

    let controller = controller(pinned, tracker.clone(), FakeShell::new());
    controller.refresh();

    assert!(!controller.items()[0].is_flashing);

    tracker.trigger_window_flashing(1001);

    assert!(controller.items()[0].is_flashing);
}

#[test]
fn handle_item_click_clears_flashing_state() {
    let pinned = FakePins::new(vec![pin("App", r"C:\Pins\app.lnk", Some(r"C:\app.exe"))]);
    let tracker = FakeTracker::with_windows(vec![win(1001, "App", r"C:\app.exe", 100, WindowRect::new(0, 0, 400, 400))]);

    let controller = controller(pinned, tracker.clone(), FakeShell::new());
    controller.refresh();

    tracker.trigger_window_flashing(1001);
    assert!(controller.items()[0].is_flashing);

    controller.handle_item_click(&controller.items()[0]);
    controller.refresh();

    assert!(!controller.items()[0].is_flashing);
}

#[test]
fn alt_tab_preserves_unpinned_apps_order() {
    let win_a = win(1001, "AppA", r"C:\AppA.exe", 10, WindowRect::new(0, 0, 400, 400));
    let win_b = win(1002, "AppB", r"C:\AppB.exe", 20, WindowRect::new(0, 0, 400, 400));
    let tracker = FakeTracker::with_windows(vec![win_a.clone(), win_b.clone()]);
    tracker.set_foreground(Some(WindowInfo { handle: 1001, bounds: WindowRect::new(0, 0, 400, 400) }));

    let controller = controller(FakePins::new(vec![]), tracker.clone(), FakeShell::new());
    controller.refresh();

    let items = controller.items();
    assert_eq!(2, items.len());
    assert_eq!(r"C:\AppA.exe", items[0].id);
    assert_eq!(r"C:\AppB.exe", items[1].id);

    // Alt+Tab: winB comes to top of Z-order in EnumWindows
    tracker.set_windows(vec![win_b, win_a]);
    tracker.trigger_foreground_change(WindowInfo { handle: 1002, bounds: WindowRect::new(0, 0, 400, 400) });

    let items = controller.items();
    assert_eq!(2, items.len());
    // Order MUST stay stable: AppA first, AppB second!
    assert_eq!(r"C:\AppA.exe", items[0].id);
    assert_eq!(r"C:\AppB.exe", items[1].id);
    assert!(!items[0].is_active);
    assert!(items[1].is_active);
}

fn debounced_controller(tracker: Arc<FakeTracker>, time: Arc<ManualTimeProvider>) -> DockController {
    DockController::new(
        FakePins::new(vec![]),
        tracker,
        FakeShell::new(),
        None,
        Some(TimeSpan::from_millis(100)),
        None,
        Some(time),
    )
}

#[test]
fn refresh_debounce_pencere_olaylarini_tek_yenilemeye_indirir() {
    let tracker = FakeTracker::new();
    let time = ManualTimeProvider::new();
    let controller = debounced_controller(tracker.clone(), time.clone());

    let refresh_count = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&refresh_count);
    controller.items_changed().subscribe(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    tracker.trigger_windows_change();
    tracker.trigger_windows_change();
    tracker.trigger_windows_change();
    assert_eq!(0, refresh_count.load(Ordering::SeqCst));

    time.fire_last();

    assert_eq!(1, refresh_count.load(Ordering::SeqCst));
}

#[test]
fn refresh_debounce_on_plan_degisimi_hemen_yeniler_ve_bekleyeni_iptal_eder() {
    let tracker = FakeTracker::new();
    let time = ManualTimeProvider::new();
    let controller = debounced_controller(tracker.clone(), time.clone());

    let refresh_count = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&refresh_count);
    controller.items_changed().subscribe(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    tracker.trigger_windows_change();
    tracker.trigger_foreground_change(WindowInfo { handle: 42, bounds: WindowRect::EMPTY });
    assert_eq!(1, refresh_count.load(Ordering::SeqCst));

    time.fire_last();
    assert_eq!(1, refresh_count.load(Ordering::SeqCst));
}

#[test]
fn move_pinned_sabit_sirayi_degistirir_ve_listeyi_yeniler() {
    let inner = FakePins::new(vec![
        pin("A", r"C:\Pins\A.lnk", None),
        pin("B", r"C:\Pins\B.lnk", None),
        pin("C", r"C:\Pins\C.lnk", None),
    ]);
    let stored: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(strs(&["A.lnk", "B.lnk", "C.lnk"])));
    let (load, save) = (Arc::clone(&stored), Arc::clone(&stored));
    let ordered = Arc::new(OrderedPinnedAppsService::new(
        inner,
        move || load.lock().unwrap().clone(),
        move |o| {
            *save.lock().unwrap() = o.to_vec();
            Ok(())
        },
        None,
    ));

    let controller = controller(ordered, FakeTracker::new(), FakeShell::new());
    controller.refresh();
    assert!(controller.can_reorder_pinned());

    controller.move_pinned(r"C:\Pins\A.lnk", 2);

    let titles: Vec<String> = controller.items().iter().map(|i| i.title.clone()).collect();
    assert_eq!(strs(&["B", "C", "A"]), titles);
    assert_eq!(strs(&["B.lnk", "C.lnk", "A.lnk"]), *stored.lock().unwrap());
}

#[test]
fn move_pinned_siralanamaz_kaynakta_ve_bilinmeyen_ogede_sessiz_gecer() {
    let inner = FakePins::new(vec![pin("A", r"C:\Pins\A.lnk", None)]);
    let controller = controller(inner, FakeTracker::new(), FakeShell::new());
    controller.refresh();

    assert!(!controller.can_reorder_pinned());
    controller.move_pinned(r"C:\Pins\A.lnk", 0);
    controller.move_pinned(r"C:\Pins\Yok.lnk", 3);

    assert_eq!(1, controller.items().len());
}

#[test]
fn dock_controller_grouping_mode_never_ayrik_simgeler_uretir() {
    let win1 = win(1001, "Window 1", r"C:\App.exe", 10, WindowRect::new(0, 0, 400, 400));
    let win2 = win(1002, "Window 2", r"C:\App.exe", 10, WindowRect::new(0, 0, 400, 400));
    let tracker = FakeTracker::with_windows(vec![win1, win2]);
    tracker.set_foreground(Some(WindowInfo { handle: 1001, bounds: WindowRect::new(0, 0, 400, 400) }));

    let controller = controller(FakePins::new(vec![]), tracker, FakeShell::new());
    controller.set_grouping_mode(DockGroupingMode::Never);
    controller.refresh();

    let items = controller.items();
    assert_eq!(2, items.len());
    assert_eq!(1, items[0].windows.as_ref().expect("Windows dolu olmalı").len());
    assert_eq!(1, items[1].windows.as_ref().expect("Windows dolu olmalı").len());
}

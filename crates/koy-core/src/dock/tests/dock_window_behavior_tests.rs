use super::fakes::{controller, FakePins, FakeShell, FakeTracker};
use crate::dock::{
    DockClickKind, DockClickPlan, DockClickPolicy, DockItem, DockLeftClickAction, DockMonitorFilter, DockWindowEntry,
};
use crate::shell::{OpenWindowInfo, WindowInfo, WindowRect};

const LEFT: WindowRect = WindowRect::new(0, 0, 1920, 1080);
const RIGHT: WindowRect = WindowRect::new(1920, 0, 3840, 1080);

fn win(handle: isize, bounds: WindowRect) -> DockWindowEntry {
    DockWindowEntry::new(handle, format!("t{handle}"), bounds)
}

fn item(id: &str, pinned: bool, active: bool, windows: Vec<DockWindowEntry>) -> DockItem {
    DockItem {
        id: id.to_string(),
        title: id.to_string(),
        file_path: Some(id.to_string()),
        target_path: Some(id.to_string()),
        icon_data: None,
        is_pinned: pinned,
        is_running: !windows.is_empty(),
        is_active: active,
        window_handles: windows.iter().map(|w| w.handle).collect(),
        process_id: if windows.is_empty() { 0 } else { 7 },
        is_flashing: true,
        badge_count: 3,
        windows: Some(windows),
        app_user_model_id: None,
    }
}

// --- Click policy ---

#[test]
fn minimize_if_active_minimizes_when_active_else_activates() {
    let handles = [10, 20];

    assert_eq!(
        DockClickPlan::new(DockClickKind::Minimize, 10),
        DockClickPolicy::decide_left_click(DockLeftClickAction::MinimizeIfActive, &handles, 20, true)
    );
    assert_eq!(
        DockClickPlan::new(DockClickKind::Activate, 10),
        DockClickPolicy::decide_left_click(DockLeftClickAction::MinimizeIfActive, &handles, 99, false)
    );
}

#[test]
fn undefined_action_falls_back_to_todays_behaviour() {
    // C# `(DockLeftClickAction)42` Rust'ta kurulamaz; "bugünkü davranış" kolu `MinimizeIfActive` ile paylaşıldığından
    // davranış onun üzerinden sınanır.
    let plan = DockClickPolicy::decide_left_click(DockLeftClickAction::MinimizeIfActive, &[10], 10, true);

    assert_eq!(DockClickPlan::new(DockClickKind::Minimize, 10), plan);
}

#[test]
fn bring_to_front_never_minimizes() {
    let plan = DockClickPolicy::decide_left_click(DockLeftClickAction::BringToFront, &[10, 20], 10, true);

    assert_eq!(DockClickPlan::new(DockClickKind::Activate, 10), plan);
}

#[test]
fn cycle_moves_to_next_window_and_wraps() {
    let handles = [10, 20, 30];

    assert_eq!(20, DockClickPolicy::decide_left_click(DockLeftClickAction::CycleWindows, &handles, 10, true).handle);
    assert_eq!(10, DockClickPolicy::decide_left_click(DockLeftClickAction::CycleWindows, &handles, 30, true).handle);
}

#[test]
fn cycle_activates_first_window_when_app_is_not_active() {
    let plan = DockClickPolicy::decide_left_click(DockLeftClickAction::CycleWindows, &[10, 20], 99, false);

    assert_eq!(DockClickPlan::new(DockClickKind::Activate, 10), plan);
}

#[test]
fn cycle_with_inconsistent_foreground_starts_from_first() {
    let plan = DockClickPolicy::decide_left_click(DockLeftClickAction::CycleWindows, &[10, 20], 99, true);

    assert_eq!(10, plan.handle);
}

#[test]
fn no_windows_means_no_plan() {
    for action in [
        DockLeftClickAction::BringToFront,
        DockLeftClickAction::MinimizeIfActive,
        DockLeftClickAction::CycleWindows,
        DockLeftClickAction::ShowThumbnails,
    ] {
        assert_eq!(DockClickKind::None, DockClickPolicy::decide_left_click(action, &[], 0, false).kind, "{action:?}");
    }
}

#[test]
fn show_thumbnails_asks_ui_to_open_preview() {
    let plan = DockClickPolicy::decide_left_click(DockLeftClickAction::ShowThumbnails, &[10], 0, false);

    assert_eq!(DockClickPlan::new(DockClickKind::ShowThumbnails, 10), plan);
}

// --- Monitor filter ---

#[test]
fn is_on_monitor_uses_window_center() {
    assert!(DockMonitorFilter::is_on_monitor(WindowRect::new(1800, 100, 2000, 300), LEFT)); // center 1900
    assert!(!DockMonitorFilter::is_on_monitor(WindowRect::new(1800, 100, 2200, 300), LEFT)); // center 2000
    assert!(DockMonitorFilter::is_on_monitor(WindowRect::new(1800, 100, 2200, 300), RIGHT));
}

#[test]
fn is_on_monitor_keeps_minimized_windows_everywhere() {
    let minimized = WindowRect::new(-32000, -32000, -31840, -31972);

    assert!(DockMonitorFilter::is_on_monitor(minimized, LEFT));
    assert!(DockMonitorFilter::is_on_monitor(minimized, RIGHT));
}

#[test]
fn filter_drops_unpinned_item_without_windows_on_monitor() {
    let items = [item("a", false, false, vec![win(1, WindowRect::new(2000, 0, 2400, 400))])];

    assert!(DockMonitorFilter::apply(&items, LEFT, 0).is_empty());
}

#[test]
fn filter_keeps_pinned_item_as_not_running() {
    let items = [item("a", true, true, vec![win(1, WindowRect::new(2000, 0, 2400, 400))])];

    let result = DockMonitorFilter::apply(&items, LEFT, 1);

    assert_eq!(1, result.len());
    let only = &result[0];
    assert!(only.is_pinned);
    assert!(!only.is_running);
    assert!(!only.is_active);
    assert!(only.window_handles.is_empty());
    assert!(only.windows.as_ref().unwrap().is_empty());
    assert_eq!(0, only.badge_count);
}

#[test]
fn filter_narrows_mixed_item_and_recomputes_active() {
    let on_left = win(1, WindowRect::new(100, 100, 500, 500));
    let on_right = win(2, WindowRect::new(2000, 100, 2400, 500));
    let items = [item("a", false, true, vec![on_left, on_right])];

    let result = DockMonitorFilter::apply(&items, LEFT, 2);
    assert_eq!(1, result.len());
    assert_eq!(vec![1isize], result[0].window_handles);
    assert!(!result[0].is_active);

    let result = DockMonitorFilter::apply(&items, RIGHT, 2);
    assert_eq!(1, result.len());
    assert_eq!(vec![2isize], result[0].window_handles);
    assert!(result[0].is_active);
}

#[test]
fn filter_passes_everything_through_when_all_windows_match() {
    let items = [item("a", false, false, vec![win(1, WindowRect::new(100, 100, 500, 500))])];

    let result = DockMonitorFilter::apply(&items, LEFT, 0);

    // C# `Assert.Same`: aynı nesne; Rust'ta değer eşitliği (öğe değişmeden geçer).
    assert_eq!(1, result.len());
    assert_eq!(items[0], result[0]);
}

#[test]
fn filter_passes_items_without_window_info() {
    let items = [DockItem { windows: None, ..item("a", true, false, vec![]) }];

    let result = DockMonitorFilter::apply(&items, LEFT, 0);

    assert_eq!(1, result.len());
    assert_eq!(items[0], result[0]);
}

// --- Controller wiring ---

fn build(handles: &[isize]) -> (crate::dock::DockController, std::sync::Arc<FakeTracker>) {
    let tracker = FakeTracker::new();
    tracker.set_windows(
        handles
            .iter()
            .map(|h| OpenWindowInfo::new(*h, format!("w{h}"), Some(r"C:\app.exe".to_string()), 5, WindowRect::new(0, 0, 100, 100)))
            .collect(),
    );
    let controller = controller(FakePins::new(vec![]), tracker.clone(), FakeShell::new());
    controller.refresh();
    (controller, tracker)
}

#[test]
fn default_left_click_matches_previous_behaviour() {
    let (controller, tracker) = build(&[10, 20]);
    assert_eq!(DockLeftClickAction::MinimizeIfActive, controller.left_click_action());

    controller.handle_item_click(&controller.items()[0]);
    assert_eq!(vec![10isize], tracker.activated());

    tracker.set_foreground(Some(WindowInfo { handle: 10, bounds: WindowRect::new(0, 0, 100, 100) }));
    controller.refresh();
    controller.handle_item_click(&controller.items()[0]);
    assert_eq!(vec![10isize], tracker.minimized());
}

#[test]
fn cycle_click_activates_next_window() {
    let (controller, tracker) = build(&[10, 20]);
    controller.set_left_click_action(DockLeftClickAction::CycleWindows);
    tracker.set_foreground(Some(WindowInfo { handle: 10, bounds: WindowRect::new(0, 0, 100, 100) }));
    controller.refresh();

    controller.handle_item_click(&controller.items()[0]);

    assert_eq!(vec![20isize], tracker.activated());
    assert!(tracker.minimized().is_empty());
}

#[test]
fn show_thumbnails_falls_back_to_default_when_ui_does_not_handle_it() {
    let (controller, tracker) = build(&[10]);
    controller.set_left_click_action(DockLeftClickAction::ShowThumbnails);

    controller.handle_item_click(&controller.items()[0]);

    assert_eq!(vec![10isize], tracker.activated());
}

#[test]
fn foreground_move_refreshes_only_when_enabled() {
    let (controller, tracker) = build(&[10]);
    tracker.trigger_foreground_change(WindowInfo { handle: 10, bounds: WindowRect::new(0, 0, 100, 100) });
    let baseline = tracker.scan_count();

    // Same window, new bounds: ignored by default (today's behaviour).
    tracker.trigger_foreground_change(WindowInfo { handle: 10, bounds: WindowRect::new(50, 0, 150, 100) });
    assert_eq!(baseline, tracker.scan_count());

    controller.set_refresh_on_foreground_move(true);
    tracker.trigger_foreground_change(WindowInfo { handle: 10, bounds: WindowRect::new(90, 0, 190, 100) });
    assert_eq!(baseline + 1, tracker.scan_count());
}

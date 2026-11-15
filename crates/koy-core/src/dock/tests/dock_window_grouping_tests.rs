use crate::dock::{DockGroupingMode, DockLabelMode, DockLeftClickAction, DockWindowFact, DockWindowGrouping, DockWindowMonitorScope};
use crate::settings::AppSettings;

fn w(name: &str, monitor: i32, handle: isize) -> DockWindowFact {
    DockWindowFact::new(name, monitor, handle)
}

fn sample() -> Vec<DockWindowFact> {
    vec![w("chrome", 1, 10), w("code", 2, 20), w("Chrome", 2, 30), w("explorer", 1, 40), w("chrome", 1, 50)]
}

#[test]
fn never_gives_one_group_per_window_in_input_order() {
    let groups = DockWindowGrouping::group(&sample(), DockGroupingMode::Never, 0);

    assert_eq!(5, groups.len());
    assert_eq!(vec![10isize], groups[0].handles);
    assert_eq!(vec![20isize], groups[1].handles);
    assert_eq!(vec![30isize], groups[2].handles);
    assert_eq!(vec![40isize], groups[3].handles);
    assert_eq!(vec![50isize], groups[4].handles);
}

#[test]
fn always_groups_case_insensitively_in_first_appearance_order() {
    let groups = DockWindowGrouping::group(&sample(), DockGroupingMode::Always, 100);

    assert_eq!(3, groups.len());
    assert_eq!("chrome", groups[0].process_name);
    assert_eq!(vec![10isize, 30, 50], groups[0].handles);
    assert_eq!("code", groups[1].process_name);
    assert_eq!(vec![20isize], groups[1].handles);
    assert_eq!("explorer", groups[2].process_name);
    assert_eq!(vec![40isize], groups[2].handles);
}

#[test]
fn always_is_deterministic_for_the_same_input() {
    let first = DockWindowGrouping::group(&sample(), DockGroupingMode::Always, 0);
    let second = DockWindowGrouping::group(&sample(), DockGroupingMode::Always, 0);

    // Record eşitliği IReadOnlyList için referansa bakar; içerik karşılaştırması elle yapılır.
    fn flatten(groups: &[crate::dock::DockWindowGroup]) -> Vec<String> {
        groups
            .iter()
            .map(|g| format!("{}:{}", g.process_name, g.handles.iter().map(|h| h.to_string()).collect::<Vec<_>>().join(",")))
            .collect()
    }

    assert_eq!(flatten(&first), flatten(&second));
}

#[test]
fn when_space_runs_out_keeps_windows_separate_while_they_fit() {
    let groups = DockWindowGrouping::group(&sample(), DockGroupingMode::WhenSpaceRunsOut, 5);

    assert_eq!(5, groups.len());
    assert!(groups.iter().all(|g| g.handles.len() == 1));
}

#[test]
fn when_space_runs_out_groups_once_windows_exceed_the_slots() {
    let groups = DockWindowGrouping::group(&sample(), DockGroupingMode::WhenSpaceRunsOut, 4);

    assert_eq!(3, groups.len());
    assert_eq!(3, groups[0].handles.len());
}

#[test]
fn empty_input_gives_no_groups_in_any_mode() {
    for mode in [DockGroupingMode::Never, DockGroupingMode::Always, DockGroupingMode::WhenSpaceRunsOut] {
        assert!(DockWindowGrouping::group(&[], mode, 3).is_empty(), "{mode:?}");
    }
}

#[test]
fn undefined_mode_from_the_file_keeps_windows_separate() {
    // C# `(DockGroupingMode)7` Rust'ta kurulamaz; "gruplamasız" kolu `Never` ile paylaşıldığından davranış onun üzerinden sınanır.
    let groups = DockWindowGrouping::group(&sample(), DockGroupingMode::Never, 0);

    assert_eq!(5, groups.len());
}

#[test]
fn missing_process_name_reads_as_empty_in_every_mode() {
    // C# `null!` işlem adı; Rust'ta `String` null olamaz, boş metinle karşılığı denenir ("" okunur).
    let windows = [w("", 1, 1), w("", 1, 2)];

    assert!(DockWindowGrouping::group(&windows, DockGroupingMode::Never, 0).iter().all(|g| g.process_name.is_empty()));
    let always = DockWindowGrouping::group(&windows, DockGroupingMode::Always, 0);
    assert_eq!(1, always.len());
    assert_eq!("", always[0].process_name);
}

#[test]
#[should_panic]
fn negative_slot_count_is_rejected() {
    DockWindowGrouping::group(&sample(), DockGroupingMode::WhenSpaceRunsOut, -1);
}

#[test]
fn all_monitors_keeps_every_window() {
    let filtered = DockWindowGrouping::filter_by_monitor(&sample(), DockWindowMonitorScope::AllMonitors, 2);

    assert_eq!(sample(), filtered);
}

#[test]
fn current_monitor_keeps_only_that_monitor_in_input_order() {
    let filtered = DockWindowGrouping::filter_by_monitor(&sample(), DockWindowMonitorScope::CurrentMonitor, 1);

    assert_eq!(vec![w("chrome", 1, 10), w("explorer", 1, 40), w("chrome", 1, 50)], filtered);
}

#[test]
fn filtered_windows_group_into_the_expected_entries() {
    let filtered = DockWindowGrouping::filter_by_monitor(&sample(), DockWindowMonitorScope::CurrentMonitor, 2);
    let groups = DockWindowGrouping::group(&filtered, DockGroupingMode::Always, 0);

    assert_eq!(2, groups.len());
    assert_eq!(vec![20isize], groups[0].handles);
    assert_eq!(vec![30isize], groups[1].handles);
}

#[test]
fn display_settings_default_to_todays_behaviour() {
    let dock = AppSettings::default().dock;

    assert_eq!(DockGroupingMode::Always, dock.grouping_mode);
    assert_eq!(DockWindowMonitorScope::AllMonitors, dock.window_monitor_scope);
    assert_eq!(DockLeftClickAction::MinimizeIfActive, dock.left_click_action);
    assert_eq!(DockLabelMode::Never, dock.label_mode);
}

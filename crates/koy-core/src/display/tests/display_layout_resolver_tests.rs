use crate::display::*;
use crate::shell::WindowRect;

fn primary_mon() -> DisplayMonitor {
    DisplayMonitor::new(r"\\.\DISPLAY1", "Main Display", WindowRect::new(0, 0, 1920, 1080), WindowRect::new(0, 0, 1920, 1040), true)
        .with_dpi(1.0, 1.0)
        .with_handle(1001)
}

fn secondary_mon() -> DisplayMonitor {
    DisplayMonitor::new(r"\\.\DISPLAY2", "Side Monitor", WindowRect::new(1920, 0, 3840, 1080), WindowRect::new(1920, 0, 3840, 1080), false)
        .with_dpi(1.25, 1.25)
        .with_handle(1002)
}

fn third_mon() -> DisplayMonitor {
    DisplayMonitor::new(r"\\.\DISPLAY3", "Top Display", WindowRect::new(0, -1080, 1920, 0), WindowRect::new(0, -1080, 1920, 0), false)
        .with_dpi(1.0, 1.0)
        .with_handle(1003)
}

#[test]
fn resolve_target_monitors_empty_list_returns_empty() {
    assert!(DisplayLayoutResolver::resolve_target_monitors_with_mode(&[], "Primary", MonitorTargetMode::Primary).is_empty());
    assert!(DisplayLayoutResolver::resolve_target_monitors_with_mode(&[], "All", MonitorTargetMode::All).is_empty());
    assert!(DisplayLayoutResolver::resolve_target_monitors_with_mode(&[], "DISPLAY1", MonitorTargetMode::Specific).is_empty());
}

#[test]
fn resolve_target_monitors_primary_mode_returns_primary() {
    let monitors = [secondary_mon(), primary_mon(), third_mon()];
    let resolved = DisplayLayoutResolver::resolve_target_monitors_with_mode(&monitors, "Primary", MonitorTargetMode::Primary);

    assert_eq!(1, resolved.len());
    assert_eq!(r"\\.\DISPLAY1", resolved[0].id);
    assert!(resolved[0].is_primary);
    assert_eq!(1001, resolved[0].handle);
}

#[test]
fn resolve_target_monitors_primary_mode_no_primary_marked_returns_first() {
    let no_primary1 = DisplayMonitor { is_primary: false, ..secondary_mon() };
    let no_primary2 = DisplayMonitor { is_primary: false, ..third_mon() };
    let monitors = [no_primary1, no_primary2];

    let resolved = DisplayLayoutResolver::resolve_target_monitors_with_mode(&monitors, "Primary", MonitorTargetMode::Primary);

    assert_eq!(1, resolved.len());
    assert_eq!(r"\\.\DISPLAY2", resolved[0].id);
}

#[test]
fn resolve_target_monitors_all_mode_returns_all_monitors() {
    let monitors = [primary_mon(), secondary_mon(), third_mon()];
    let resolved = DisplayLayoutResolver::resolve_target_monitors_with_mode(&monitors, "All", MonitorTargetMode::All);

    assert_eq!(3, resolved.len());
    assert!(resolved.contains(&primary_mon()));
    assert!(resolved.contains(&secondary_mon()));
    assert!(resolved.contains(&third_mon()));
}

#[test]
fn resolve_target_monitors_specific_mode_matches_id() {
    let monitors = [primary_mon(), secondary_mon(), third_mon()];
    let resolved = DisplayLayoutResolver::resolve_target_monitors_with_mode(&monitors, r"\\.\DISPLAY2", MonitorTargetMode::Specific);

    assert_eq!(1, resolved.len());
    assert_eq!(r"\\.\DISPLAY2", resolved[0].id);
    assert_eq!(1002, resolved[0].handle);
}

#[test]
fn resolve_target_monitors_specific_mode_matches_name() {
    let monitors = [primary_mon(), secondary_mon(), third_mon()];
    let resolved = DisplayLayoutResolver::resolve_target_monitors_with_mode(&monitors, "Side Monitor", MonitorTargetMode::Specific);

    assert_eq!(1, resolved.len());
    assert_eq!(r"\\.\DISPLAY2", resolved[0].id);
}

#[test]
fn resolve_target_monitors_specific_mode_case_insensitive_match() {
    let monitors = [primary_mon(), secondary_mon()];
    let resolved = DisplayLayoutResolver::resolve_target_monitors_with_mode(&monitors, r"\\.\display2", MonitorTargetMode::Specific);

    assert_eq!(1, resolved.len());
    assert_eq!(r"\\.\DISPLAY2", resolved[0].id);
}

#[test]
fn resolve_target_monitors_specific_mode_not_found_falls_back_to_primary() {
    let monitors = [secondary_mon(), primary_mon()];
    let resolved = DisplayLayoutResolver::resolve_target_monitors_with_mode(&monitors, "NonExistentDisplay", MonitorTargetMode::Specific);

    assert_eq!(1, resolved.len());
    assert_eq!(r"\\.\DISPLAY1", resolved[0].id);
    assert!(resolved[0].is_primary);
}

#[test]
fn resolve_target_monitors_specific_mode_not_found_and_no_primary_falls_back_to_first() {
    let no_primary1 = DisplayMonitor { is_primary: false, ..secondary_mon() };
    let no_primary2 = DisplayMonitor { is_primary: false, ..third_mon() };
    let monitors = [no_primary1, no_primary2];

    let resolved = DisplayLayoutResolver::resolve_target_monitors_with_mode(&monitors, "Unknown", MonitorTargetMode::Specific);

    assert_eq!(1, resolved.len());
    assert_eq!(r"\\.\DISPLAY2", resolved[0].id);
}

#[test]
fn parse_mode_parses_setting_string_correctly() {
    let rows: [(Option<&str>, MonitorTargetMode); 10] = [
        (Some("All"), MonitorTargetMode::All),
        (Some("all"), MonitorTargetMode::All),
        (Some("ALL"), MonitorTargetMode::All),
        (Some("Primary"), MonitorTargetMode::Primary),
        (Some("primary"), MonitorTargetMode::Primary),
        (Some(""), MonitorTargetMode::Primary),
        (None, MonitorTargetMode::Primary),
        (Some("   "), MonitorTargetMode::Primary),
        (Some(r"\\.\DISPLAY1"), MonitorTargetMode::Specific),
        (Some("HDMI-1"), MonitorTargetMode::Specific),
    ];
    for (input, expected) in rows {
        assert_eq!(expected, DisplayLayoutResolver::parse_mode(input), "{input:?}");
    }
}

#[test]
fn resolve_target_monitors_with_string_overload_resolves_correctly() {
    let monitors = [primary_mon(), secondary_mon()];

    let all = DisplayLayoutResolver::resolve_target_monitors(&monitors, "All");
    assert_eq!(2, all.len());

    let primary = DisplayLayoutResolver::resolve_target_monitors(&monitors, "Primary");
    assert_eq!(1, primary.len());
    assert_eq!(r"\\.\DISPLAY1", primary[0].id);

    let specific = DisplayLayoutResolver::resolve_target_monitors(&monitors, r"\\.\DISPLAY2");
    assert_eq!(1, specific.len());
    assert_eq!(r"\\.\DISPLAY2", specific[0].id);
}

#[test]
fn display_monitor_handle_preserved_and_defaults_to_zero() {
    let default_mon = DisplayMonitor::new("D0", "Default", WindowRect::EMPTY, WindowRect::EMPTY, true);
    assert_eq!(0, default_mon.handle);

    let with_handle = DisplayMonitor { handle: 0x12345, ..default_mon };
    assert_eq!(0x12345, with_handle.handle);
}

/// `DisplayMonitorTests.cs` (DisplayMonitorNumberTests).
#[test]
fn number_is_the_trailing_digits_of_the_device_name() {
    let rows = [(r"\\.\DISPLAY1", 1), (r"\\.\DISPLAY12", 12), ("", 0), ("kimlik", 0)];
    for (device_name, expected) in rows {
        let monitor = DisplayMonitor::new("id", "ad", WindowRect::default(), WindowRect::default(), false).with_device_name(device_name);
        assert_eq!(expected, monitor.number(), "{device_name:?}");
    }
}

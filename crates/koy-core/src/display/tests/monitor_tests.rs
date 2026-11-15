use crate::display::*;
use crate::event::Event;
use crate::shell::WindowRect;
use crate::text;

struct FakeMonitorService {
    monitors: Vec<DisplayMonitor>,
    monitors_changed: Event<()>,
}

impl FakeMonitorService {
    fn new(monitors: Vec<DisplayMonitor>) -> Self {
        FakeMonitorService { monitors, monitors_changed: Event::new() }
    }

    #[allow(dead_code)]
    fn trigger_monitors_changed(&self) {
        self.monitors_changed.invoke(&());
    }
}

impl IMonitorService for FakeMonitorService {
    fn monitors_changed(&self) -> &Event<()> {
        &self.monitors_changed
    }

    fn get_monitors(&self) -> Vec<DisplayMonitor> {
        self.monitors.clone()
    }

    fn get_primary_monitor(&self) -> Option<DisplayMonitor> {
        self.monitors.iter().find(|m| m.is_primary).or_else(|| self.monitors.first()).cloned()
    }

    fn get_monitor(&self, id_or_name: &str) -> Option<DisplayMonitor> {
        self.monitors
            .iter()
            .find(|m| text::eq_ignore_case(&m.id, id_or_name) || text::eq_ignore_case(&m.name, id_or_name))
            .cloned()
    }

    fn start(&self) {}
    fn stop(&self) {}
}

fn close(expected: f64, actual: f64, tolerance: f64) {
    assert!((expected - actual).abs() <= tolerance, "beklenen {expected}, gelen {actual}");
}

#[test]
fn display_monitor_calculates_dip_dimensions_correctly() {
    let monitor = DisplayMonitor::new(
        r"\\.\DISPLAY1",
        "Built-in Display",
        WindowRect::new(0, 0, 3840, 2160),
        WindowRect::new(0, 0, 3840, 2112),
        true,
    )
    .with_dpi(1.5, 1.5);

    // Assert.Equal(expected, actual, precision: 1) → 1 ondalık basamak.
    close(2560.0, monitor.width_dip(), 0.05);
    close(1440.0, monitor.height_dip(), 0.05);
    assert_eq!(0.0, monitor.left_dip());
    assert_eq!(0.0, monitor.top_dip());
    assert!(monitor.is_primary);
}

#[test]
fn display_monitor_calculates_secondary_monitor_offset() {
    let secondary = DisplayMonitor::new(
        r"\\.\DISPLAY2",
        "External Monitor",
        WindowRect::new(1920, 0, 3840, 1080),
        WindowRect::new(1920, 0, 3840, 1080),
        false,
    )
    .with_dpi(1.0, 1.0);

    assert_eq!(1920.0, secondary.left_dip());
    assert_eq!(0.0, secondary.top_dip());
    assert_eq!(1920.0, secondary.width_dip());
    assert_eq!(1080.0, secondary.height_dip());
    assert!(!secondary.is_primary);
}

#[test]
fn get_primary_monitor_returns_primary_display() {
    let service = FakeMonitorService::new(vec![
        DisplayMonitor::new("D2", "Ext", WindowRect::new(1920, 0, 3840, 1080), WindowRect::EMPTY, false),
        DisplayMonitor::new("D1", "Primary", WindowRect::new(0, 0, 1920, 1080), WindowRect::EMPTY, true),
    ]);

    let primary = service.get_primary_monitor().unwrap();

    assert_eq!("D1", primary.id);
    assert!(primary.is_primary);
}

#[test]
fn get_monitor_finds_by_id_or_name() {
    let service = FakeMonitorService::new(vec![DisplayMonitor::new("MON1", "LG UltraWide", WindowRect::new(0, 0, 2560, 1080), WindowRect::EMPTY, true)]);

    assert!(service.get_monitor("MON1").is_some());
    assert!(service.get_monitor("LG UltraWide").is_some());
    assert!(service.get_monitor("Unknown").is_none());
}

#[test]
fn resolve_target_monitor_handles_primary_specific_and_fallbacks() {
    let primary = DisplayMonitor::new("D1", "Primary Display", WindowRect::new(0, 0, 1920, 1080), WindowRect::EMPTY, true);
    let secondary = DisplayMonitor::new("D2", "Side Display", WindowRect::new(1920, 0, 3840, 1080), WindowRect::EMPTY, false);
    let service = FakeMonitorService::new(vec![primary, secondary]);

    // 1. Specific
    let specific = service.get_monitor("D2").or_else(|| service.get_primary_monitor());
    assert_eq!(Some("D2".to_string()), specific.map(|m| m.id));

    // 2. Name
    let by_name = service.get_monitor("Side Display").or_else(|| service.get_primary_monitor());
    assert_eq!(Some("D2".to_string()), by_name.map(|m| m.id));

    // 3. Fallback when unknown
    let fallback = service.get_monitor("NonExistent").or_else(|| service.get_primary_monitor());
    assert_eq!(Some("D1".to_string()), fallback.map(|m| m.id));

    // 4. Fallback when Primary
    let primary_target = service.get_monitor("Primary").or_else(|| service.get_primary_monitor());
    assert_eq!(Some("D1".to_string()), primary_target.map(|m| m.id));
}

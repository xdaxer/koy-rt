use crate::display::*;
use crate::shell::WindowRect;

fn mon(id: &str, work: WindowRect, dpi: f64, primary: bool) -> DisplayMonitor {
    DisplayMonitor::new(id, id, WindowRect::new(0, 0, 1920, 1080), work, primary).with_dpi(dpi, dpi)
}

#[test]
fn work_area_change_does_not_change_the_signature() {
    let a = MonitorLayoutSignature::compute(&[mon("m", WindowRect::new(0, 0, 1920, 1034), 1.0, true)]);
    let b = MonitorLayoutSignature::compute(&[mon("m", WindowRect::new(0, 0, 1920, 1080), 1.0, true)]);
    let c = MonitorLayoutSignature::compute(&[mon("m", WindowRect::new(0, 0, 1920, 1032), 1.0, true)]);

    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn dpi_bounds_and_primary_changes_do_change_it() {
    let work = WindowRect::new(0, 0, 1920, 1080);
    let base_sig = MonitorLayoutSignature::compute(&[mon("m", work, 1.0, true)]);

    assert_ne!(base_sig, MonitorLayoutSignature::compute(&[mon("m", work, 1.5, true)]));
    assert_ne!(base_sig, MonitorLayoutSignature::compute(&[mon("m", work, 1.0, false)]));
    assert_ne!(base_sig, MonitorLayoutSignature::compute(&[mon("m", work, 1.0, true), mon("n", work, 1.0, false)]));
    assert_ne!(
        base_sig,
        MonitorLayoutSignature::compute(&[DisplayMonitor::new("m", "m", WindowRect::new(0, 0, 2560, 1440), work, true)])
    );
}

#[test]
fn order_of_monitors_does_not_matter() {
    let w = WindowRect::new(0, 0, 1920, 1080);
    assert_eq!(
        MonitorLayoutSignature::compute(&[mon("a", w, 1.0, true), mon("b", w, 1.0, false)]),
        MonitorLayoutSignature::compute(&[mon("b", w, 1.0, false), mon("a", w, 1.0, true)])
    );
}

/// .NET `F2` biçimi: tam yarımda sıfırdan uzağa (1.125 → 1.13), ayrıca imza metninin biçimi.
#[test]
fn signature_text_matches_the_dotnet_format() {
    let m = mon("m", WindowRect::new(0, 0, 1, 1), 1.125, true);
    assert_eq!(
        "m;WindowRect { Left = 0, Top = 0, Right = 1920, Bottom = 1080 };1.13;True",
        MonitorLayoutSignature::compute(&[m])
    );
}

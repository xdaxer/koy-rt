use super::dock_position_resolver_tests::resolve_default;
use crate::display::DisplayMonitor;
use crate::dock::{DockCandidateWindow, DockForegroundFilter, DockProximityMode, DockStyleMode};
use crate::shell::{WindowInfo, WindowRect};

const PRIMARY: isize = 101;
const SECONDARY: isize = 202;

// Ana ekranda (0–1920) büyütülmüş pencere: Windows kenarlardan 8 px taşırır.
const MAXIMIZED_ON_PRIMARY: WindowRect = WindowRect::new(-8, -8, 1928, 1048);

#[test]
fn yan_ekrandaki_pencere_bu_dock_icin_bos_sayilir() {
    let bounds = DockForegroundFilter::bounds_for_dock(MAXIMIZED_ON_PRIMARY, PRIMARY, SECONDARY);
    assert!(bounds.is_empty());
    assert!(!DockForegroundFilter::applies_to_dock(PRIMARY, SECONDARY));
}

#[test]
fn kendi_ekranindaki_pencere_oldugu_gibi_verilir() {
    assert_eq!(MAXIMIZED_ON_PRIMARY, DockForegroundFilter::bounds_for_dock(MAXIMIZED_ON_PRIMARY, PRIMARY, PRIMARY));
    assert!(DockForegroundFilter::applies_to_dock(PRIMARY, PRIMARY));
}

#[test]
fn ekrani_bilinmeyen_pencere_her_dock_a_verilir() {
    assert_eq!(MAXIMIZED_ON_PRIMARY, DockForegroundFilter::bounds_for_dock(MAXIMIZED_ON_PRIMARY, 0, SECONDARY));
    assert!(DockForegroundFilter::applies_to_dock(0, SECONDARY));
}

#[test]
fn tasma_yan_ekrana_bos_sinir_verilince_dock_cakisma_gormez() {
    // Yan ekran 1920'den başlıyor: filtre olmadan 1928'lik sağ kenar bu ekranın alt şeridiyle kesişirdi.
    let secondary = DisplayMonitor::new(
        "MONITOR_2",
        "Yan ekran",
        WindowRect::new(1920, 0, 3456, 864),
        WindowRect::new(1920, 0, 3456, 864),
        false,
    )
    .with_handle(SECONDARY);
    let bounds = DockForegroundFilter::bounds_for_dock(MAXIMIZED_ON_PRIMARY, PRIMARY, SECONDARY);

    let state = resolve_default(&secondary, DockStyleMode::FloatingCapsule, DockProximityMode::Dodge, 400.0, bounds);

    assert!(!state.is_hidden);
    assert!(!state.is_snapped);
}

#[test]
fn resolve_monitor_window_ayni_monitorde_on_plan_penceresini_dondurur() {
    let fg = WindowInfo { handle: 999, bounds: MAXIMIZED_ON_PRIMARY };
    let (bounds, is_maximized, handle) = DockForegroundFilter::resolve_monitor_window(PRIMARY, fg, PRIMARY, true, None);

    assert_eq!(MAXIMIZED_ON_PRIMARY, bounds);
    assert!(is_maximized);
    assert_eq!(999, handle);
}

#[test]
fn resolve_monitor_window_farkli_monitorde_kendi_monitorundeki_en_ust_adayi_dondurur() {
    let fg = WindowInfo { handle: 999, bounds: MAXIMIZED_ON_PRIMARY };
    let candidate1 = DockCandidateWindow::with_state(10, WindowRect::new(0, 0, 800, 600), PRIMARY, false, false);
    let candidate2 = DockCandidateWindow::with_state(20, WindowRect::new(1920, 500, 2920, 1000), SECONDARY, true, false);
    let candidate3 = DockCandidateWindow::with_state(30, WindowRect::new(1920, 200, 2500, 700), SECONDARY, false, false);

    let (bounds, is_maximized, handle) =
        DockForegroundFilter::resolve_monitor_window(SECONDARY, fg, PRIMARY, true, Some(&[candidate1, candidate2, candidate3]));

    assert_eq!(candidate2.bounds, bounds);
    assert!(is_maximized);
    assert_eq!(20, handle);
}

#[test]
fn resolve_monitor_window_kucultulmus_adaylar_atlanir() {
    let fg = WindowInfo { handle: 999, bounds: MAXIMIZED_ON_PRIMARY };
    let minimized = DockCandidateWindow::with_state(20, WindowRect::new(1920, 0, 2500, 500), SECONDARY, false, true);
    let visible = DockCandidateWindow::with_state(30, WindowRect::new(1920, 500, 2920, 1000), SECONDARY, false, false);

    let (bounds, is_maximized, handle) =
        DockForegroundFilter::resolve_monitor_window(SECONDARY, fg, PRIMARY, true, Some(&[minimized, visible]));

    assert_eq!(visible.bounds, bounds);
    assert!(!is_maximized);
    assert_eq!(30, handle);
}

#[test]
fn resolve_monitor_window_kendi_monitorunde_aday_yoksa_bos_doner() {
    let fg = WindowInfo { handle: 999, bounds: MAXIMIZED_ON_PRIMARY };
    let candidate_on_primary = DockCandidateWindow::with_state(10, WindowRect::new(0, 0, 800, 600), PRIMARY, false, false);

    let (bounds, is_maximized, handle) =
        DockForegroundFilter::resolve_monitor_window(SECONDARY, fg, PRIMARY, true, Some(&[candidate_on_primary]));

    assert_eq!(WindowRect::EMPTY, bounds);
    assert!(!is_maximized);
    assert_eq!(0, handle);
}

#[test]
fn resolve_monitor_window_sinirlari_bos_adaylar_atlanir() {
    let fg = WindowInfo { handle: 999, bounds: MAXIMIZED_ON_PRIMARY };
    let empty_window = DockCandidateWindow::with_state(20, WindowRect::EMPTY, SECONDARY, false, false);
    let zero_size_window = DockCandidateWindow::with_state(25, WindowRect::new(100, 100, 100, 100), SECONDARY, false, false);
    let valid_window = DockCandidateWindow::with_state(30, WindowRect::new(1920, 500, 2920, 1000), SECONDARY, false, false);

    let (bounds, is_maximized, handle) = DockForegroundFilter::resolve_monitor_window(
        SECONDARY,
        fg,
        PRIMARY,
        true,
        Some(&[empty_window, zero_size_window, valid_window]),
    );

    assert_eq!(valid_window.bounds, bounds);
    assert!(!is_maximized);
    assert_eq!(30, handle);
}

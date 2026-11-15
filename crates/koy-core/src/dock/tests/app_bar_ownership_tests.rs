use crate::dock::AppBarOwnership;
use crate::shell::WindowRect;

#[test]
fn ownership_follows_the_real_hidden_state() {
    // (setting, hidden, manual, expected)
    let cases = [
        (true, true, false, true),   // gizlendi
        (true, false, false, false), // ayar açık ama henüz gizlenmedi (kayıt anı): Gezgin hâlâ sayılır
        (true, true, true, false),   // elle geri getirildi
        (false, true, false, false), // ayar kapandı
        (false, false, false, false),
    ];
    for (setting, hidden, manual, expected) in cases {
        assert_eq!(expected, AppBarOwnership::owns_bottom_edge(setting, hidden, manual), "({setting}, {hidden}, {manual})");
    }
}

#[test]
fn manual_restore_then_rehide_gives_the_edge_back() {
    let owns = AppBarOwnership::owns_bottom_edge(true, true, false);
    let after_manual_restore = AppBarOwnership::owns_bottom_edge(true, true, true);
    let after_rehide = AppBarOwnership::owns_bottom_edge(true, true, false);

    assert!(owns);
    assert!(!after_manual_restore);
    assert!(after_rehide);
}

const MONITOR: WindowRect = WindowRect::new(0, 0, 1920, 1080);

#[test]
fn owned_edge_requires_the_exact_reservation_in_both_directions() {
    // (workBottom, heightPx, expected)
    let cases = [
        (1080, 46, true), // hiçbir şey ayrılmamış
        (1034, 46, false), // tam pay
        (986, 46, true),  // FAZLA ayrılmış: kayıt anında Gezgin'in 48 px'i de sayılmış (94 px) - canlıda kalan hata
        (1032, 48, false),
        (1080, 48, true),
        (986, 48, true),
    ];
    for (work_bottom, height_px, expected) in cases {
        assert_eq!(
            expected,
            AppBarOwnership::needs_reassert(MONITOR, WindowRect::new(0, 0, 1920, work_bottom), height_px, true, None),
            "({work_bottom}, {height_px})"
        );
    }
}

#[test]
fn shared_edge_only_fixes_a_missing_reservation() {
    let cases = [
        (1080, 46, true),
        (1034, 46, false),
        (1010, 46, false), // başka çubuklar da yığılmış: fazlası normal
    ];
    for (work_bottom, height_px, expected) in cases {
        assert_eq!(
            expected,
            AppBarOwnership::needs_reassert(MONITOR, WindowRect::new(0, 0, 1920, work_bottom), height_px, false, None),
            "({work_bottom}, {height_px})"
        );
    }
}

#[test]
fn an_unresolved_mismatch_is_not_retried_for_the_same_work_area() {
    let work = WindowRect::new(0, 0, 1920, 986);

    assert!(AppBarOwnership::needs_reassert(MONITOR, work, 46, true, None));
    assert!(!AppBarOwnership::needs_reassert(MONITOR, work, 46, true, Some(986)));
    assert!(AppBarOwnership::needs_reassert(MONITOR, WindowRect::new(0, 0, 1920, 1080), 46, true, Some(986)));
}

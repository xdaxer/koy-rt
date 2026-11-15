use crate::dock::DockRightSection;

#[test]
fn nothing_visible_hides_separator_and_tray() {
    let s = DockRightSection::resolve(false, false, false, true);
    assert!(!s.show_first_separator);
    assert!(!s.show_full_tray);
    assert!(!s.show_small_tray);
}

#[test]
fn small_tray_does_not_need_the_first_separator() {
    let s = DockRightSection::resolve(false, false, true, true);
    assert!(!s.show_first_separator);
    assert!(!s.show_full_tray);
    assert!(s.show_small_tray);
}

#[test]
fn full_tray_alone_shows_the_separator() {
    let s = DockRightSection::resolve(false, false, true, false);
    assert!(s.show_first_separator);
    assert!(s.show_full_tray);
    assert!(!s.show_small_tray);
}

#[test]
fn folders_or_recents_show_the_separator() {
    for (folders, recents) in [(true, false), (false, true)] {
        assert!(DockRightSection::resolve(folders, recents, false, true).show_first_separator, "({folders}, {recents})");
    }
}

#[test]
fn missing_tray_service_hides_both_tray_buttons() {
    let s = DockRightSection::resolve(true, true, false, false);
    assert!(!s.show_full_tray);
    assert!(!s.show_small_tray);
}

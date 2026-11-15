use crate::display::*;

#[test]
fn pick_chooses_source_for_target() {
    use BrightnessSourceKind::{Ddc, None as NoSource, Wmi};
    use BrightnessTarget::{External, Internal, Unknown};
    let rows = [
        (Internal, true, true, Wmi),
        (Internal, true, false, Wmi),
        (Internal, false, true, Ddc),
        (Internal, false, false, NoSource),
        // Harici ekranda WMI kullanılmaz: dizüstü panelini değiştirirdi.
        (External, true, true, Ddc),
        (External, true, false, NoSource),
        (External, false, false, NoSource),
        (Unknown, true, true, Wmi),
        (Unknown, false, true, Ddc),
        (Unknown, false, false, NoSource),
    ];
    for (target, wmi, ddc, expected) in rows {
        assert_eq!(expected, BrightnessSourcePicker::pick(target, wmi, ddc), "{target:?} {wmi} {ddc}");
    }
}

#[test]
fn pick_is_reevaluated_when_layout_changes() {
    // İkinci ekran kapanınca: önce harici (DDC), sonra dahili panel (WMI).
    assert_eq!(BrightnessSourceKind::Ddc, BrightnessSourcePicker::pick(BrightnessTarget::External, true, true));
    assert_eq!(BrightnessSourceKind::Wmi, BrightnessSourcePicker::pick(BrightnessTarget::Internal, true, false));
}

#[test]
fn to_percent_scales_ddc_range() {
    let rows = [(0, 0, 100, 0), (50, 0, 100, 50), (35, 10, 60, 50), (200, 0, 100, 100), (5, 0, 0, 5)];
    for (value, min, max, expected) in rows {
        assert_eq!(expected, BrightnessSourcePicker::to_percent(value, min, max), "{value} {min} {max}");
    }
}

#[test]
fn from_percent_scales_to_ddc_range() {
    let rows = [(0, 10, 60, 10), (100, 10, 60, 60), (40, 10, 60, 30), (150, 0, 100, 100)];
    for (percent, min, max, expected) in rows {
        assert_eq!(expected, BrightnessSourcePicker::from_percent(percent, min, max), "{percent} {min} {max}");
    }
}

#[test]
fn queue_first_post_starts_worker_later_posts_coalesce() {
    let q = BrightnessWriteQueue::new();

    assert!(q.post(10));
    assert!(!q.post(20));
    assert!(!q.post(30));

    assert_eq!(Some(30), q.try_take());
    assert_eq!(None, q.try_take());
}

#[test]
fn queue_value_posted_while_applying_is_applied_next_without_second_worker() {
    let q = BrightnessWriteQueue::new();
    assert!(q.post(10));
    assert_eq!(Some(10), q.try_take());

    // Uygulama sürerken gelen değerler birikmez, yeni işçi de başlamaz.
    assert!(!q.post(40));
    assert!(!q.post(55));

    assert_eq!(Some(55), q.try_take());
    assert_eq!(None, q.try_take());
}

#[test]
fn queue_restarts_worker_after_it_drained() {
    let q = BrightnessWriteQueue::new();
    assert!(q.post(1));
    assert!(q.try_take().is_some());
    assert!(q.try_take().is_none());

    assert!(q.post(2));
}

#[test]
fn queue_clamps_values() {
    let q = BrightnessWriteQueue::new();
    q.post(500);
    assert_eq!(Some(100), q.try_take());
}

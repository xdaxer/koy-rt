use crate::tray::TrayHostPolicy;

#[test]
fn should_host_only_when_taskbar_is_actually_hidden() {
    let cases = [
        (true, true, 1, true),
        (true, true, 3, true),
        (true, true, 0, false),  // dock penceresi yok: görev çubuğu geri gelir, tepsiye dokunulmaz
        (false, true, 2, false), // görev çubuğu görünür: Windows'un tepsisine dokunulmaz
        (true, false, 2, false), // ayar kapalı
        (false, false, 0, false),
    ];
    for (hide, show, windows, expected) in cases {
        assert_eq!(expected, TrayHostPolicy::should_host(hide, show, windows), "{hide}/{show}/{windows}");
    }
}

#[test]
fn should_show_notification_area_icon_only_without_dock_windows() {
    let cases = [
        (0, true),  // dock yok / duraklatıldı: bildirim alanı simgesi gelir
        (-1, true),
        (1, false), // dock açık: Koy simgesi dock'un sağ ucunda, bildirim alanında ikinci simge yok
        (3, false),
    ];
    for (dock_windows, expected) in cases {
        assert_eq!(expected, TrayHostPolicy::should_show_notification_area_icon(dock_windows), "{dock_windows}");
    }
}

#[test]
fn is_own_icon_matches_only_koy_process() {
    assert!(TrayHostPolicy::is_own_icon(4242, 4242));
    assert!(!TrayHostPolicy::is_own_icon(1111, 4242));
}

#[test]
fn is_own_icon_unknown_owner_is_not_filtered() {
    // GetWindowThreadProcessId geçersiz pencerede 0 döner; sahte "kendi simgesi" sayılmaz (ölü pencere temizliği halleder).
    assert!(!TrayHostPolicy::is_own_icon(0, 0));
    assert!(!TrayHostPolicy::is_own_icon(0, 4242));
}

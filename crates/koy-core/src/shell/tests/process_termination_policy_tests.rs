use crate::shell::ProcessTerminationPolicy;

#[test]
fn sradan_uygulama_sonlandirilabilir() {
    for name in ["chrome", "notepad.exe", "Spotify"] {
        assert!(ProcessTerminationPolicy::can_terminate(1234, Some(name), 1), "{name}");
    }
}

#[test]
fn ortak_ve_kabuk_surecleri_sonlandirilmaz() {
    for name in ["ApplicationFrameHost", "applicationframehost.exe", "explorer", "dwm"] {
        assert!(!ProcessTerminationPolicy::can_terminate(1234, Some(name), 1), "{name}");
    }
}

#[test]
fn gecersiz_pid_sonlandirilmaz() {
    for pid in [0, -5] {
        assert!(!ProcessTerminationPolicy::can_terminate(pid, Some("chrome"), 1), "pid {pid}");
    }
}

#[test]
fn kendi_surecimiz_sonlandirilmaz() {
    assert!(!ProcessTerminationPolicy::can_terminate(99, Some("Koy"), 99));
}

#[test]
fn bilinmeyen_ad_sonlandirilmaz() {
    assert!(!ProcessTerminationPolicy::can_terminate(1234, None, 1));
    assert!(!ProcessTerminationPolicy::can_terminate(1234, Some(" "), 1));
}

use crate::shell::ShellFlyouts;

#[test]
fn kabuk_acilir_pencere_surecleri_taninir() {
    for name in ["StartMenuExperienceHost", "startmenuexperiencehost", "SearchHost", "SearchApp", "ShellExperienceHost"] {
        assert!(ShellFlyouts::is_flyout_process(Some(name)), "{name}");
    }
}

#[test]
fn siradan_surecler_kabuk_acilir_penceresi_sayilmaz() {
    for name in [None, Some(""), Some("explorer"), Some("chrome"), Some("StartMenuExperienceHost.exe")] {
        assert!(!ShellFlyouts::is_flyout_process(name), "{name:?}");
    }
}

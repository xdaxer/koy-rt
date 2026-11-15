//! `FullscreenRuleTests.cs` + `EdgeRevealAndFullscreenTests.cs` içindeki `FullscreenRuleStateTests`.
//! (`EdgeRevealTrackerTests` ve `DockEdgeRevealResolverTests` Dock modülüne ait, orada taşınacak.)

use crate::shell::*;

const MONITOR: WindowRect = WindowRect::new(0, 0, 1920, 1080);

mod fullscreen_rule_tests {
    use super::*;

    fn fs(window: WindowRect, has_caption: bool, is_maximized: bool) -> bool {
        FullscreenRule::is_fullscreen(window, MONITOR, has_caption, is_maximized, ShellNotificationState::default())
    }

    #[test]
    fn borderless_window_covering_monitor_is_fullscreen() {
        // Medya Oynatıcı tam ekran: başlık yok, büyütülmüş değil, monitörü tam kaplıyor.
        assert!(fs(MONITOR, false, false));
    }

    #[test]
    fn maximized_borderless_window_with_hidden_taskbar_is_not_fullscreen() {
        // Görev çubuğu gizliyken büyütülmüş Chrome/Electron penceresi kenar payıyla monitörü aşar.
        let maximized = WindowRect::new(-9, -9, 1929, 1089);
        assert!(!fs(maximized, false, true));
    }

    #[test]
    fn captioned_window_covering_monitor_is_not_fullscreen() {
        assert!(!fs(MONITOR, true, false));
    }

    #[test]
    fn f11_browser_covering_monitor_with_maximized_flag_is_fullscreen() {
        // F11 tarayıcı veya video oynatıcı: WS_MAXIMIZE bayrağını korusa dahi monitöre tam oturur.
        assert!(fs(MONITOR, false, true));
    }

    #[test]
    fn window_smaller_than_monitor_is_not_fullscreen() {
        let window = WindowRect::new(100, 100, 900, 700);
        assert!(!fs(window, false, false));
    }
}

mod fullscreen_rule_state_tests {
    use super::*;

    const MAXIMIZED_OVERFLOW: WindowRect = WindowRect::new(-8, -8, 1928, 1088);
    const SMALL: WindowRect = WindowRect::new(100, 100, 900, 700);

    #[test]
    fn ozel_kip_oyun_ve_sunum_modu_kesin_tam_ekran() {
        for state in [ShellNotificationState::D3dFullScreen, ShellNotificationState::Presentation] {
            // Geometri ne olursa olsun.
            assert!(FullscreenRule::is_fullscreen(SMALL, MONITOR, true, false, state), "{state:?}");
        }
    }

    #[test]
    fn windows_acikca_tam_ekran_yok_diyorsa_geometri_yetmez() {
        assert!(!FullscreenRule::is_fullscreen(MONITOR, MONITOR, false, false, ShellNotificationState::Normal));
    }

    #[test]
    fn mesgul_plus_basliksiz_plus_kaplama_tam_ekran() {
        assert!(FullscreenRule::is_fullscreen(MONITOR, MONITOR, false, false, ShellNotificationState::Busy));
    }

    #[test]
    fn sessiz_saatlerde_mesgul_donup_geometri_tutmuyorsa_tam_ekran_degil() {
        // Odaklanma Yardımı "meşgul" derken küçük pencere: tek başına sistem işareti yetmez.
        assert!(!FullscreenRule::is_fullscreen(SMALL, MONITOR, false, false, ShellNotificationState::Busy));
        assert!(!FullscreenRule::is_fullscreen(MONITOR, MONITOR, true, false, ShellNotificationState::Busy));
    }

    #[test]
    fn buyutulmus_normal_pencere_mesgul_olsa_bile_tam_ekran_degil() {
        assert!(!FullscreenRule::is_fullscreen(MAXIMIZED_OVERFLOW, MONITOR, false, true, ShellNotificationState::Busy));
        assert!(!FullscreenRule::is_fullscreen(MAXIMIZED_OVERFLOW, MONITOR, true, true, ShellNotificationState::Busy));
    }

    #[test]
    fn f11_tarayici_monitore_tam_oturur_buyutulmus_bayragiyla_tam_ekran() {
        assert!(FullscreenRule::is_fullscreen(MONITOR, MONITOR, false, true, ShellNotificationState::Busy));
    }

    #[test]
    fn buyutulmus_tam_oturan_pencereyi_windows_onaylamazsa_tam_ekran_degil() {
        assert!(!FullscreenRule::is_fullscreen(MONITOR, MONITOR, false, true, ShellNotificationState::Normal));
    }

    #[test]
    fn baska_ekrandaki_pencere_icin_durum_bilinmez_geometri_karar() {
        assert!(FullscreenRule::is_fullscreen(MONITOR, MONITOR, false, false, ShellNotificationState::Unknown));
        assert!(!FullscreenRule::is_fullscreen(MAXIMIZED_OVERFLOW, MONITOR, false, true, ShellNotificationState::Unknown));
    }

    #[test]
    fn from_quns_sayisal_degeri_sadelestirir() {
        let cases = [
            (2, ShellNotificationState::Busy),
            (7, ShellNotificationState::Busy),
            (3, ShellNotificationState::D3dFullScreen),
            (4, ShellNotificationState::Presentation),
            (5, ShellNotificationState::Normal),
            (1, ShellNotificationState::Unknown),
            (6, ShellNotificationState::Unknown),
            (99, ShellNotificationState::Unknown),
        ];
        for (quns, expected) in cases {
            assert_eq!(expected, ShellNotificationStates::from_quns(quns), "quns {quns}");
        }
    }
}

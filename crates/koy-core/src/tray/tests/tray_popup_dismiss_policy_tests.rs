use crate::time::TimeSpan;
use crate::tray::{TrayPopupDismissAction, TrayPopupDismissPolicy};

fn ms(ms: i64) -> TimeSpan {
    TimeSpan::from_millis(ms)
}

#[test]
fn pencere_disina_tiklaninca_kapanir() {
    let mut policy = TrayPopupDismissPolicy::new();

    let action = policy.tick(ms(0), true, false, false, false);

    assert_eq!(TrayPopupDismissAction::Close, action);
}

#[test]
fn pencere_icine_tiklaninca_kapanmaz() {
    let mut policy = TrayPopupDismissPolicy::new();

    let action = policy.tick(ms(0), true, true, false, false);

    assert_eq!(TrayPopupDismissAction::None, action);
}

#[test]
fn basmadan_imlec_disarda_gezmek_pencereyi_kapatmaz_odak_kaybi_sayilmaz() {
    let mut policy = TrayPopupDismissPolicy::new();

    for t in (0..2000).step_by(50) {
        assert_eq!(TrayPopupDismissAction::None, policy.tick(ms(t), false, false, false, false));
    }
}

#[test]
fn sag_tiktan_sonra_menu_acikken_pencere_acik_kalir() {
    let mut policy = TrayPopupDismissPolicy::new();
    policy.on_icon_right_clicked(ms(0));

    assert_eq!(TrayPopupDismissAction::None, policy.tick(ms(50), false, true, false, false));
    assert_eq!(TrayPopupDismissAction::None, policy.tick(ms(100), false, false, true, true));
    assert_eq!(TrayPopupDismissAction::None, policy.tick(ms(2000), false, false, true, true));
}

#[test]
fn menuden_secim_yapilinca_menu_kapanir_pencere_de_kapanir() {
    let mut policy = TrayPopupDismissPolicy::new();
    policy.on_icon_right_clicked(ms(0));
    policy.tick(ms(100), false, false, true, true);

    // Menü öğesine tıklandı (imleç menüde), sonraki adımda menü yok.
    assert_eq!(TrayPopupDismissAction::None, policy.tick(ms(200), true, false, true, true));
    assert_eq!(TrayPopupDismissAction::Close, policy.tick(ms(250), false, false, false, false));
}

#[test]
fn menu_acikken_menu_ve_pencere_disina_tiklaninca_kapanir() {
    let mut policy = TrayPopupDismissPolicy::new();
    policy.on_icon_right_clicked(ms(0));
    policy.tick(ms(100), false, false, true, true);

    let action = policy.tick(ms(300), true, false, false, true);

    assert_eq!(TrayPopupDismissAction::Close, action);
}

#[test]
fn menu_pencereye_tiklanarak_kapandiysa_pencere_acik_kalir() {
    let mut policy = TrayPopupDismissPolicy::new();
    policy.on_icon_right_clicked(ms(0));
    policy.tick(ms(100), false, false, true, true);

    // Kullanıcı menü açıkken başka bir tepsi simgesine tıkladı: menü kapanır, pencere kalır.
    policy.tick(ms(300), true, true, false, true);
    let action = policy.tick(ms(350), false, true, false, false);

    assert_eq!(TrayPopupDismissAction::None, action);
}

#[test]
fn menu_hic_gelmezse_bekleme_biter_ve_pencere_acik_kalir() {
    let mut policy = TrayPopupDismissPolicy::new();
    policy.on_icon_right_clicked(ms(0));

    assert_eq!(TrayPopupDismissAction::None, policy.tick(ms(900), false, true, false, false));

    // Sonradan başka biri menü açıp kapatsa bile bu sağ tıkla ilişkilendirilmez.
    policy.tick(ms(1000), false, true, false, true);
    assert_eq!(TrayPopupDismissAction::None, policy.tick(ms(1100), false, true, false, false));
}

#[test]
fn klavyeyle_menu_kapaninca_pencere_kapanir() {
    let mut policy = TrayPopupDismissPolicy::new();
    policy.on_icon_right_clicked(ms(0));
    policy.tick(ms(100), false, false, true, true);

    assert_eq!(TrayPopupDismissAction::Close, policy.tick(ms(400), false, false, false, false));
}

#[test]
fn reset_beklemeyi_sifirlar() {
    let mut policy = TrayPopupDismissPolicy::new();
    policy.on_icon_right_clicked(ms(0));
    policy.tick(ms(100), false, false, true, true);
    policy.reset();

    assert_eq!(TrayPopupDismissAction::None, policy.tick(ms(200), false, false, false, false));
}

use super::support::{s, settings_file_load};
use crate::notch::{
    HomeCard, HomeCardLayout, NotchOpenAction, NotchOpenBehavior, NotchOpenDecision, NotchOpenPolicy, NotchPanel, NotchPanels, NotchPointerEvent,
    NotchTabBarStyle, NotchVisualizer, NotchVisualizerColorSource, NotchVisualizerStyle,
};
use crate::settings::{AppSettings, MusicSettings, NotchSettings, SettingsStore, ValueList};

#[test]
fn open_policy_maps_event_to_action() {
    use NotchOpenAction::{Close, None as NoAction, Open};
    use NotchOpenBehavior::{Click, DragOver, Hover};
    use NotchPointerEvent::{Click as ClickEv, DragEnter, DragLeave, Enter, Leave};
    let rows = [
        (Hover, Enter, false, Open, 120),
        (Hover, Enter, true, NoAction, 0),
        (Hover, Leave, true, Close, 300),
        (Hover, Leave, false, NoAction, 0),
        (Hover, ClickEv, false, NoAction, 0),
        (Click, ClickEv, false, Open, 0),
        (Click, ClickEv, true, Close, 0),
        (Click, Enter, false, NoAction, 0),
        // Bugünkü davranış: açık çentik her kipte imleç çıkınca kapanır.
        (Click, Leave, true, Close, 300),
        (DragOver, Leave, true, Close, 300),
        (DragOver, DragEnter, false, Open, 120),
        (DragOver, DragEnter, true, NoAction, 0),
        (DragOver, DragLeave, true, Close, 300),
        (DragOver, Enter, false, NoAction, 0),
        (DragOver, ClickEv, false, NoAction, 0),
        // Bugünkü davranış: dosya sürüklenince çentik her kipte Raf için hemen açılır; gecikme yalnız DragOver kipinin.
        (Hover, DragEnter, false, Open, 0),
        (Click, DragEnter, false, Open, 0),
        (Hover, DragLeave, true, Close, 300),
    ];
    for (behavior, ev, is_open, action, expected_delay) in rows {
        let decision = NotchOpenPolicy::decide(behavior, ev, is_open, 120, 300);

        assert_eq!(action, decision.action, "{behavior:?} {ev:?} {is_open}");
        assert_eq!(if action == NoAction { 0 } else { expected_delay }, decision.delay_ms, "{behavior:?} {ev:?} {is_open}");
    }
}

#[test]
fn click_ignores_open_delay() {
    let decision = NotchOpenPolicy::decide(NotchOpenBehavior::Click, NotchPointerEvent::Click, false, 900, 0);

    assert_eq!(NotchOpenDecision::new(NotchOpenAction::Open, 0), decision);
}

#[test]
fn delays_are_clamped_to_zero_through_max() {
    let rows = [(-5, 0), (0, 0), (450, 450), (2000, 2000), (99999, 2000)];
    for (requested, expected) in rows {
        let decision = NotchOpenPolicy::decide(NotchOpenBehavior::Hover, NotchPointerEvent::Enter, false, requested, 0);

        assert_eq!(expected, decision.delay_ms, "{requested}");
    }
}

#[test]
fn null_lists_in_the_file_are_restored() {
    let loaded = settings_file_load(r#"{ "Version": 4, "Notch": { "PanelOrder": null, "HomeCards": null } }"#);

    assert_eq!(0, loaded.notch.panel_order.len());
    assert_eq!(HomeCardLayout::default().to_vec(), loaded.notch.home_cards.iter().copied().collect::<Vec<HomeCard>>());
}

#[test]
fn default_home_cards_are_a_copy_not_the_shared_default() {
    let mut first = NotchSettings::default();
    first.home_cards = ValueList::from(Vec::new());

    assert_eq!(HomeCardLayout::default().to_vec(), NotchSettings::default().home_cards.iter().copied().collect::<Vec<HomeCard>>());
}

#[test]
fn visualizer_bar_count_is_clamped() {
    for (requested, expected) in [(0, 3), (1, 3), (5, 5), (16, 16), (40, 16)] {
        assert_eq!(expected, NotchVisualizer::clamp_bar_count(requested), "{requested}");
    }
}

#[test]
fn panel_order_puts_valid_keys_first_and_appends_missing_panels() {
    let order = NotchPanels::normalize_order(Some(&s(&["system", "bogus", "home", "system", "shelf"])));

    assert_eq!(
        vec![
            NotchPanel::System,
            NotchPanel::Home,
            NotchPanel::Shelf,
            NotchPanel::Focus,
            NotchPanel::Calendar,
            NotchPanel::Clipboard,
            NotchPanel::ControlCenter
        ],
        order
    );
}

#[test]
fn panel_order_without_keys_is_default_order() {
    assert_eq!(NotchPanels::order().to_vec(), NotchPanels::normalize_order(None));
    assert_eq!(NotchPanels::order().to_vec(), NotchPanels::normalize_order(Some(&[])));
}

#[test]
fn home_cards_keep_order_and_drop_duplicates() {
    // C#'taki `(HomeCard)99` Rust'ta kurulamaz (Enum.IsDefined elemesi gereksiz); tekrar elemesi sınanır.
    let cards = HomeCardLayout::normalize(Some(&[HomeCard::Weather, HomeCard::Music, HomeCard::Weather]));

    assert_eq!(vec![HomeCard::Weather, HomeCard::Music], cards.iter().copied().collect::<Vec<HomeCard>>());
}

#[test]
fn home_cards_null_gives_default_and_empty_stays_empty() {
    assert_eq!(HomeCardLayout::default().to_vec(), HomeCardLayout::normalize(None).iter().copied().collect::<Vec<HomeCard>>());
    assert_eq!(0, HomeCardLayout::normalize(Some(&[])).len());
}

#[test]
fn new_notch_options_have_safe_defaults() {
    let notch = AppSettings::default().notch;
    let music = AppSettings::default().music;

    // Bugünkü sekme çubuğu simge ve adı birlikte gösterir; varsayılan onu korur.
    assert_eq!(NotchTabBarStyle::IconAndLabel, notch.tab_bar_style);
    assert_eq!(0, notch.open_delay_ms);
    assert_eq!(NotchOpenPolicy::DEFAULT_CLOSE_DELAY_MS, notch.close_delay_ms);
    assert_eq!(0, notch.panel_order.len());
    assert_eq!(HomeCardLayout::default().to_vec(), notch.home_cards.iter().copied().collect::<Vec<HomeCard>>());
    assert_eq!(NotchVisualizerStyle::Bars, music.visualizer_style);
    assert_eq!(NotchVisualizerColorSource::Accent, music.visualizer_color_source);
    assert_eq!(5, music.visualizer_bar_count);
}

#[test]
fn new_notch_options_survive_save_and_load() {
    let path = std::env::temp_dir().join(format!("koy-notch-options-{}.json", std::process::id()));
    let original = AppSettings {
        notch: NotchSettings {
            open_behavior: NotchOpenBehavior::DragOver,
            open_delay_ms: 250,
            close_delay_ms: 600,
            tab_bar_style: NotchTabBarStyle::Hidden,
            panel_order: ValueList::from(s(&["system", "home"])),
            home_cards: ValueList::from(vec![HomeCard::Weather, HomeCard::Timers]),
            ..NotchSettings::default()
        },
        music: MusicSettings {
            visualizer_style: NotchVisualizerStyle::Dots,
            visualizer_color_source: NotchVisualizerColorSource::Artwork,
            visualizer_bar_count: 9,
            ..MusicSettings::default()
        },
        ..AppSettings::default()
    };

    let store = SettingsStore::new(path.clone());
    store.save(&original);
    let loaded = store.load();

    assert_eq!(original.notch, loaded.notch);
    assert_eq!(original.music, loaded.music);
    let _ = std::fs::remove_file(&path);
}

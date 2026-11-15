use crate::radio::*;

#[test]
fn resolve_controllable_radio_toggles_and_shows_state() {
    for (state, is_on, status) in [(RadioState::On, true, RadioTileStatus::On), (RadioState::Off, false, RadioTileStatus::Off)] {
        let tile = RadioTileResolver::resolve(RadioAccess::Allowed, state, false);

        assert_eq!(status, tile.status, "{state:?}");
        assert_eq!(is_on, tile.is_on, "{state:?}");
        assert_eq!(RadioTileAction::Toggle, tile.action, "{state:?}");
        assert!(!tile.is_unavailable(), "{state:?}");
    }
}

#[test]
fn resolve_not_scanned_yet_is_loading_and_ignores_clicks() {
    for access in [RadioAccess::Unknown, RadioAccess::Allowed] {
        let tile = RadioTileResolver::resolve(access, RadioState::Unknown, false);

        assert_eq!(RadioTileStatus::Loading, tile.status, "{access:?}");
        assert_eq!(RadioTileAction::None, tile.action, "{access:?}");
        assert!(!tile.is_on, "{access:?}");
        assert!(!tile.is_unavailable(), "{access:?}");
    }
}

#[test]
fn resolve_access_denied_opens_privacy_settings_instead_of_fake_toggle() {
    for (access, state) in [
        (RadioAccess::DeniedByUser, RadioState::NotPresent),
        (RadioAccess::DeniedBySystem, RadioState::NotPresent),
        (RadioAccess::DeniedByUser, RadioState::On),
    ] {
        let tile = RadioTileResolver::resolve(access, state, false);

        assert_eq!(RadioTileStatus::NoAccess, tile.status, "{access:?} {state:?}");
        assert_eq!(RadioTileAction::OpenPrivacySettings, tile.action, "{access:?} {state:?}");
        assert!(!tile.is_on, "{access:?} {state:?}");
        assert!(tile.is_unavailable(), "{access:?} {state:?}");
    }
}

#[test]
fn resolve_no_adapter_says_not_found_and_opens_settings() {
    let tile = RadioTileResolver::resolve(RadioAccess::Allowed, RadioState::NotPresent, false);

    assert_eq!(RadioTileStatus::NotFound, tile.status);
    assert_eq!(RadioTileAction::OpenSettings, tile.action);
    assert!(tile.is_unavailable());
}

#[test]
fn resolve_scan_failed_with_unknown_access_still_shows_not_found() {
    // Radyo yığını hata verirse izin bilinmez ama tür "yok" sayılır; döşeme sonsuza dek yüklenmez.
    let tile = RadioTileResolver::resolve(RadioAccess::Unknown, RadioState::NotPresent, false);

    assert_eq!(RadioTileStatus::NotFound, tile.status);
    assert_eq!(RadioTileAction::OpenSettings, tile.action);
}

#[test]
fn resolve_hardware_switch_off_says_disabled_and_opens_settings() {
    let tile = RadioTileResolver::resolve(RadioAccess::Allowed, RadioState::Disabled, false);

    assert_eq!(RadioTileStatus::HardwareOff, tile.status);
    assert_eq!(RadioTileAction::OpenSettings, tile.action);
    assert!(!tile.is_on);
    assert!(tile.is_unavailable());
}

#[test]
fn resolve_last_toggle_failed_shows_failure_but_keeps_real_state_and_retry() {
    for (state, is_on) in [(RadioState::On, true), (RadioState::Off, false)] {
        let tile = RadioTileResolver::resolve(RadioAccess::Allowed, state, true);

        assert_eq!(RadioTileStatus::Failed, tile.status, "{state:?}");
        assert_eq!(is_on, tile.is_on, "{state:?}");
        assert_eq!(RadioTileAction::Toggle, tile.action, "{state:?}");
    }
}

#[test]
fn resolve_failure_flag_ignored_when_radio_unavailable() {
    let tile = RadioTileResolver::resolve(RadioAccess::Allowed, RadioState::NotPresent, true);

    assert_eq!(RadioTileStatus::NotFound, tile.status);
}

#[test]
fn is_present_shows_notch_icon_only_for_known_existing_radio() {
    let cases = [
        (RadioAccess::Allowed, RadioState::On, true),
        (RadioAccess::Allowed, RadioState::Off, true),
        (RadioAccess::Allowed, RadioState::Disabled, true),
        (RadioAccess::Allowed, RadioState::NotPresent, false),
        (RadioAccess::DeniedByUser, RadioState::On, false),
        (RadioAccess::Unknown, RadioState::Unknown, false),
    ];
    // Çentikte simge yalnız radyo varsa çıkar; Bluetooth'suz kasada boş simge gösterilmez.
    for (access, state, expected) in cases {
        assert_eq!(expected, RadioTileResolver::resolve(access, state, false).is_present(), "{access:?} {state:?}");
    }
}

#[test]
fn is_present_stays_true_when_toggle_failed() {
    assert!(RadioTileResolver::resolve(RadioAccess::Allowed, RadioState::On, true).is_present());
}

// ModelContractTests.RadioStateChangedEventArgs_InitializesProperly
#[test]
fn radio_state_changed_event_args_initializes_properly() {
    let args = RadioStateChangedEventArgs::new(RadioKind::WiFi, RadioState::On);
    assert_eq!(RadioKind::WiFi, args.kind);
    assert_eq!(RadioState::On, args.state);
}

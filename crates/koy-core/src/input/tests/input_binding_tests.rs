//! Karşılığı: `InputBindingTests.cs`.

use std::collections::{HashMap, HashSet};

use crate::input::*;
use crate::shortcuts::{Hotkey, HotkeyModifiers, ShortcutAction, ShortcutBindings};

fn parse(action: &str, kind: &str, trigger: &str) -> InputBindingParseResult {
    parse_full(action, kind, trigger, None, None, None)
}

fn parse_full(
    action: &str,
    kind: &str,
    trigger: &str,
    dwell: Option<i32>,
    push_back: Option<i32>,
    custom_id: Option<&str>,
) -> InputBindingParseResult {
    InputBindingParser::parse(Some(&InputBindingData {
        action: Some(action.to_string()),
        kind: Some(kind.to_string()),
        trigger: Some(trigger.to_string()),
        dwell_ms: dwell,
        push_back_px: push_back,
        custom_action_id: custom_id.map(str::to_string),
    }))
}

fn shortcut(modifiers: HotkeyModifiers, vk: i32) -> InputTrigger {
    InputTrigger::Shortcut(ShortcutTrigger::new(Hotkey::new(modifiers, vk)))
}

fn validate(bindings: &[InputBinding]) -> InputBindingReport {
    InputBindingValidator::validate(bindings, &HashSet::new(), None)
}

#[test]
fn parse_kisayol_bagi_cozulur() {
    let result = parse("OpenSearch", "Shortcut", "Ctrl+Alt+N");

    assert!(result.is_valid());
    let binding = result.binding.unwrap();
    assert_eq!(InputAction::OpenSearch, binding.action);
    assert_eq!(shortcut(HotkeyModifiers::Control | HotkeyModifiers::Alt, 0x4E), binding.trigger);
}

#[test]
fn parse_bilinmeyen_eylem_dusurulur() {
    let result = parse("ShowMoonbase", "Shortcut", "Ctrl+Alt+N");

    assert!(!result.is_valid());
    assert_eq!(Some(InputBindingDropReason::UnknownAction), result.drop_reason);
}

#[test]
fn parse_bilinmeyen_tetikleyici_turu_dusurulur() {
    let result = parse("ShowDesktop", "Gesture", "Wheel");

    assert!(!result.is_valid());
    assert_eq!(Some(InputBindingDropReason::UnknownKind), result.drop_reason);
}

#[test]
fn parse_sayisal_metin_ad_kabul_edilmez() {
    assert_eq!(Some(InputBindingDropReason::UnknownAction), parse("3", "Shortcut", "Ctrl+Alt+N").drop_reason);
}

#[test]
fn parse_ad_buyuk_kucuk_harf_duyarsiz() {
    assert!(parse("showdesktop", "SHORTCUT", "Win+D").is_valid());
}

#[test]
fn parse_bozuk_kisayol_dusurulur() {
    let result = parse("OpenSearch", "Shortcut", "Ctrl+Alt+Bilinmeyen");

    assert_eq!(Some(InputBindingDropReason::InvalidTrigger), result.drop_reason);
}

#[test]
fn parse_kose_varsayilanlari_kullanir() {
    let result = parse("OpenNotch", "HotCorner", "TopLeft");

    let InputTrigger::HotCorner(trigger) = result.binding.unwrap().trigger else {
        panic!("HotCornerTrigger bekleniyordu");
    };
    assert_eq!(ScreenCorner::TopLeft, trigger.corner);
    assert_eq!(InputBindingParser::DEFAULT_DWELL_MS, trigger.dwell_ms);
    assert_eq!(InputBindingParser::DEFAULT_PUSH_BACK_PX, trigger.push_back_px);
}

#[test]
fn parse_kose_aralik_disi_sureyi_dusurur() {
    assert_eq!(
        Some(InputBindingDropReason::InvalidParameter),
        parse_full("OpenNotch", "HotCorner", "TopLeft", Some(-1), None, None).drop_reason
    );
    assert_eq!(
        Some(InputBindingDropReason::InvalidParameter),
        parse_full("OpenNotch", "HotCorner", "TopLeft", Some(InputBindingParser::MAX_DWELL_MS + 1), None, None).drop_reason
    );
    assert_eq!(
        Some(InputBindingDropReason::InvalidParameter),
        parse_full("OpenNotch", "HotCorner", "TopLeft", None, Some(InputBindingParser::MAX_PUSH_BACK_PX + 1), None).drop_reason
    );
}

#[test]
fn parse_fare_bagi_surface_ve_jest_iki_parcalidir() {
    let result = parse("ToggleWifi", "MouseButton", "Dock.MiddleClick");

    assert_eq!(
        InputTrigger::Mouse(MouseTrigger::new(InputSurface::Dock, MouseGesture::MiddleClick)),
        result.binding.unwrap().trigger
    );
}

#[test]
fn parse_fare_bagi_tek_parca_dusurulur() {
    assert_eq!(Some(InputBindingDropReason::InvalidTrigger), parse("ToggleWifi", "MouseButton", "Wheel").drop_reason);
    assert_eq!(Some(InputBindingDropReason::InvalidTrigger), parse("ToggleWifi", "MouseButton", "Notch.Scroll").drop_reason);
}

#[test]
fn parse_ozel_eylem_kimligi_zorunlu() {
    assert_eq!(Some(InputBindingDropReason::InvalidParameter), parse("RunCustomAction", "Shortcut", "Ctrl+Alt+N").drop_reason);
    assert_eq!(
        Some("build".to_string()),
        parse_full("RunCustomAction", "Shortcut", "Ctrl+Alt+N", None, None, Some(" build ")).binding.unwrap().custom_action_id
    );
}

#[test]
fn parse_ozel_kimlik_baska_eylemde_tutulmaz() {
    let result = parse_full("OpenSearch", "Shortcut", "Ctrl+Alt+N", None, None, Some("build"));

    assert!(result.binding.unwrap().custom_action_id.is_none());
}

#[test]
fn validate_ayni_eylem_ayni_tetikleyici_duplicate() {
    let kisayol = shortcut(HotkeyModifiers::Control | HotkeyModifiers::Alt, 0x4E);
    let bindings = [InputBinding::new(InputAction::OpenSearch, kisayol), InputBinding::new(InputAction::OpenSearch, kisayol)];

    let report = validate(&bindings);

    assert_eq!(1, report.accepted.len());
    assert_eq!(1, report.issues.len());
    let issue = &report.issues[0];
    assert_eq!(InputIssueKind::Duplicate, issue.kind);
    assert_eq!(1, issue.index);
    assert_eq!(Some(0), issue.conflicts_with_index);
}

#[test]
fn validate_farkli_eylem_ayni_tetikleyici_conflict() {
    let kose = HotCornerTrigger::new(ScreenCorner::TopRight, 300, 4);
    let bindings = [
        InputBinding::new(InputAction::ShowDesktop, InputTrigger::HotCorner(kose)),
        InputBinding::new(InputAction::OpenSearch, InputTrigger::HotCorner(HotCornerTrigger { dwell_ms: 800, ..kose })),
    ];

    let report = validate(&bindings);

    assert_eq!(1, report.accepted.len());
    assert_eq!(InputAction::ShowDesktop, report.accepted[0].action);
    assert_eq!(1, report.issues.len());
    assert_eq!(InputIssueKind::Conflict, report.issues[0].kind);
}

#[test]
fn validate_bir_eylem_birden_fazla_kisayol_alabilir() {
    let bindings = [
        InputBinding::new(InputAction::ShowDesktop, shortcut(HotkeyModifiers::Win, 0x44)),
        InputBinding::new(InputAction::ShowDesktop, InputTrigger::HotCorner(HotCornerTrigger::new(ScreenCorner::BottomLeft, 300, 4))),
        InputBinding::new(
            InputAction::ShowDesktop,
            InputTrigger::Mouse(MouseTrigger::new(InputSurface::Notch, MouseGesture::DoubleClick)),
        ),
    ];

    let report = validate(&bindings);

    assert_eq!(3, report.accepted.len());
    assert!(report.issues.is_empty());
}

#[test]
fn validate_sistem_kisayolu_reddedilir() {
    let bindings = [InputBinding::new(InputAction::OpenSearch, shortcut(HotkeyModifiers::Win, 0x4C))];

    let report = validate(&bindings);

    assert!(report.accepted.is_empty());
    assert_eq!(1, report.issues.len());
    assert_eq!(InputIssueKind::ReservedBySystem, report.issues[0].kind);
}

#[test]
fn validate_bilinmeyen_ozel_eylem_reddedilir() {
    let bindings = [
        InputBinding::with_custom_action_id(
            InputAction::RunCustomAction,
            InputTrigger::Mouse(MouseTrigger::new(InputSurface::Dock, MouseGesture::RightClick)),
            Some("silinmis".to_string()),
        ),
        InputBinding::with_custom_action_id(
            InputAction::RunCustomAction,
            InputTrigger::Mouse(MouseTrigger::new(InputSurface::Dock, MouseGesture::Wheel)),
            Some("build".to_string()),
        ),
    ];

    let ids: HashSet<String> = ["build".to_string()].into_iter().collect();
    let report = InputBindingValidator::validate(&bindings, &ids, None);

    assert_eq!(1, report.accepted.len());
    assert_eq!(bindings[1], report.accepted[0]);
    assert_eq!(1, report.issues.len());
    assert_eq!(InputIssueKind::UnknownCustomAction, report.issues[0].kind);
}

#[test]
fn validate_bos_liste_bos_rapor() {
    let report = validate(&[]);

    assert!(report.accepted.is_empty());
    assert!(report.issues.is_empty());
}

#[test]
fn parse_null_veri_cokertmez_dusurulur() {
    assert!(!InputBindingParser::parse(None).is_valid());
}

#[test]
fn validate_yerlesik_kisayol_ile_cakisan_dusurulur() {
    // Ctrl+Alt+N varsayılan olarak ToggleNotch'a bağlı.
    let built_in: HashMap<ShortcutAction, String> =
        ShortcutBindings::all().iter().map(|&a| (a, ShortcutBindings::default_text(a).to_string())).collect();
    let ctrl_alt = HotkeyModifiers::Control | HotkeyModifiers::Alt;
    let bindings = [
        InputBinding::new(InputAction::OpenSearch, shortcut(ctrl_alt, 'N' as i32)),
        InputBinding::new(InputAction::OpenSearch, shortcut(ctrl_alt, 'J' as i32)),
    ];

    let report = InputBindingValidator::validate(&bindings, &HashSet::new(), Some(&built_in));

    assert_eq!(1, report.accepted.len());
    assert_eq!(bindings[1], report.accepted[0]);
    assert_eq!(1, report.issues.len());
    let issue = &report.issues[0];
    assert_eq!(InputIssueKind::ConflictsWithBuiltIn, issue.kind);
    assert_eq!(Some(ShortcutAction::ToggleNotch), issue.built_in);
}

#[test]
fn validate_yerlesik_kisayol_verilmezse_denetlenmez() {
    let bindings = [InputBinding::new(
        InputAction::OpenSearch,
        shortcut(HotkeyModifiers::Control | HotkeyModifiers::Alt, 'N' as i32),
    )];

    assert_eq!(1, validate(&bindings).accepted.len());
}

#[test]
fn validate_ozel_eylem_kimligi_buyuk_kucuk_harf_duyarsiz() {
    let bindings = [InputBinding::with_custom_action_id(
        InputAction::RunCustomAction,
        InputTrigger::Mouse(MouseTrigger::new(InputSurface::Dock, MouseGesture::Wheel)),
        Some("Build".to_string()),
    )];

    // Küme büyük-küçük harf duyarlı olsa bile ayrıştırıcıyla aynı kuralla eşleşmeli.
    let ids: HashSet<String> = ["build".to_string()].into_iter().collect();
    let report = InputBindingValidator::validate(&bindings, &ids, None);

    assert_eq!(1, report.accepted.len());
}

#[test]
fn validate_en_cok_max_bindings_kabul_edilir() {
    // 26 harf x farklı değiştirici birleşimi: hepsi benzersiz.
    let bindings: Vec<InputBinding> = (0..(InputBindingValidator::MAX_BINDINGS as i32 + 3))
        .map(|i| InputBinding::new(InputAction::OpenSearch, shortcut(HotkeyModifiers(1 + i / 26), 0x41 + i % 26)))
        .collect();

    let report = validate(&bindings);

    assert_eq!(InputBindingValidator::MAX_BINDINGS, report.accepted.len());
    assert_eq!(3, report.issues.iter().filter(|x| x.kind == InputIssueKind::TooMany).count());
}

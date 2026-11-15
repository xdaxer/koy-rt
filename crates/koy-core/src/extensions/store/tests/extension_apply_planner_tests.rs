//! Karşılığı: `Extensions/Store/ExtensionApplyPlannerTests.cs`.

use super::store_test_kit::*;
use crate::appearance::IconPack;
use crate::extensions::store::*;
use crate::notch::{LiveBackgroundSourceKind, LiveBackgroundTarget};
use crate::settings::AppSettings;

const FOLDER: &str = "C:\\Users\\u\\AppData\\Roaming\\Koy\\extensions\\koyhq.ornek";

fn inst(kind: ExtensionKind, payload: &str) -> InstalledEntry {
    installed("koyhq.ornek", kind, "1.0.0", false, payload)
}

fn inst_applied(kind: ExtensionKind, applied: bool, payload: &str) -> InstalledEntry {
    installed("koyhq.ornek", kind, "1.0.0", applied, payload)
}

fn apply(current: &AppSettings, entry: &InstalledEntry) -> ExtensionApplyPlan {
    ExtensionApplyPlanner::apply(current, entry, FOLDER, false)
}

fn revert(current: &AppSettings, entry: &InstalledEntry) -> ExtensionApplyPlan {
    ExtensionApplyPlanner::revert_on_uninstall(current, entry, FOLDER, None)
}

#[test]
fn theme_apply_backs_up_copies_and_enables_overrides() {
    let current = AppSettings::default();

    let plan = apply(&current, &inst(ExtensionKind::Theme, "theme.json"));

    assert!(plan.is_success());
    assert!(plan.settings.as_ref().unwrap().appearance.user_style_overrides_enabled);
    assert_eq!(ExtensionFileActionKind::BackupUserStyle, plan.file_actions[0].kind);
    assert_eq!(ExtensionFileActionKind::CopyThemeToUserStyle, plan.file_actions[1].kind);
    assert_eq!(Some(format!("{FOLDER}\\theme.json")), plan.file_actions[1].source_path);
    assert!(!current.appearance.user_style_overrides_enabled);
}

#[test]
fn iconpack_apply_points_to_icons_folder() {
    let plan = apply(&AppSettings::default(), &inst(ExtensionKind::IconPack, "icons"));

    let settings = plan.settings.unwrap();
    assert_eq!(IconPack::Folder, settings.appearance.icon_pack);
    assert_eq!(format!("{FOLDER}\\icons"), settings.appearance.icon_pack_folder);
    assert!(plan.file_actions.is_empty());
}

#[test]
fn background_apply_sets_local_file_and_defaults_target_to_notch_when_off() {
    let entry = inst(ExtensionKind::Background, "background.gif");

    let plan = apply(&AppSettings::default(), &entry);

    let settings = plan.settings.unwrap();
    assert_eq!(LiveBackgroundSourceKind::LocalFile, settings.notch.live_background_source);
    assert_eq!(format!("{FOLDER}\\background.gif"), settings.notch.live_background_file_path);
    assert_eq!(LiveBackgroundTarget::Notch, settings.notch.live_background_target);
}

#[test]
fn background_apply_keeps_an_existing_target() {
    let mut current = AppSettings::default();
    current.notch.live_background_target = LiveBackgroundTarget::MusicCard;

    let plan = apply(&current, &inst(ExtensionKind::Background, "background.webm"));

    assert_eq!(LiveBackgroundTarget::MusicCard, plan.settings.unwrap().notch.live_background_target);
}

#[test]
fn background_apply_rejects_a_bad_payload_name() {
    for payload in ["", "background.exe", "..\\background.gif"] {
        let plan = apply(&AppSettings::default(), &inst(ExtensionKind::Background, payload));

        assert_eq!(ExtensionApplyError::InvalidPath, plan.error, "{payload:?}");
        assert!(plan.settings.is_none(), "{payload:?}");
    }
}

#[test]
fn apply_rejects_unsafe_install_folders() {
    for folder in ["", "relative\\folder", "\\\\server\\share\\x", "C:\\a\\..\\b"] {
        assert_eq!(
            ExtensionApplyError::InvalidPath,
            ExtensionApplyPlanner::apply(&AppSettings::default(), &inst(ExtensionKind::IconPack, "icons"), folder, false).error,
            "{folder:?}"
        );
    }
}

#[test]
fn profile_apply_only_imports_and_leaves_settings_unchanged() {
    let current = AppSettings::default();

    let plan = apply(&current, &inst(ExtensionKind::Profile, "profile.json"));

    assert!(plan.is_success());
    assert_eq!(Some(current), plan.settings);
    assert_eq!(1, plan.file_actions.len());
    let action = &plan.file_actions[0];
    assert_eq!(ExtensionFileActionKind::ImportProfile, action.kind);
    assert_eq!(Some(format!("{FOLDER}\\profile.json")), action.source_path);
}

#[test]
fn apply_does_not_mutate_the_given_settings() {
    let current = AppSettings::default();

    apply(&current, &inst(ExtensionKind::IconPack, "icons"));
    apply(&current, &inst(ExtensionKind::Background, "background.gif"));

    assert_eq!(IconPack::Builtin, current.appearance.icon_pack);
    assert_eq!(LiveBackgroundTarget::None, current.notch.live_background_target);
    assert_eq!("", current.notch.live_background_file_path);
}

// Kaldırma

#[test]
fn uninstalling_an_applied_iconpack_returns_to_builtin() {
    let entry = inst(ExtensionKind::IconPack, "icons");
    let applied = apply(&AppSettings::default(), &entry).settings.unwrap();

    let plan = revert(&applied, &entry);

    let settings = plan.settings.unwrap();
    assert_eq!(IconPack::Builtin, settings.appearance.icon_pack);
    assert_eq!("", settings.appearance.icon_pack_folder);
}

#[test]
fn uninstalling_an_iconpack_the_user_replaced_does_not_touch_settings() {
    let entry = inst(ExtensionKind::IconPack, "icons");
    let mut current = AppSettings::default();
    current.appearance.icon_pack = IconPack::Folder;
    current.appearance.icon_pack_folder = "D:\\Baska\\Paket".to_string();

    let plan = revert(&current, &entry);

    let settings = plan.settings.unwrap();
    assert_eq!("D:\\Baska\\Paket", settings.appearance.icon_pack_folder);
    assert_eq!(IconPack::Folder, settings.appearance.icon_pack);
}

#[test]
fn icon_path_comparison_ignores_case_and_trailing_separator() {
    let entry = inst(ExtensionKind::IconPack, "icons");
    let mut current = AppSettings::default();
    current.appearance.icon_pack = IconPack::Folder;
    current.appearance.icon_pack_folder = format!("{FOLDER}\\ICONS\\").to_uppercase();

    assert_eq!(IconPack::Builtin, revert(&current, &entry).settings.unwrap().appearance.icon_pack);
}

#[test]
fn uninstalling_an_applied_background_returns_to_ambient() {
    let entry = inst(ExtensionKind::Background, "background.gif");
    let applied = apply(&AppSettings::default(), &entry).settings.unwrap();

    let plan = revert(&applied, &entry);

    let settings = plan.settings.unwrap();
    assert_eq!(LiveBackgroundSourceKind::BuiltInAmbient, settings.notch.live_background_source);
    assert_eq!("", settings.notch.live_background_file_path);
    assert_eq!(applied.notch.live_background_target, settings.notch.live_background_target);
}

#[test]
fn uninstalling_a_background_the_user_replaced_does_not_touch_settings() {
    let entry = inst(ExtensionKind::Background, "background.gif");
    let mut current = AppSettings::default();
    current.notch.live_background_source = LiveBackgroundSourceKind::LocalFile;
    current.notch.live_background_file_path = "D:\\mine.gif".to_string();

    let plan = revert(&current, &entry);

    assert_eq!("D:\\mine.gif", plan.settings.unwrap().notch.live_background_file_path);
}

#[test]
fn uninstalling_an_applied_theme_restores_backup_and_keeps_overrides_when_a_backup_exists() {
    let entry = inst_applied(ExtensionKind::Theme, true, "theme.json");
    let mut current = AppSettings::default();
    current.appearance.user_style_overrides_enabled = true;

    let with_backup = ExtensionApplyPlanner::revert_on_uninstall(&current, &entry, FOLDER, Some(UserStyleBackup::new(true, true)));
    let without_backup = ExtensionApplyPlanner::revert_on_uninstall(&current, &entry, FOLDER, None);

    assert_eq!(1, with_backup.file_actions.len());
    assert_eq!(ExtensionFileActionKind::RestoreUserStyleBackup, with_backup.file_actions[0].kind);
    assert!(with_backup.settings.unwrap().appearance.user_style_overrides_enabled);
    assert_eq!(1, without_backup.file_actions.len());
    assert_eq!(ExtensionFileActionKind::RestoreUserStyleBackup, without_backup.file_actions[0].kind);
    assert!(!without_backup.settings.unwrap().appearance.user_style_overrides_enabled);
}

#[test]
fn theme_apply_records_the_original_overrides_flag_in_the_backup_action() {
    let mut current = AppSettings::default();
    current.appearance.user_style_overrides_enabled = true;

    let plan = apply(&current, &inst(ExtensionKind::Theme, "theme.json"));

    assert!(plan.file_actions[0].original_overrides_enabled);
    assert!(!apply(&AppSettings::default(), &inst(ExtensionKind::Theme, "theme.json")).file_actions[0].original_overrides_enabled);
}

#[test]
fn switching_between_themes_never_backs_up_the_previous_store_theme() {
    let entry = inst(ExtensionKind::Theme, "theme.json");
    let mut current = AppSettings::default();
    current.appearance.user_style_overrides_enabled = true;

    let plan = ExtensionApplyPlanner::apply(&current, &entry, FOLDER, true);

    assert_eq!(1, plan.file_actions.len());
    assert_eq!(ExtensionFileActionKind::CopyThemeToUserStyle, plan.file_actions[0].kind);
    assert!(plan.settings.unwrap().appearance.user_style_overrides_enabled);
}

#[test]
fn uninstalling_the_applied_theme_restores_the_original_state_even_when_there_was_no_original_file() {
    let entry = inst_applied(ExtensionKind::Theme, true, "theme.json");
    let mut current = AppSettings::default();
    current.appearance.user_style_overrides_enabled = true;

    // Özgün: dosya yoktu, geçersiz kılma kapalıydı.
    let none = ExtensionApplyPlanner::revert_on_uninstall(&current, &entry, FOLDER, Some(UserStyleBackup::new(false, false)));
    // Özgün: dosya vardı ama geçersiz kılma kapalıydı.
    let off_with_file = ExtensionApplyPlanner::revert_on_uninstall(&current, &entry, FOLDER, Some(UserStyleBackup::new(true, false)));

    assert!(!none.settings.as_ref().unwrap().appearance.user_style_overrides_enabled);
    assert!(!off_with_file.settings.unwrap().appearance.user_style_overrides_enabled);
    assert_eq!(1, none.file_actions.len());
    assert_eq!(ExtensionFileActionKind::RestoreUserStyleBackup, none.file_actions[0].kind);
}

#[test]
fn theme_chain_a_then_b_then_uninstalling_either_leaves_a_consistent_state() {
    let a = InstalledEntry { id: "koyhq.a".to_string(), ..inst(ExtensionKind::Theme, "theme.json") };
    let b = InstalledEntry { id: "koyhq.b".to_string(), ..inst(ExtensionKind::Theme, "theme.json") };

    // A uygulanır: yedek alınır. B uygulanır: kayıtta uygulanmış tema var, yedek alınmaz.
    let apply_a = apply(&AppSettings::default(), &a);
    let mut reg = registry(vec![a.clone(), b.clone()]).with_applied("koyhq.a");
    let apply_b = ExtensionApplyPlanner::apply(apply_a.settings.as_ref().unwrap(), &b, FOLDER, reg.has_applied(ExtensionKind::Theme));
    reg = reg.with_applied("koyhq.b");
    assert!(apply_a.file_actions.iter().any(|x| x.kind == ExtensionFileActionKind::BackupUserStyle));
    assert!(!apply_b.file_actions.iter().any(|x| x.kind == ExtensionFileActionKind::BackupUserStyle));

    // A'yı kaldırmak B'ye dokunmaz (A artık uygulanmış değil).
    let remove_a = ExtensionApplyPlanner::revert_on_uninstall(
        apply_b.settings.as_ref().unwrap(),
        reg.find(Some("koyhq.a")).unwrap(),
        FOLDER,
        Some(UserStyleBackup::new(false, false)),
    );
    assert!(remove_a.file_actions.is_empty());
    assert!(remove_a.settings.unwrap().appearance.user_style_overrides_enabled);

    // B'yi kaldırmak özgün duruma döner.
    let remove_b = ExtensionApplyPlanner::revert_on_uninstall(
        apply_b.settings.as_ref().unwrap(),
        reg.find(Some("koyhq.b")).unwrap(),
        FOLDER,
        Some(UserStyleBackup::new(false, false)),
    );
    assert_eq!(1, remove_b.file_actions.len());
    assert_eq!(ExtensionFileActionKind::RestoreUserStyleBackup, remove_b.file_actions[0].kind);
    assert!(!remove_b.settings.unwrap().appearance.user_style_overrides_enabled);
}

#[test]
fn backup_state_text_parses_and_round_trips() {
    let rows: [(Option<&str>, bool, bool); 5] = [
        (Some("overrides=true"), true, true),
        (Some("overrides=false\n"), true, false),
        (Some(""), false, false),
        (Some("overrides=maybe"), false, false),
        (None, false, false),
    ];
    for (text, ok, value) in rows {
        let parsed = UserStyleBackup::try_parse_state(text);

        assert_eq!(ok, parsed.is_some(), "{text:?}");
        assert_eq!(value, parsed.unwrap_or(false), "{text:?}");
    }
    assert_eq!(Some(true), UserStyleBackup::try_parse_state(Some(&UserStyleBackup::serialize_state(true))));
    assert_eq!(Some(false), UserStyleBackup::try_parse_state(Some(&UserStyleBackup::serialize_state(false))));
}

#[test]
fn uninstalling_a_theme_that_was_not_applied_does_nothing() {
    let entry = inst_applied(ExtensionKind::Theme, false, "theme.json");
    let mut current = AppSettings::default();
    current.appearance.user_style_overrides_enabled = true;

    let plan = revert(&current, &entry);

    assert!(plan.file_actions.is_empty());
    assert!(plan.settings.unwrap().appearance.user_style_overrides_enabled);
}

#[test]
fn uninstalling_a_profile_does_nothing() {
    let plan = revert(&AppSettings::default(), &inst(ExtensionKind::Profile, "profile.json"));

    assert!(plan.is_success());
    assert!(plan.file_actions.is_empty());
}

#[test]
fn is_active_follows_settings_for_icons_and_background_and_flag_for_theme() {
    let icons = inst(ExtensionKind::IconPack, "icons");
    let bg = inst(ExtensionKind::Background, "background.gif");
    let theme = inst_applied(ExtensionKind::Theme, true, "theme.json");
    let mut settings = AppSettings::default();

    assert!(!ExtensionApplyPlanner::is_active(&settings, &icons, FOLDER));
    assert!(!ExtensionApplyPlanner::is_active(&settings, &bg, FOLDER));
    assert!(!ExtensionApplyPlanner::is_active(&settings, &theme, FOLDER));

    settings = apply(&settings, &icons).settings.unwrap();
    settings = apply(&settings, &bg).settings.unwrap();
    settings = apply(&settings, &theme).settings.unwrap();

    assert!(ExtensionApplyPlanner::is_active(&settings, &icons, FOLDER));
    assert!(ExtensionApplyPlanner::is_active(&settings, &bg, FOLDER));
    assert!(ExtensionApplyPlanner::is_active(&settings, &theme, FOLDER));
    assert!(!ExtensionApplyPlanner::is_active(&settings, &default_installed_kind(ExtensionKind::Profile), FOLDER));
}

fn default_installed_kind(kind: ExtensionKind) -> InstalledEntry {
    inst(kind, "")
}

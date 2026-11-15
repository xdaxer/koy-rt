//! Karşılığı: `Extensions/Store/PackageEntryPolicyTests.cs`.

use crate::appearance::UserStyleParser;
use crate::extensions::store::*;

fn e(name: &str) -> PackageEntry {
    PackageEntry::new(name, 10)
}

fn el(name: &str, length: i64) -> PackageEntry {
    PackageEntry::new(name, length)
}

fn check_theme(entries: &[PackageEntry]) -> PackageCheck {
    PackageEntryPolicy::check(entries, ExtensionKind::Theme, 1000)
}

fn valid_theme() -> Vec<PackageEntry> {
    vec![e("manifest.json"), e("theme.json")]
}

#[test]
fn valid_packages_of_every_kind_pass() {
    assert!(PackageEntryPolicy::check(&[e("manifest.json"), e("theme.json"), e("preview.png")], ExtensionKind::Theme, 1).is_ok());
    assert!(PackageEntryPolicy::check(&[e("manifest.json"), e("profile.json")], ExtensionKind::Profile, 1).is_ok());
    assert!(
        PackageEntryPolicy::check(&[e("manifest.json"), e("icons/Firefox.png"), e("icons/Code.ICO"), e("icons/x.svg")], ExtensionKind::IconPack, 1).is_ok()
    );
    assert!(PackageEntryPolicy::check(&[e("manifest.json"), e("background.webm")], ExtensionKind::Background, 1).is_ok());
}

#[test]
fn zip_slip_and_unsafe_names_are_rejected() {
    let rows = [
        "../evil.json",
        "icons/../../evil.png",
        "..",
        "/etc/passwd",
        "/theme.json",
        "C:/Windows/x.png",
        "C:\\Windows\\x.png",
        "c:evil.png",
        "icons\\a.png",
        "icons/a.png:stream",
        "theme.json:hidden",
        "",
        "./theme.json",
        "icons//a.png",
        "icons/a.png.",
        "icons/a.png ",
        "icons/a\0.png",
        "icons/a|b.png",
        "\\\\server\\share\\x.png",
    ];
    for name in rows {
        let result = PackageEntryPolicy::check_common(&[e("manifest.json"), e(name)], 1);

        assert_eq!(ExtensionInstallError::UnsafePath, result.error, "{name:?}");
        assert_eq!(Some(name.to_string()), result.entry, "{name:?}");
    }
}

#[test]
fn unsafe_name_in_full_check_is_also_rejected() {
    assert_eq!(ExtensionInstallError::UnsafePath, check_theme(&[e("manifest.json"), e("theme.json"), e("../x")]).error);
}

#[test]
fn download_size_limit_is_inclusive() {
    assert!(PackageEntryPolicy::check(&valid_theme(), ExtensionKind::Theme, PackageEntryPolicy::MAX_DOWNLOAD_BYTES).is_ok());
    assert_eq!(
        ExtensionInstallError::DownloadTooLarge,
        PackageEntryPolicy::check(&valid_theme(), ExtensionKind::Theme, PackageEntryPolicy::MAX_DOWNLOAD_BYTES + 1).error
    );
}

#[test]
fn extracted_total_limit_is_inclusive() {
    let at_limit = [el("manifest.json", 100), el("background.gif", PackageEntryPolicy::MAX_EXTRACTED_BYTES - 100)];
    let over = [el("manifest.json", 101), el("background.gif", PackageEntryPolicy::MAX_EXTRACTED_BYTES - 100)];

    assert!(PackageEntryPolicy::check(&at_limit, ExtensionKind::Background, 1).is_ok());
    assert_eq!(ExtensionInstallError::ExtractedTooLarge, PackageEntryPolicy::check(&over, ExtensionKind::Background, 1).error);
}

#[test]
fn total_does_not_overflow_with_huge_declared_lengths() {
    let result = PackageEntryPolicy::check_common(&[el("manifest.json", i64::MAX / 2 + 1), el("a.bin", i64::MAX / 2 + 1)], 1);

    assert_eq!(ExtensionInstallError::ExtractedTooLarge, result.error);
}

#[test]
fn file_count_limit_is_inclusive() {
    let icons = |count: usize| -> Vec<PackageEntry> {
        let mut entries = vec![e("manifest.json")];
        entries.extend((0..count - 1).map(|i| el(&format!("icons/i{i}.png"), 1)));
        entries
    };

    assert!(PackageEntryPolicy::check(&icons(PackageEntryPolicy::MAX_FILES), ExtensionKind::IconPack, 1).is_ok());
    assert_eq!(
        ExtensionInstallError::TooManyFiles,
        PackageEntryPolicy::check(&icons(PackageEntryPolicy::MAX_FILES + 1), ExtensionKind::IconPack, 1).error
    );
}

#[test]
fn negative_declared_length_is_invalid() {
    assert_eq!(ExtensionInstallError::InvalidArchive, PackageEntryPolicy::check_common(&[el("manifest.json", -1)], 1).error);
}

#[test]
fn duplicate_names_are_rejected_case_insensitively() {
    assert_eq!(ExtensionInstallError::DuplicateEntry, check_theme(&[e("manifest.json"), e("theme.json"), e("THEME.JSON")]).error);
    assert_eq!(
        ExtensionInstallError::DuplicateEntry,
        PackageEntryPolicy::check(&[e("manifest.json"), e("icons/a.png"), e("icons/A.png")], ExtensionKind::IconPack, 1).error
    );
}

#[test]
fn missing_manifest_is_rejected() {
    assert_eq!(ExtensionInstallError::MissingManifest, check_theme(&[e("theme.json")]).error);
    assert_eq!(ExtensionInstallError::MissingManifest, check_theme(&[e("Manifest.json"), e("theme.json")]).error);
}

#[test]
fn missing_payload_is_rejected_for_every_kind() {
    assert_eq!(ExtensionInstallError::MissingPayload, PackageEntryPolicy::check(&[e("manifest.json")], ExtensionKind::Theme, 1).error);
    assert_eq!(
        ExtensionInstallError::MissingPayload,
        PackageEntryPolicy::check(&[e("manifest.json"), e("preview.png")], ExtensionKind::Profile, 1).error
    );
    assert_eq!(
        ExtensionInstallError::MissingPayload,
        PackageEntryPolicy::check(&[e("manifest.json"), e("icons/")], ExtensionKind::IconPack, 1).error
    );
    assert_eq!(ExtensionInstallError::MissingPayload, PackageEntryPolicy::check(&[e("manifest.json")], ExtensionKind::Background, 1).error);
}

#[test]
fn background_needs_exactly_one_file() {
    assert_eq!(
        ExtensionInstallError::MultiplePayloads,
        PackageEntryPolicy::check(&[e("manifest.json"), e("background.gif"), e("background.webm")], ExtensionKind::Background, 1).error
    );
}

#[test]
fn wrong_background_files_are_disallowed() {
    for name in ["background.png", "background.exe", "bg.gif", "Background.gif", "sub/background.gif", "background"] {
        assert_eq!(
            ExtensionInstallError::DisallowedFile,
            PackageEntryPolicy::check(&[e("manifest.json"), e(name)], ExtensionKind::Background, 1).error,
            "{name}"
        );
    }
}

#[test]
fn wrong_icon_files_are_disallowed() {
    for name in ["icons/a.exe", "icons/a.dll", "icons/a", "icons/sub/a.png", "icons/NUL.png", "icons/con.ico", "a.png", "Icons/a.png"] {
        assert_eq!(
            ExtensionInstallError::DisallowedFile,
            PackageEntryPolicy::check(&[e("manifest.json"), e("icons/ok.png"), e(name)], ExtensionKind::IconPack, 1).error,
            "{name}"
        );
    }
}

#[test]
fn files_of_another_kind_are_disallowed() {
    assert_eq!(
        ExtensionInstallError::DisallowedFile,
        PackageEntryPolicy::check(&[e("manifest.json"), e("theme.json"), e("profile.json")], ExtensionKind::Theme, 1).error
    );
    assert_eq!(
        ExtensionInstallError::DisallowedFile,
        PackageEntryPolicy::check(&[e("manifest.json"), e("profile.json"), e("theme.json")], ExtensionKind::Profile, 1).error
    );
    assert_eq!(ExtensionInstallError::DisallowedFile, check_theme(&[e("manifest.json"), e("theme.json"), e("run.ps1")]).error);
    assert_eq!(ExtensionInstallError::DisallowedFile, check_theme(&[e("manifest.json"), e("theme.json"), e("Preview.PNG")]).error);
}

#[test]
fn directory_entries_are_allowed_only_as_icons_folder_of_iconpack() {
    assert!(PackageEntryPolicy::check(&[e("manifest.json"), el("icons/", 0), e("icons/a.png")], ExtensionKind::IconPack, 1).is_ok());
    assert_eq!(ExtensionInstallError::DisallowedFile, check_theme(&[e("manifest.json"), e("theme.json"), el("icons/", 0)]).error);
    assert_eq!(
        ExtensionInstallError::DisallowedFile,
        PackageEntryPolicy::check(&[e("manifest.json"), el("other/", 0), e("icons/a.png")], ExtensionKind::IconPack, 1).error
    );
    assert_eq!(ExtensionInstallError::UnsafePath, PackageEntryPolicy::check_common(&[e("manifest.json"), el("../", 0)], 1).error);
}

#[test]
fn per_file_size_limits_are_enforced() {
    let max_style = i64::from(UserStyleParser::MAX_BYTES);
    assert_eq!(
        ExtensionInstallError::FileTooLarge,
        PackageEntryPolicy::check(&[el("manifest.json", PackageEntryPolicy::MAX_MANIFEST_BYTES + 1), e("theme.json")], ExtensionKind::Theme, 1).error
    );
    assert_eq!(
        ExtensionInstallError::FileTooLarge,
        PackageEntryPolicy::check(&[e("manifest.json"), el("theme.json", max_style + 1)], ExtensionKind::Theme, 1).error
    );
    assert_eq!(
        ExtensionInstallError::FileTooLarge,
        PackageEntryPolicy::check(&[e("manifest.json"), el("icons/a.png", PackageEntryPolicy::MAX_ICON_BYTES + 1)], ExtensionKind::IconPack, 1).error
    );
    assert!(
        PackageEntryPolicy::check(
            &[el("manifest.json", PackageEntryPolicy::MAX_MANIFEST_BYTES), el("theme.json", max_style)],
            ExtensionKind::Theme,
            1
        )
        .is_ok()
    );
}

#[test]
fn name_length_limit_is_enforced() {
    let long_name = format!("icons/{}.png", "a".repeat(PackageEntryPolicy::MAX_ENTRY_NAME_LENGTH));

    assert_eq!(ExtensionInstallError::UnsafePath, PackageEntryPolicy::check_common(&[e("manifest.json"), e(&long_name)], 1).error);
}

#[test]
fn empty_entry_list_has_no_manifest() {
    assert_eq!(ExtensionInstallError::MissingManifest, PackageEntryPolicy::check_common(&[], 1).error);
}

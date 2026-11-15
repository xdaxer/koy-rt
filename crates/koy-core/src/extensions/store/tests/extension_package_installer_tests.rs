//! Karşılığı: `Extensions/Store/ExtensionPackageInstallerTests.cs`.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use chrono::{TimeDelta, TimeZone};

use super::store_test_kit::*;
use crate::extensions::store::uri::Uri;
use crate::extensions::store::*;
use crate::time::DateTimeOffset;

struct Fixture {
    dir: TempDir,
    registry: Arc<InstalledRegistryStore>,
    installer: ExtensionPackageInstaller,
    root: PathBuf,
    now: Arc<Mutex<DateTimeOffset>>,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = TempDir::new();
        let root = dir.combine("extensions");
        let registry = Arc::new(InstalledRegistryStore::new(Some(dir.combine("extensions.json")), None));
        let now = Arc::new(Mutex::new(chrono::FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2026, 11, 12, 12, 0, 0).unwrap()));
        let clock_now = Arc::clone(&now);
        let installer = ExtensionPackageInstaller::new(&root, Arc::clone(&registry), Some(Box::new(move || *clock_now.lock().unwrap())));
        Fixture { dir, registry, installer, root, now }
    }

    fn now(&self) -> DateTimeOffset {
        *self.now.lock().unwrap()
    }

    fn install(&self, zip: Cursor<Vec<u8>>) -> InstallResult {
        self.install_with(zip, None, ExtensionSource::File)
    }

    fn install_with(&self, zip: Cursor<Vec<u8>>, expected: Option<&StoreItem>, source: ExtensionSource) -> InstallResult {
        let length = zip.get_ref().len() as i64;
        self.installer.install(zip, length, expected, source)
    }

    fn leftover_temp(&self) -> Vec<String> {
        if !self.root.is_dir() {
            return Vec::new();
        }

        fs::read_dir(&self.root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('.'))
            .collect()
    }

    fn entry(&self, id: &str) -> Option<InstalledEntry> {
        self.registry.load().find(Some(id)).cloned()
    }
}

fn theme_package(id: &str, version: &str) -> Cursor<Vec<u8>> {
    zip(vec![f("manifest.json", manifest(id, "theme", version, None)), f("theme.json", THEME_JSON), f("preview.png", png())])
}

fn default_theme_package() -> Cursor<Vec<u8>> {
    theme_package("koyhq.ornek", "1.0.0")
}

#[test]
fn theme_installs_extracts_and_registers() {
    let fx = Fixture::new();

    let result = fx.install(default_theme_package());

    assert!(result.is_success(), "{:?}", result.error);
    let entry = result.entry.clone().unwrap();
    assert_eq!("theme.json", entry.payload_file);
    assert_eq!(ExtensionKind::Theme, entry.kind);
    assert_eq!(fx.now(), entry.installed_at);
    assert!(fx.root.join("koyhq.ornek").join("theme.json").is_file());
    assert!(fx.root.join("koyhq.ornek").join("manifest.json").is_file());
    assert!(fx.root.join("koyhq.ornek").join("preview.png").is_file());
    assert_eq!(Some(entry), fx.entry("koyhq.ornek"));
    assert!(fx.leftover_temp().is_empty());
}

#[test]
fn store_install_matches_the_index_item() {
    let fx = Fixture::new();
    let item = item("koyhq.ornek", ExtensionKind::Theme, "1.0.0");

    let result = fx.install_with(default_theme_package(), Some(&item), ExtensionSource::Store);

    assert!(result.is_success());
    assert_eq!(ExtensionSource::Store, result.entry.unwrap().source);
}

#[test]
fn index_mismatch_installs_nothing() {
    for (id, version) in [("koyhq.baska", "1.0.0"), ("koyhq.ornek", "1.0.1")] {
        let fx = Fixture::new();
        let item = item(id, ExtensionKind::Theme, version);

        let result = fx.install_with(default_theme_package(), Some(&item), ExtensionSource::Store);

        assert_eq!(ExtensionInstallError::ManifestMismatch, result.error, "{id} {version}");
        assert!(fx.registry.load().entries().is_empty());
        assert!(!fx.root.join("koyhq.ornek").is_dir());
        assert!(fx.leftover_temp().is_empty());
    }
}

#[test]
fn kind_mismatch_with_index_is_rejected() {
    let fx = Fixture::new();

    let result = fx.install_with(default_theme_package(), Some(&item("koyhq.ornek", ExtensionKind::Profile, "1.0.0")), ExtensionSource::Store);

    assert_eq!(ExtensionInstallError::ManifestMismatch, result.error);
}

#[test]
fn iconpack_installs_with_icons_folder_payload() {
    let fx = Fixture::new();
    let package = zip(vec![
        f("manifest.json", manifest("koyhq.icons", "iconpack", "1.0.0", None)),
        f("icons/Firefox.png", png()),
        f("icons/Code.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
    ]);

    let result = fx.install(package);

    assert!(result.is_success(), "{:?}", result.error);
    assert_eq!("icons", result.entry.unwrap().payload_file);
    assert!(fx.root.join("koyhq.icons").join("icons").join("Firefox.png").is_file());
}

#[test]
fn iconpack_with_wrong_signature_is_rejected_and_cleaned_up() {
    let fx = Fixture::new();
    let package = zip(vec![f("manifest.json", manifest("koyhq.icons", "iconpack", "1.0.0", None)), f("icons/Firefox.png", "MZ not an image")]);

    assert_eq!(ExtensionInstallError::InvalidPayload, fx.install(package).error);
    assert!(!fx.root.join("koyhq.icons").is_dir());
    assert!(fx.leftover_temp().is_empty());
}

#[test]
fn svg_with_script_is_rejected() {
    let fx = Fixture::new();
    let package = zip(vec![f("manifest.json", manifest("koyhq.icons", "iconpack", "1.0.0", None)), f("icons/x.svg", "<svg><script>alert(1)</script></svg>")]);

    assert_eq!(ExtensionInstallError::InvalidPayload, fx.install(package).error);
}

#[test]
fn background_installs_and_records_the_actual_file_name() {
    let fx = Fixture::new();
    let package = zip(vec![f("manifest.json", manifest("koyhq.bg", "background", "1.0.0", None)), f("background.gif", gif())]);

    let result = fx.install(package);

    assert!(result.is_success(), "{:?}", result.error);
    assert_eq!("background.gif", result.entry.unwrap().payload_file);
}

#[test]
fn background_with_mismatching_signature_is_rejected() {
    let fx = Fixture::new();
    let package = zip(vec![f("manifest.json", manifest("koyhq.bg", "background", "1.0.0", None)), f("background.mp4", gif())]);

    assert_eq!(ExtensionInstallError::InvalidPayload, fx.install(package).error);
}

#[test]
fn profile_installs_when_codec_accepts_it_and_rejects_otherwise() {
    let fx = Fixture::new();
    let good = zip(vec![f("manifest.json", manifest("koyhq.prof", "profile", "1.0.0", None)), f("profile.json", profile_json())]);
    let bad = zip(vec![f("manifest.json", manifest("koyhq.prof2", "profile", "1.0.0", None)), f("profile.json", "{ \"Format\": \"x\" }")]);

    assert!(fx.install(good).is_success());
    assert_eq!(ExtensionInstallError::InvalidPayload, fx.install(bad).error);
    assert!(fx.entry("koyhq.prof2").is_none());
}

#[test]
fn theme_without_valid_entries_is_rejected() {
    for theme_json in ["{ \"NotAToken\": \"#112233\" }", "[]", "not json", "{}"] {
        let fx = Fixture::new();
        let package = zip(vec![f("manifest.json", manifest("koyhq.ornek", "theme", "1.0.0", None)), f("theme.json", theme_json)]);

        assert_eq!(ExtensionInstallError::InvalidPayload, fx.install(package).error, "{theme_json}");
    }
}

#[test]
fn theme_with_utf8_bom_is_accepted() {
    let fx = Fixture::new();
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(THEME_JSON.as_bytes());
    let package = zip(vec![f("manifest.json", manifest("koyhq.ornek", "theme", "1.0.0", None)), f("theme.json", bytes)]);

    assert!(fx.install(package).is_success());
}

#[test]
fn zip_slip_entry_is_rejected_and_nothing_is_written_outside() {
    let fx = Fixture::new();
    let package = zip(vec![f("manifest.json", manifest("koyhq.ornek", "theme", "1.0.0", None)), f("theme.json", THEME_JSON), f("../evil.txt", "x")]);

    let result = fx.install(package);

    assert_eq!(ExtensionInstallError::UnsafePath, result.error);
    assert!(!fx.dir.combine("evil.txt").exists());
    assert!(!fx.root.join("evil.txt").exists());
    assert!(fx.leftover_temp().is_empty());
}

#[test]
fn disallowed_executable_is_rejected() {
    let fx = Fixture::new();
    let package = zip(vec![f("manifest.json", manifest("koyhq.ornek", "theme", "1.0.0", None)), f("theme.json", THEME_JSON), f("run.exe", "MZ")]);

    assert_eq!(ExtensionInstallError::DisallowedFile, fx.install(package).error);
}

#[test]
fn package_requesting_capabilities_is_rejected() {
    let fx = Fixture::new();
    let json = manifest("koyhq.ornek", "theme", "1.0.0", None).replace("\"capabilities\": []", "\"capabilities\": [\"files\"]");
    let package = zip(vec![f("manifest.json", json), f("theme.json", THEME_JSON)]);

    assert_eq!(ExtensionInstallError::CapabilitiesNotAllowed, fx.install(package).error);
}

#[test]
fn missing_manifest_and_garbage_archives_fail_cleanly() {
    let fx = Fixture::new();

    assert_eq!(ExtensionInstallError::MissingManifest, fx.install(zip(vec![f("theme.json", THEME_JSON)])).error);
    assert_eq!(
        ExtensionInstallError::InvalidArchive,
        fx.install(Cursor::new(b"this is not a zip file at all".to_vec())).error
    );
    assert_eq!(ExtensionInstallError::InvalidArchive, fx.install(Cursor::new(Vec::new())).error);
    assert!(fx.registry.load().entries().is_empty());
}

#[test]
fn oversized_download_is_rejected_before_opening() {
    let fx = Fixture::new();

    assert_eq!(
        ExtensionInstallError::DownloadTooLarge,
        fx.installer.install(default_theme_package(), PackageEntryPolicy::MAX_DOWNLOAD_BYTES + 1, None, ExtensionSource::File).error
    );
}

#[test]
fn extracted_size_over_limit_is_rejected() {
    let fx = Fixture::new();
    // 51 MB sıfır: zip'te çok küçük (deflate), açılınca sınırı aşar.
    let mut payload = gif();
    payload.extend(std::iter::repeat_n(0u8, 51 * 1024 * 1024));
    let package = zip(vec![f("manifest.json", manifest("koyhq.bg", "background", "1.0.0", None)), ZipFile::new("background.gif", payload).deflated()]);
    assert!(package.get_ref().len() < 2 * 1024 * 1024, "sıkıştırılmış paket küçük olmalı");

    assert_eq!(ExtensionInstallError::ExtractedTooLarge, fx.install(package).error);
    assert!(fx.leftover_temp().is_empty());
}

#[test]
fn update_replaces_old_files_and_keeps_applied_flag() {
    let fx = Fixture::new();
    let first = fx.install(theme_package("koyhq.ornek", "1.0.0"));
    fx.registry.update(|r| Some(r.with_applied("koyhq.ornek")));
    fs::write(fx.root.join("koyhq.ornek").join("stale.txt"), "old").unwrap();
    *fx.now.lock().unwrap() += TimeDelta::days(1);

    let second = fx.install(theme_package("koyhq.ornek", "1.1.0"));

    assert!(first.is_success() && second.is_success());
    assert!(second.previously_applied);
    assert_eq!("1.1.0", fx.entry("koyhq.ornek").unwrap().version);
    assert!(fx.entry("koyhq.ornek").unwrap().applied);
    assert!(!fx.root.join("koyhq.ornek").join("stale.txt").exists());
    assert_eq!(1, fx.registry.load().entries().len());
    assert!(fx.leftover_temp().is_empty());
}

#[test]
fn same_id_with_a_different_kind_is_a_conflict_and_keeps_the_old_install() {
    let fx = Fixture::new();
    fx.install(default_theme_package());
    let profile = zip(vec![f("manifest.json", manifest("koyhq.ornek", "profile", "1.0.0", None)), f("profile.json", profile_json())]);

    assert_eq!(ExtensionInstallError::KindConflict, fx.install(profile).error);
    assert_eq!(ExtensionKind::Theme, fx.entry("koyhq.ornek").unwrap().kind);
    assert!(fx.root.join("koyhq.ornek").join("theme.json").is_file());
}

#[test]
fn install_limit_of_64_is_enforced_for_new_ids_only() {
    let fx = Fixture::new();
    for i in 0..64 {
        assert!(fx.install(theme_package(&format!("ext-{i}"), "1.0.0")).is_success());
    }

    assert_eq!(ExtensionInstallError::TooManyInstalled, fx.install(theme_package("one-more", "1.0.0")).error);
    assert!(!fx.root.join("one-more").is_dir());
    assert!(fx.install(theme_package("ext-5", "2.0.0")).is_success());
}

#[test]
fn uninstall_removes_folder_and_registry_entry_only_for_registered_ids() {
    let fx = Fixture::new();
    fx.install(theme_package("koyhq.one", "1.0.0"));
    fx.install(theme_package("koyhq.two", "1.0.0"));
    fs::create_dir_all(fx.root.join("unregistered")).unwrap();

    assert!(fx.installer.uninstall("koyhq.one"));

    assert!(!fx.root.join("koyhq.one").is_dir());
    assert!(fx.root.join("koyhq.two").is_dir());
    let loaded = fx.registry.load();
    assert_eq!(vec!["koyhq.two"], loaded.entries().iter().map(|e| e.id.as_str()).collect::<Vec<_>>());

    assert!(!fx.installer.uninstall("unregistered"));
    assert!(fx.root.join("unregistered").is_dir());
    assert!(!fx.installer.uninstall("koyhq.one"));
}

#[test]
fn uninstall_refuses_ids_that_could_escape_the_root() {
    for id in ["..", "..\\outside", "../outside", "", "C:\\Windows", "."] {
        let fx = Fixture::new();
        fs::create_dir_all(fx.dir.combine("outside")).unwrap();

        assert!(!fx.installer.uninstall(id), "{id:?}");
        assert!(fx.dir.combine("outside").is_dir(), "{id:?}");
    }
}

#[test]
fn uninstall_of_registered_entry_with_missing_folder_still_clears_the_registry() {
    let fx = Fixture::new();
    fx.install(default_theme_package());
    fs::remove_dir_all(fx.root.join("koyhq.ornek")).unwrap();

    assert!(fx.installer.uninstall("koyhq.ornek"));
    assert!(fx.registry.load().entries().is_empty());
}

#[cfg(windows)]
fn set_modified(path: &Path, time: SystemTime) {
    use std::os::windows::fs::OpenOptionsExt;

    // FILE_WRITE_ATTRIBUTES + FILE_FLAG_BACKUP_SEMANTICS: klasör zamanı da ayarlanabilsin.
    let handle = fs::OpenOptions::new().access_mode(0x100).custom_flags(0x0200_0000).open(path).unwrap();
    handle.set_modified(time).unwrap();
}

#[cfg(windows)]
#[test]
fn stale_temp_folders_and_files_are_cleaned_but_fresh_ones_are_kept() {
    let fx = Fixture::new();
    fs::create_dir_all(&fx.root).unwrap();
    let stale = fx.root.join(".tmp-old");
    let fresh = fx.root.join(".tmp-fresh");
    let stale_file = fx.root.join(".tmp-old.part");
    let keep = fx.root.join("koyhq.keep");
    for d in [&stale, &fresh, &keep] {
        fs::create_dir_all(d).unwrap();
    }

    fs::write(&stale_file, "x").unwrap();
    let now: SystemTime = fx.now().into();
    let past = now - Duration::from_secs(3 * 3600);
    set_modified(&stale, past);
    set_modified(&stale_file, past);
    set_modified(&fresh, now);
    set_modified(&keep, past);

    fx.installer.cleanup_stale();

    assert!(!stale.exists());
    assert!(!stale_file.exists());
    assert!(fresh.is_dir());
    assert!(keep.is_dir());
}

#[test]
fn store_packages_in_the_repository_install_cleanly() {
    let fx = Fixture::new();
    let Some(root) = repo_root() else {
        eprintln!("atlandı: C# deposu bulunamadı");
        return;
    };
    let packages = root.join("store").join("packages");
    if !packages.is_dir() {
        return; // depo kökü yok: atla
    }

    let index = StoreIndexParser::parse_bytes(&fs::read(root.join("store").join("index.json")).unwrap());
    assert!(index.is_success());
    let index = index.index.unwrap();

    for item in &index.items {
        let uri = Uri::new(&item.package.url).unwrap();
        let file = packages.join(uri.absolute_path().rsplit('/').next().unwrap());
        assert!(file.is_file(), "{}", file.display());

        let bytes = fs::read(&file).unwrap();
        assert_eq!(item.package.sha256, sha256_hex(&bytes));
        assert_eq!(item.package.size, bytes.len() as i64);

        let result = fx.installer.install(fs::File::open(&file).unwrap(), bytes.len() as i64, Some(item), ExtensionSource::Store);

        assert!(result.is_success(), "{}: {:?}", item.id, result.error);
        assert_eq!(item.kind, result.entry.unwrap().kind);
    }

    assert_eq!(index.items.len(), fx.registry.load().entries().len());
}

// ---------------------------------------------------------------- Rust'a özgü ek testler (zip okuyucu + açıcı bütünleşmesi)

#[test]
fn deflated_entries_install_like_stored_ones() {
    let fx = Fixture::new();
    let package = zip(vec![
        f("manifest.json", manifest("koyhq.ornek", "theme", "1.0.0", None)).deflated(),
        ZipFile::new("theme.json", THEME_JSON).deflated(),
        ZipFile::new("preview.png", png()).deflated(),
    ]);

    let result = fx.install(package);

    assert!(result.is_success(), "{:?}", result.error);
    assert_eq!(THEME_JSON, fs::read_to_string(fx.root.join("koyhq.ornek").join("theme.json")).unwrap());
    assert_eq!(png(), fs::read(fx.root.join("koyhq.ornek").join("preview.png")).unwrap());
}

#[test]
fn entry_that_inflates_beyond_its_declared_length_is_an_invalid_archive() {
    let fx = Fixture::new();
    // Başlık 5 bayt diyor, gerçek içerik çok daha uzun: zip bombası savunması.
    let long_theme = format!("{{ \"KoyBgWindow\": \"#112233\", \"x\": \"{}\" }}", "a".repeat(200));
    let package = zip(vec![
        f("manifest.json", manifest("koyhq.ornek", "theme", "1.0.0", None)),
        ZipFile::new("theme.json", long_theme).deflated().declared(5),
    ]);

    assert_eq!(ExtensionInstallError::InvalidArchive, fx.install(package).error);
    assert!(fx.leftover_temp().is_empty());
    assert!(!fx.root.join("koyhq.ornek").is_dir());
}

#[test]
fn corrupt_deflate_data_is_an_invalid_archive() {
    let fx = Fixture::new();
    let mut bytes = build_zip(&[
        f("manifest.json", manifest("koyhq.ornek", "theme", "1.0.0", None)),
        ZipFile::new("theme.json", THEME_JSON).deflated(),
    ]);
    // theme.json'un sıkıştırılmış verisinin ilk baytı: blok türü 3 (geçersiz).
    let marker = bytes.windows(10).position(|w| w == b"theme.json").unwrap() + 10;
    bytes[marker] = 0x07;

    assert_eq!(ExtensionInstallError::InvalidArchive, fx.install(Cursor::new(bytes)).error);
    assert!(fx.leftover_temp().is_empty());
}

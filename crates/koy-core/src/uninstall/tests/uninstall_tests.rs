//! Karşılığı: `tests/Core.Tests/UninstallTests.cs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::uninstall::{UninstallAvailability, UninstallCleaner, UninstallPlan};

fn combine(a: &str, b: &str) -> String {
    Path::new(a).join(b).to_string_lossy().into_owned()
}

#[test]
fn evaluate_kurulu_ve_kaldirici_varsa_available() {
    let r = UninstallPlan::evaluate(true, false, Some(r"C:\k\Update.exe"), &|_| true);
    assert_eq!(UninstallAvailability::Available, r);
}

#[test]
fn evaluate_kurulu_degilse_not_installed() {
    let r = UninstallPlan::evaluate(false, false, None, &|_| true);
    assert_eq!(UninstallAvailability::NotInstalled, r);
}

#[test]
fn evaluate_tasinabilirse_kurul_sanilmaz() {
    let r = UninstallPlan::evaluate(true, true, Some(r"C:\k\Update.exe"), &|_| true);
    assert_eq!(UninstallAvailability::Portable, r);
}

#[test]
fn evaluate_kaldirici_yolu_yoksa_updater_missing() {
    for path in [None, Some(""), Some("  ")] {
        let r = UninstallPlan::evaluate(true, false, path, &|_| true);
        assert_eq!(UninstallAvailability::UpdaterMissing, r, "{path:?}");
    }
}

#[test]
fn evaluate_kaldirici_dosyasi_yoksa_updater_missing() {
    let r = UninstallPlan::evaluate(true, false, Some(r"C:\k\Update.exe"), &|_| false);
    assert_eq!(UninstallAvailability::UpdaterMissing, r);
}

#[test]
fn files_to_delete_ayarlar_saklaniyorsa_ayar_dosyalari_listede_yok() {
    let files = UninstallPlan::files_to_delete("R", "L", true);

    assert!(!files.iter().any(|f| f.ends_with("settings.json")));
    assert!(!files.iter().any(|f| f.ends_with("shelf.json")));
    assert!(files.contains(&combine("R", "taskbar-state.json")));
    assert!(files.contains(&combine("L", "crash.log")));
}

#[test]
fn files_to_delete_ayarlar_silinecekse_ayar_ve_raf_listede() {
    let files = UninstallPlan::files_to_delete("R", "L", false);

    assert!(files.contains(&combine("R", "settings.json")));
    assert!(files.contains(&combine("R", "shelf.json")));
    assert!(files.contains(&combine("L", "crash.log.old")));
}

#[test]
fn files_to_delete_kaldirici_ve_uygulama_dosyalarini_icermez() {
    // Velopack kök klasörü (yerel) bütünüyle silinmez; yalnız bilinen tanı dosyaları gider.
    let files = UninstallPlan::files_to_delete("R", "L", false);
    assert!(!files.iter().any(|f| f.ends_with("Update.exe") || f.ends_with("Koy.exe")));
}

#[test]
fn windows_dan_kaldirmada_ayarlar_silinmez() {
    assert!(UninstallPlan::KEEP_SETTINGS_ON_EXTERNAL_UNINSTALL);
}

#[test]
fn run_ayarlar_saklaniyorsa_ayar_kalir_durum_gider() {
    let t = TempDirs::new();
    fs::write(t.roaming.join("settings.json"), "{}").unwrap();
    fs::write(t.roaming.join("taskbar-state.json"), "{}").unwrap();
    fs::write(t.local.join("crash.log"), "x").unwrap();

    let result = UninstallCleaner::run(&t.roaming_str(), &t.local_str(), true);

    assert!(t.roaming.join("settings.json").is_file());
    assert!(!t.roaming.join("taskbar-state.json").is_file());
    assert!(!t.local.join("crash.log").is_file());
    assert_eq!(2, result.deleted);
    assert!(result.failed.is_empty());
}

#[test]
fn run_ayarlar_silinecekse_bos_klasor_de_gider() {
    let t = TempDirs::new();
    fs::write(t.roaming.join("settings.json"), "{}").unwrap();
    fs::write(t.roaming.join("shelf.json"), "[]").unwrap();

    UninstallCleaner::run(&t.roaming_str(), &t.local_str(), false);

    assert!(!t.roaming.is_dir());
}

#[test]
fn run_klasorde_baska_dosya_varsa_klasore_dokunmaz() {
    let t = TempDirs::new();
    fs::write(t.roaming.join("settings.json"), "{}").unwrap();
    let other = t.roaming.join("baska.txt");
    fs::write(&other, "k").unwrap();

    UninstallCleaner::run(&t.roaming_str(), &t.local_str(), false);

    assert!(other.is_file());
}

#[cfg(windows)]
#[test]
fn run_kilitli_dosyada_istisna_firlatmaz_ve_basarisiz_olarak_bildirir() {
    use std::os::windows::fs::OpenOptionsExt;

    let t = TempDirs::new();
    let settings = t.roaming.join("settings.json");
    fs::write(&settings, "{}").unwrap();
    // FileShare.None: paylaşım kipi 0.
    let _lock = fs::OpenOptions::new().read(true).share_mode(0).open(&settings).unwrap();

    let result = UninstallCleaner::run(&t.roaming_str(), &t.local_str(), false);

    assert!(result.failed.contains(&settings.to_string_lossy().into_owned()));
}

#[test]
fn run_klasorler_yoksa_sorunsuz_gecer() {
    let result = UninstallCleaner::run(r"Z:\yok\r", r"Z:\yok\l", false);
    assert_eq!(0, result.deleted);
}

struct TempDirs {
    root: PathBuf,
    roaming: PathBuf,
    local: PathBuf,
}

impl TempDirs {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir()
            .join(format!("koy-uninstall-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst)));
        let roaming = root.join("roaming");
        let local = root.join("local");
        fs::create_dir_all(&roaming).unwrap();
        fs::create_dir_all(&local).unwrap();
        TempDirs { root, roaming, local }
    }

    fn roaming_str(&self) -> String {
        self.roaming.to_string_lossy().into_owned()
    }

    fn local_str(&self) -> String {
        self.local.to_string_lossy().into_owned()
    }
}

impl Drop for TempDirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

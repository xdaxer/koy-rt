//! Karşılığı: `koy/src/App/Features/Settings/AppUninstaller.cs`.
//!
//! Uygulama içinden kaldırma. Karar ve silme mantığı Core'da (`UninstallPlan`, `UninstallCleaner`); burada yalnız
//! kaldırıcının yerini bulmak, başlatmak ve Koy'u kapatmak var. C#'ta yer Velopack'ten (`VelopackLocator`) okunuyordu;
//! burada aynı kurulum düzeni doğrudan okunur: `<kök>\current\koy.exe` ve `<kök>\Update.exe`, taşınabilir sürümde
//! kökte `.portable` işareti.

use std::cell::Cell;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::settings::SettingsStore;
pub use koy_core::uninstall::UninstallAvailability;
use koy_core::uninstall::{UninstallCleaner, UninstallPlan};

thread_local! {
    static IN_PROGRESS: Cell<bool> = const { Cell::new(false) };
    static DELETE_SETTINGS: Cell<bool> = const { Cell::new(false) };
}

/// Kaldırma başladı: ayarlar artık yazılmamalı (kapanırken pencereler kaydetmeye çalışır).
pub fn in_progress() -> bool {
    IN_PROGRESS.with(Cell::get)
}

fn roaming_dir() -> String {
    SettingsStore::default_path().parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

fn local_dir() -> String {
    Path::new(&CrashLog::default_path()).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

struct Locator {
    is_installed: bool,
    is_portable: bool,
    update_exe: Option<PathBuf>,
}

fn locate() -> Locator {
    let exe = std::env::current_exe().ok();
    let current_dir = exe.as_deref().and_then(Path::parent);
    let root = current_dir
        .filter(|d| d.file_name().is_some_and(|n| n.eq_ignore_ascii_case("current")))
        .and_then(Path::parent);
    match root {
        Some(root) => Locator {
            is_installed: true,
            is_portable: root.join(".portable").exists(),
            update_exe: Some(root.join("Update.exe")),
        },
        None => Locator { is_installed: false, is_portable: false, update_exe: None },
    }
}

pub fn availability() -> UninstallAvailability {
    let l = locate();
    let update = l.update_exe.as_ref().map(|p| p.to_string_lossy().into_owned());
    UninstallPlan::evaluate(l.is_installed, l.is_portable, update.as_deref(), &|p: &str| Path::new(p).exists())
}

/// Sıra: görev çubuğunu geri getir, verileri sil, kaldırıcıyı (`Update.exe --uninstall`) başlat, Koy'u kapat
/// (`shutdown`). Kaldırıcı başlamazsa false döner ve Koy çalışmaya devam eder.
pub fn begin(keep_settings: bool, shutdown: impl FnOnce()) -> bool {
    if availability() != UninstallAvailability::Available {
        return false;
    }
    let Some(update_exe) = locate().update_exe else { return false };

    IN_PROGRESS.with(|c| c.set(true));
    DELETE_SETTINGS.with(|c| c.set(!keep_settings));

    koy_platform::shell::taskbar_service::restore(0, false);
    let clean = UninstallCleaner::run(&roaming_dir(), &local_dir(), keep_settings);
    if !clean.failed.is_empty() {
        CrashLog::write("Kaldırma: silinemeyen dosyalar", &CrashError::object(clean.failed.join("\n")));
    }

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let started = Command::new(&update_exe)
        .arg(UninstallPlan::UPDATER_ARGUMENTS)
        .current_dir(update_exe.parent().unwrap_or(Path::new(".")))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if let Err(e) = started {
        IN_PROGRESS.with(|c| c.set(false));
        CrashLog::write("Kaldırıcı başlatılamadı", &CrashError::object(e.to_string()));
        return false;
    }

    shutdown();
    true
}

/// Kapanışta bir kez daha: pencereler kapanırken yeniden yazılmış olabilecek ayar dosyalarını sil.
pub fn clean_after_exit() {
    if in_progress() {
        let delete = DELETE_SETTINGS.with(Cell::get);
        UninstallCleaner::run(&roaming_dir(), &local_dir(), !delete);
    }
}

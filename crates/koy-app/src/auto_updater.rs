//! Karşılığı: `koy/src/App/AutoUpdater.cs` ve `KoyWindowViewModel` güncelleme komutları (`CheckUpdatesAsync`,
//! `StartUpdateAsync`).
//!
//! "Güncellemeleri otomatik denetle" açıkken açılıştan 45 sn sonra bir kez denetler. Yeni sürüm bulunursa otomatik
//! indirilmez: kullanıcı onayı için durum kaydedilir (`UpdateStatus`). Kurulu değilse (geliştirme çalıştırması) hiçbir şey
//! yapılmaz.
//!
//! Sapma (paketleme kararı açık): C# Velopack'in `GithubSource` + `UpdateManager`'ını kullanıyordu. Burada aynı kaynak
//! (koyhq/koy GitHub sürümleri, Önizleme kanalında ön sürümler dahil) doğrudan okunur; güncelleme, sürümün kurulum
//! programı (`*Setup.exe`) indirilip çalıştırılarak ve Koy kapatılarak uygulanır (Velopack kurulumu mevcut kurulumun
//! üstüne kurar).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use koy_core::cancel::CancellationToken;
use koy_core::settings::{AppSettings, UpdateChannel};
use koy_core::updates::UpdateFlowPolicy;
use koy_platform::net::{HttpClient, Request};

use crate::infra::dispatch;
use crate::update_status::{AppInfo, UpdateCheckResult, UpdateInfo, UpdateStatus};

const RELEASES_URL: &str = "https://api.github.com/repos/koyhq/koy/releases?per_page=20";
const STARTUP_DELAY: Duration = Duration::from_secs(45);
const MAX_INSTALLER_BYTES: usize = 300 * 1024 * 1024;

static CHECKING: AtomicBool = AtomicBool::new(false);

/// Bulunan sürüm ve notları (besleme notları "What's new" penceresine gider).
#[derive(Clone, Debug)]
pub struct FoundRelease {
    pub info: UpdateInfo,
    pub notes_markdown: Option<String>,
}

thread_local! {
    static LAST_NOTES: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Son bulunan sürümün notları (güncelleme penceresi için).
pub fn available_notes() -> Option<String> {
    LAST_NOTES.with(|n| n.borrow().clone())
}

/// Kurulu sürüm mü (Velopack düzeni: `<kök>\current\koy.exe` ve `<kök>\Update.exe`).
pub fn is_installed() -> bool {
    crate::features::settings::app_uninstaller::availability()
        != crate::features::settings::app_uninstaller::UninstallAvailability::NotInstalled
}

/// GitHub sürüm listesinden kanala uyan en yeni sürüm; yeni değilse `None`. Ağ hatasında `Err`.
fn find_newer(preview: bool) -> Result<Option<FoundRelease>, ()> {
    let request = Request::get(RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Koy-Updater");
    let mut response = HttpClient::shared().send(&request, &CancellationToken::none()).map_err(|_| ())?;
    if !response.is_success() {
        return Err(());
    }
    let text = response.text().map_err(|_| ())?;
    let releases: serde_json::Value = serde_json::from_str(&text).map_err(|_| ())?;
    let current = AppInfo::version();
    let Some(list) = releases.as_array() else { return Err(()) };
    for r in list {
        if r["draft"].as_bool().unwrap_or(false) {
            continue;
        }
        if r["prerelease"].as_bool().unwrap_or(false) && !preview {
            continue;
        }
        let tag = r["tag_name"].as_str().unwrap_or("").trim_start_matches(['v', 'V']).to_string();
        if tag.is_empty() {
            continue;
        }
        // Liste en yeniden eskiye: kanala uyan ilk sürüm en yenisidir.
        if !UpdateFlowPolicy::is_newer_version(current, &tag) {
            return Ok(None);
        }
        let asset = r["assets"].as_array().and_then(|a| {
            a.iter()
                .find(|x| x["name"].as_str().is_some_and(|n| n.to_ascii_lowercase().ends_with("setup.exe")))
                .and_then(|x| x["browser_download_url"].as_str())
        });
        let Some(url) = asset.filter(|u| u.starts_with("https://")) else { return Ok(None) };
        return Ok(Some(FoundRelease {
            info: UpdateInfo { version: tag, download_url: url.to_string() },
            notes_markdown: r["body"].as_str().map(str::to_string),
        }));
    }
    Ok(None)
}

/// Denetim (arka planda); sonuç arayüz iş parçacığında `UpdateStatus`'a yazılır. `on_done` arayüzde çağrılır.
pub fn check(channel: UpdateChannel, on_done: impl FnOnce() + Send + 'static) {
    if !is_installed() {
        // Geliştirme çalıştırmasında denetim yapılamaz; başarı gibi gösterilmez.
        UpdateStatus::record(UpdateCheckResult::NotInstalled);
        on_done();
        return;
    }
    if CHECKING.swap(true, Ordering::SeqCst) {
        return;
    }
    let preview = channel == UpdateChannel::Preview;
    std::thread::spawn(move || {
        let result = find_newer(preview);
        CHECKING.store(false, Ordering::SeqCst);
        dispatch::post(move || {
            match result {
                Ok(Some(found)) => {
                    LAST_NOTES.with(|n| *n.borrow_mut() = found.notes_markdown.clone());
                    UpdateStatus::record_available(found.info);
                }
                Ok(None) => UpdateStatus::record(UpdateCheckResult::UpToDate),
                // Ağ yoksa ya da GitHub yanıt vermiyorsa sessizce vazgeç.
                Err(()) => UpdateStatus::record(UpdateCheckResult::Failed),
            }
            on_done();
        });
    });
}

pub fn is_checking() -> bool {
    CHECKING.load(Ordering::SeqCst)
}

/// Açılışta: ayar açıksa 45 sn sonra bir kez (açılış yükü kullanıcı çentik ve dock'u kullanmaya başladıktan sonra).
pub fn run_on_startup(settings: &AppSettings) {
    if !settings.general.auto_check_updates {
        return;
    }
    let channel = settings.general.update_channel;
    slint::Timer::single_shot(STARTUP_DELAY, move || check(channel, || {}));
}

/// Kurulum programını indirir (ilerleme 0..1 arayüzde), görev çubuğunu geri getirir, kurulumu başlatır ve `exit` çağrılır.
/// Hata olursa `on_failed` çağrılır.
pub fn start_update(
    progress: impl Fn(f64) + Send + Sync + 'static,
    on_failed: impl FnOnce() + Send + 'static,
    exit: impl FnOnce() + Send + 'static,
) {
    let Some(update) = UpdateStatus::available_update() else { return };
    let progress = Arc::new(progress);
    std::thread::spawn(move || {
        let download = || -> Option<std::path::PathBuf> {
            let request = Request::get(&update.download_url).header("User-Agent", "Koy-Updater");
            let mut response = HttpClient::shared().send(&request, &CancellationToken::none()).ok()?;
            if !response.is_success() {
                return None;
            }
            let total = response.content_length().unwrap_or(0) as f64;
            let p = Arc::clone(&progress);
            let mut report = move |n: usize| {
                if total > 0.0 {
                    let v = (n as f64 / total).clamp(0.0, 1.0);
                    let p = Arc::clone(&p);
                    dispatch::post(move || p(v));
                }
            };
            let bytes = response.read_to_end_limited(Some(MAX_INSTALLER_BYTES), None, Some(&mut report)).ok()?;
            let path = std::env::temp_dir().join(format!("KoySetup-{}.exe", update.version));
            std::fs::write(&path, bytes).ok()?;
            Some(path)
        };
        match download() {
            Some(path) => dispatch::post(move || {
                koy_platform::shell::taskbar_service::restore(0, false);
                if std::process::Command::new(&path).arg("--silent").spawn().is_ok() {
                    UpdateStatus::record(UpdateCheckResult::UpdateReady);
                    exit();
                } else {
                    on_failed();
                }
            }),
            None => dispatch::post(on_failed),
        }
    });
}

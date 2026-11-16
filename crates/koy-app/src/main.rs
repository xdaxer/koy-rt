//! Koy (Rust). Karşılığı: `koy/src/App/` (WPF) — arayüz Slint ile.

#![windows_subsystem = "windows"]

mod features;
mod infra;
mod resources;
mod shared_services;
mod update_status;

use koy_core::settings::SettingsStore;
use resources::theme_manager::ThemeManager;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let settings = SettingsStore::new(SettingsStore::default_path()).try_load().unwrap_or_default();
    resources::strings::Strings::set_ui_culture(&settings.general.language);
    ThemeManager::apply(&settings);

    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let view = args.get(i + 1).cloned().unwrap_or_default();
        let path = args.get(i + 2).cloned().unwrap_or_else(|| "snapshot.png".into());
        return dev_snapshot(&view, path);
    }

    let probe = Probe::new()?;
    ThemeManager::install(&probe);
    if args.iter().any(|a| a == "--probe") {
        let _t = infra::dispatch::delay(std::time::Duration::from_millis(800), || {
            let _ = slint::quit_event_loop();
        });
        probe.run()?;
        return Ok(());
    }
    probe.run()
}

/// Geliştirici yardımcısı: adı verilen görünümü çizip PNG'ye kaydeder ve çıkar.
fn dev_snapshot(view: &str, path: String) -> Result<(), slint::PlatformError> {
    use slint::ComponentHandle;
    let save_after = |w: slint::Weak<SagAltMenuWindow>, path: String| {
        infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
            if let Some(v) = w.upgrade() {
                infra::snapshot::save(v.window(), &path);
            }
            let _ = slint::quit_event_loop();
        })
    };
    match view {
        "menu" => {
            let v = SagAltMenuWindow::new()?;
            ThemeManager::install(&v);
            v.global::<MenuText>().set_nav_notch(resources::strings::Strings::get("NavNotch").into());
            v.global::<MenuText>().set_nav_dock(resources::strings::Strings::get("NavDock").into());
            v.global::<MenuText>().set_on(resources::strings::Strings::get("On").into());
            v.global::<MenuText>().set_off(resources::strings::Strings::get("Off").into());
            v.global::<MenuText>().set_open_koy_window(resources::strings::Strings::get("OpenKoyWindowAction").into());
            v.set_monitor_summary(resources::strings::Strings::format("MonitorSummary", &["2"]).into());
            v.set_notch_enabled(true);
            v.set_pause_text(resources::strings::Strings::get("PauseOneHourAction").into());
            v.set_exit_text(resources::strings::Strings::get("ExitKoyAction").into());
            v.set_has_available_update(true);
            v.set_available_version_text(resources::strings::Strings::format("UpdateAvailableBadge", &["0.2.13"]).into());
            let _t = save_after(v.as_weak(), path);
            v.run()
        }
        _ => {
            let p = Probe::new()?;
            ThemeManager::install(&p);
            let w = p.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            p.run()
        }
    }
}

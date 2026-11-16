//! Koy (Rust). Karşılığı: `koy/src/App/` (WPF) — arayüz Slint ile.

#![windows_subsystem = "windows"]

mod features;
mod infra;
mod resources;
mod shared_services;

use koy_core::settings::SettingsStore;
use resources::theme_manager::ThemeManager;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let settings = SettingsStore::new(SettingsStore::default_path()).try_load().unwrap_or_default();
    resources::strings::Strings::set_ui_culture(&settings.general.language);
    ThemeManager::apply(&settings);

    let probe = Probe::new()?;
    ThemeManager::install(&probe);
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let path = args.get(i + 1).cloned().unwrap_or_else(|| "snapshot.png".into());
        let weak = probe.as_weak();
        let _t = infra::dispatch::delay(std::time::Duration::from_millis(600), move || {
            if let Some(p) = weak.upgrade() {
                infra::snapshot::save(p.window(), &path);
            }
            let _ = slint::quit_event_loop();
        });
        probe.run()?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--probe") {
        let _t = infra::dispatch::delay(std::time::Duration::from_millis(800), || {
            let _ = slint::quit_event_loop();
        });
        probe.run()?;
        return Ok(());
    }
    probe.run()
}

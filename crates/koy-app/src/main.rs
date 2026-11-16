//! Koy (Rust). Karşılığı: `koy/src/App/` (WPF) — arayüz Slint ile.

#![windows_subsystem = "windows"]

mod features;
mod infra;
mod shared_services;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let probe = Probe::new()?;
    if std::env::args().any(|a| a == "--probe") {
        let _t = infra::dispatch::delay(std::time::Duration::from_millis(500), || {
            let _ = slint::quit_event_loop();
        });
        probe.run()?;
        return Ok(());
    }
    probe.run()
}

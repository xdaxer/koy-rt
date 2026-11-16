slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let probe = Probe::new()?;
    if std::env::args().any(|a| a == "--probe") {
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(500), || {
            let _ = slint::quit_event_loop();
        });
        probe.run()?;
        return Ok(());
    }
    probe.run()
}

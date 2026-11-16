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
        "focus" | "stopwatch" | "timer" => {
            use features::focus::{focus_binding, focus_view_model::FocusViewModel, tasks_view_model::{TaskRow, TasksState}};
            let v = NotchPanelPreview::new()?;
            ThemeManager::install(&v);
            focus_binding::install_text(&v);
            let timers = std::sync::Arc::new(koy_core::timers::FocusTimers::new(None, None, None));
            let vm = FocusViewModel::new(timers, &koy_core::settings::AppSettings::default(), None);
            match view {
                "stopwatch" => vm.select_tab(koy_core::timers::FocusKind::Stopwatch),
                "timer" => vm.select_tab(koy_core::timers::FocusKind::Countdown),
                _ => {}
            }
            let row = |t: &str, done: bool, time: &str, next: bool| TaskRow {
                id: Default::default(),
                title: t.into(),
                done,
                is_next: next,
                is_now: next,
                time_text: time.into(),
                day_text: String::new(),
            };
            let tasks = TasksState {
                rows: vec![row("Rapor taslağı", false, "", true), row("Toplantı", false, "16:00", false), row("E-posta", true, "", false)],
                title: resources::strings::Strings::get("TaskToday"),
                ..Default::default()
            };
            v.set_panel("focus".into());
            v.set_fdata(focus_binding::to_focus_data(&vm.state(), &vm.presets));
            v.set_tasks(focus_binding::to_tasks_data(&tasks));
            let w = v.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            v.run()
        }
        "system" => {
            let v = NotchPanelPreview::new()?;
            ThemeManager::install(&v);
            v.global::<SystemText>().set_processor(resources::strings::Strings::get("SystemProcessor").into());
            v.global::<SystemText>().set_memory(resources::strings::Strings::get("SystemMemory").into());
            v.global::<SystemText>().set_koy_self(resources::strings::Strings::get("SystemKoySelf").into());
            v.set_panel("system".into());
            v.set_sys(SystemStatsData {
                cpu_progress: 0.37,
                cpu_text: "%37".into(),
                top_process_text: "En çok: chrome".into(),
                ram_progress: 0.62,
                ram_text: "%62".into(),
                memory_detail_text: "9,9 / 16 GB".into(),
                network_name: "Ev Wi-Fi".into(),
                download_text: "↓ 1,2 MB/sn".into(),
                upload_text: "↑ 84 KB/sn".into(),
                koy_memory_text: "41 MB".into(),
            });
            let w = v.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
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

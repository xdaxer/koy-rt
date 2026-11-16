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
    // Arayüz iş parçacığı tek iş parçacıklı COM (OLE) olur: winit'in sürükle-bırak desteği bunu ister. Servislerin aynı
    // iş parçacığında sonradan istediği çok iş parçacıklı kip `RPC_E_CHANGED_MODE` ile reddedilir ve onlar bunu tolere eder.
    // SAFETY: süreç başında bir kez; eşleşen OleUninitialize süreç sonunda gerekmez.
    unsafe {
        let _ = windows::Win32::System::Ole::OleInitialize(None);
    }
    let settings = SettingsStore::new(SettingsStore::default_path()).try_load().unwrap_or_default();
    resources::strings::Strings::set_ui_culture(&settings.general.language);
    ThemeManager::apply(&settings);

    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let view = args.get(i + 1).cloned().unwrap_or_default();
        let path = args.get(i + 2).cloned().unwrap_or_else(|| "snapshot.png".into());
        return dev_snapshot(&view, path);
    }

    // Geliştirici: yalnız çentik özelliğini çalıştırır (`--quit-after <sn>` verilirse o kadar sonra çıkar).
    if args.iter().any(|a| a == "--notch") {
        use features::Feature;
        let shared = std::rc::Rc::new(std::cell::RefCell::new(settings));
        let services = std::rc::Rc::new(shared_services::SharedServices::default());
        let notch = features::notch::notch_feature::NotchFeature::new(shared, services);
        notch.start();
        let _quit = args.iter().position(|a| a == "--quit-after").and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u64>().ok()).map(|secs| {
            infra::dispatch::delay(std::time::Duration::from_secs(secs), || {
                let _ = slint::quit_event_loop();
            })
        });
        slint::run_event_loop_until_quit()?;
        notch.stop();
        return Ok(());
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
        "notch" | "notch-music" | "notch-peek" | "notch-ring" | "osd-volume" | "osd-card" => {
            use features::notch::notch_shape::{ShapeSpec, path_commands};
            let v = NotchShapePreview::new()?;
            ThemeManager::install(&v);
            let (w, h, r) = match view {
                "osd-volume" => (260.0, 40.0, 20.0),
                "osd-card" => (380.0, 72.0, 28.0),
                "notch-music" => (280.0, 34.0, 18.0),
                "notch-peek" => (300.0, 38.0, 20.0),
                _ => (236.0, 34.0, 18.0),
            };
            let mut d = NotchData::default();
            d.surface = slint::Color::from_argb_encoded(0xFF0A_0A0B);
            d.state = if view == "notch-peek" { 1 } else { 0 };
            d.show_collapsed_layout = view != "notch-peek";
            d.show_peek_layout = view == "notch-peek";
            d.clock_time = "14:32".into();
            d.show_idle_classic_clock = view != "notch-ring";
            d.show_idle_battery = view == "notch";
            d.has_battery = true;
            d.battery_fill_width = 11.0;
            d.show_idle_radio = view == "notch";
            d.wifi_present = true;
            d.wifi_on = true;
            d.bluetooth_present = true;
            d.bluetooth_on = false;
            d.is_idle_ring = view == "notch-ring";
            d.show_idle_ring = view == "notch-ring";
            d.battery_ring_path = koy_core::notch::IdleDisplayFormat::arc_path(64, 18.0, 7.25).into();
            d.battery_ring_color = slint::Color::from_argb_encoded(0xFF4A_DE80);
            d.battery_percent_text = "%64".into();
            d.idle_day_time = "Pzt 14:32".into();
            d.is_music_playing = view == "notch-music";
            d.music_title = "Bohemian Rhapsody".into();
            d.music_artist = "Queen".into();
            d.show_music_waveform = true;
            d.waveform_bars = slint::ModelRc::new(slint::VecModel::from(vec![6.0f32, 12.0, 14.0, 10.0, 5.0]));
            d.waveform_color = slint::Color::from_argb_encoded(0xFF4A_DE80);
            if view.starts_with("osd") {
                let osd = features::notch::osd::osd_indicator_view_model::OsdIndicatorViewModel::new();
                if view == "osd-volume" {
                    osd.show_volume(0.62, false);
                } else {
                    let mut card = koy_core::notch::NotchCard::new(koy_core::notch::NotchCardKind::Update, "Koy 0.2.13 hazır", "Yeniden başlatınca kurulur");
                    card.primary_action_label = "Güncelle".into();
                    osd.show_card(card, std::time::Duration::from_secs(30));
                }
                d.osd_active = true;
                v.set_osd(features::notch::osd::osd_binding::to_data(&osd.state()));
            }
            v.set_d(d);
            v.set_shape_width((w as f32) * 1.0);
            v.set_shape_height(h as f32);
            let cmd = path_commands(&ShapeSpec { left: (640.0 - w) / 2.0, top: 0.0, width: w, height: h, corner_radius: r, ear_radius: 12.0 });
            v.set_shape_path(cmd.into());
            let w = v.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            v.run()
        }
        v if v.starts_with("settings") => {
            // "settings:SAYFA:ALTSEKME" (ör. settings:2:1). Ayarlar geçici bir dosyadan okunur, diske yazılmaz.
            let parts: Vec<i32> = v.split(':').skip(1).filter_map(|p| p.parse().ok()).collect();
            let store = koy_core::settings::SettingsStore::new(std::env::temp_dir().join("koy-snapshot-settings.json"));
            let vm = features::settings::settings_view_model::SettingsViewModel::new(store);
            ThemeManager::apply(&vm.settings());
            let host = features::settings::koy_window::KoyWindowHost::new(vm.clone())?;
            vm.select_page(parts.first().copied().unwrap_or(0));
            if let Some(sub) = parts.get(1) {
                vm.select_sub_tab(parts[0], *sub);
            }
            let w = host.window().as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(900), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            host.window().run()
        }
        "dock" | "dock-full" => {
            use slint::{ModelRc, VecModel};
            let v = DockWindow::new()?;
            ThemeManager::install(&v);
            features::dock::dock_binding::install_text(&v);
            let full = view == "dock-full";
            let mut d = DockData::default();
            d.full_width = full;
            d.item_size = 38.0;
            d.slot_spacing = 2.0;
            d.capsule_surface = slint::Color::from_argb_encoded(0xFF18_181B);
            d.full_surface = slint::Color::from_argb_encoded(0xFF12_1216);
            d.dock_height = 60.0;
            d.margin_left = 12.0;
            d.margin_top = 12.0;
            d.margin_right = 12.0;
            d.margin_bottom = 10.0;
            d.radius_tl = 18.0;
            d.radius_tr = 18.0;
            d.radius_br = 18.0;
            d.radius_bl = 18.0;
            d.full_border_top = 1.0;
            d.opacity = 1.0;
            d.is_visible = true;
            d.show_start = true;
            d.show_folders = true;
            d.show_recents = true;
            d.show_full_tray = true;
            d.show_right_section = true;
            d.small_tray_width = 18.0;
            d.items_max_length = 100000.0;
            d.has_now_playing = full;
            d.now_playing_title = "Bohemian Rhapsody".into();
            d.now_playing_artist = "Queen".into();
            let mut p = DockPresentationData::default();
            p.draws_mark = true;
            let entry = |kind: i32, title: &str, running: bool, active: bool, badge: &str, extra: i32| DockEntryData {
                kind,
                title: title.into(),
                icon_size: 28.0,
                item_size: 38.0,
                scale: 1.0,
                press_scale: 1.0,
                press_opacity: 1.0,
                running,
                active,
                indicator_count: if running { 1 } else { 0 },
                show_badge: !badge.is_empty(),
                badge_text: badge.into(),
                extra_kind: extra,
                glyph: "\u{E74D}".into(),
                time_text: "14:32".into(),
                date_text: "16.11.2026".into(),
                ..Default::default()
            };
            let e = vec![
                entry(0, "Explorer", true, true, "", 0),
                entry(0, "Firefox", true, false, "3", 0),
                entry(0, "Terminal", false, false, "", 0),
                entry(1, "Klasör", true, false, "", 0),
                entry(2, "", false, false, "", 0),
                entry(2, "Saat", false, false, "", 2),
                entry(2, "Çöp", false, false, "", 3),
            ];
            v.set_d(d);
            v.set_p(p);
            v.set_entries(ModelRc::new(VecModel::from(e)));
            v.set_bar_width(1100.0);
            v.set_bar_height(48.0);
            let w = v.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            v.run()
        }
        "search" => {
            use slint::{ModelRc, VecModel};
            let v = NotchPanelPreview::new()?;
            ThemeManager::install(&v);
            features::search::search_binding::install_text(&v, features::search::search_factory::glyph_path(koy_core::search::SearchGlyph::Help).unwrap_or(""));
            v.set_preview_height(324.0);
            let g = |gl| features::search::search_factory::glyph_path(gl).unwrap_or("").into();
            let row = |i: i32, header: &str, title: &str, sub: &str, glyph, selected: bool| SearchRowData {
                index: i,
                header: header.into(),
                title: title.into(),
                subtitle: sub.into(),
                glyph,
                has_icon: false,
                icon: Default::default(),
                selected,
                actions: ModelRc::new(VecModel::from(if selected { vec![SearchActionData { label: "Aç".into(), active: true }, SearchActionData { label: "Klasörü aç".into(), active: false }] } else { vec![] })),
            };
            let d = SearchData {
                query: "12*8".into(),
                suggestion: "".into(),
                rows: ModelRc::new(VecModel::from(vec![
                    row(0, "Hesap makinesi", "= 96", "Kopyalamak için Enter", g(koy_core::search::SearchGlyph::Calculator), true),
                    row(1, "Dosyalar", "rapor-12-8.pdf", r"C:\Belgeler\rapor-12-8.pdf", g(koy_core::search::SearchGlyph::File), false),
                    row(2, "Web", "12*8 için ara", "DuckDuckGo", g(koy_core::search::SearchGlyph::Globe), false),
                ])),
                scroll_to: -1,
                ..Default::default()
            };
            v.set_panel("search".into());
            v.set_search(d);
            let w = v.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            v.run()
        }
        "control" | "wifi" | "bt" => {
            use slint::{ModelRc, VecModel};
            let v = NotchPanelPreview::new()?;
            ThemeManager::install(&v);
            features::control_center::control_center_binding::install_text(&v);
            v.set_preview_height(324.0);
            let mut c = ControlData::default();
            c.page = match view { "wifi" => 1, "bt" => 2, _ => 0 };
            c.tile_order = ModelRc::new(VecModel::from(vec!["wifi".into(), "bluetooth".into(), "donotdisturb".into()]));
            c.wifi = RadioTileData { on: true, unavailable: false, status: "Açık".into() };
            c.bluetooth = RadioTileData { on: false, unavailable: false, status: "Kapalı".into() };
            c.dnd_status = "Ayarlarda aç".into();
            c.volume_percent = 46;
            c.brightness = 70;
            c.brightness_supported = true;
            c.brightness_resolved = true;
            c.show_connected_card = true;
            c.connected = ModelRc::new(VecModel::from(vec![ConnectedDeviceData { name: "AirPods Pro".into(), battery: "%80".into() }]));
            c.wifi_show_list = true;
            c.wifi_rows = ModelRc::new(VecModel::from(vec![
                WifiRowData { ssid: "Ev".into(), name: "Ev Wi-Fi".into(), bars: 4, secured: true, connected: true, has_status: true, status: "Bağlı".into(), clickable: false, ..Default::default() },
                WifiRowData { ssid: "Kafe".into(), name: "Kafe".into(), bars: 2, secured: true, password_open: true, clickable: true, ..Default::default() },
                WifiRowData { ssid: "Misafir".into(), name: "Misafir".into(), bars: 1, clickable: true, ..Default::default() },
            ]));
            c.bt_show_list = true;
            c.bt_rows = ModelRc::new(VecModel::from(vec![
                BluetoothRowData { id: "1".into(), name: "AirPods Pro".into(), kind: 1, connected: true, is_audio: true, battery: "%80".into(), status: "Bağlı".into(), action: "Bağlantıyı kes".into(), ..Default::default() },
                BluetoothRowData { id: "2".into(), name: "MX Master".into(), kind: 3, status: "Eşleşmiş".into(), hint: "Windows ayarlarında aç".into(), ..Default::default() },
            ]));
            v.set_panel("control".into());
            v.set_control(c);
            let w = v.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            v.run()
        }
        "home" | "music" => {
            use slint::{ModelRc, VecModel};
            let v = NotchPanelPreview::new()?;
            ThemeManager::install(&v);
            features::home::home_binding::install_text(&v);
            v.set_preview_height(324.0);
            let mut h = HomeData::default();
            h.is_music_page = view == "music";
            h.greeting = "İyi günler".into();
            h.day_date = "Pazartesi, 16 Kasım".into();
            h.show_weather = true;
            h.weather_icon = "PartlyCloudy".into();
            h.temperature_short = "14°".into();
            h.weather_description = "Parçalı bulutlu".into();
            h.place_name = "Ankara".into();
            h.has_next_event = true;
            h.next_event = UpcomingData { index: 0, when: "Bugün 16:00".into(), title: "Ekip toplantısı".into(), until: "1 sa 28 dk sonra".into() };
            h.later_events = ModelRc::new(VecModel::from(vec![UpcomingData { index: 1, when: "Yarın 10:00".into(), title: "Diş hekimi".into(), until: "".into() }]));
            h.show_focus_row = true;
            h.focus_time_text = "25:00".into();
            h.right_cards = ModelRc::new(VecModel::from(vec![2, 3]));
            h.cpu_text = "%18".into();
            h.top_process_text = "En çok: chrome".into();
            h.cpu_progress = 0.18;
            h.ram_text = "%61".into();
            h.memory_detail_text = "9,8 / 16 GB".into();
            h.ram_progress = 0.61;
            h.network_name = "Ev Wi-Fi".into();
            h.download_text = "↓ 1,2 MB/sn".into();
            h.upload_text = "↑ 84 KB/sn".into();
            h.quick_items = ModelRc::new(VecModel::from(vec![
                QuickItemData { index: 0, name: "Notepad".into(), initials: "No".into(), has_icon: false, icon: Default::default() },
                QuickItemData { index: 1, name: "Terminal".into(), initials: "Te".into(), has_icon: false, icon: Default::default() },
            ]));
            h.quick_can_add = true;
            let mut m = MusicCardData::default();
            m.has_track = true;
            m.title = "Bohemian Rhapsody".into();
            m.artist = "Queen".into();
            m.source_app = "Spotify".into();
            m.is_playing = true;
            m.progress = 0.4;
            m.position_text = "2:23".into();
            m.duration_text = "5:55".into();
            m.show_spectrum = true;
            m.bar_count = 24;
            m.levels = ModelRc::new(VecModel::from((0..24).map(|i| ((i * 37 % 100) as f32) / 100.0).collect::<Vec<f32>>()));
            v.set_panel("home".into());
            v.set_home(h);
            v.set_music(m);
            let w = v.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            v.run()
        }
        "calendar" | "calweek" => {
            use features::calendar::{calendar_binding, calendar_view_model::CalendarViewModel, shared_events::SharedEvents};
            let v = NotchPanelPreview::new()?;
            ThemeManager::install(&v);
            calendar_binding::install_text(&v);
            let vm = CalendarViewModel::new(&koy_core::settings::AppSettings::default(), SharedEvents::new());
            vm.set_active(true);
            if view == "calweek" {
                vm.set_view(koy_core::calendar::CalendarViewMode::Week);
            }
            v.set_panel("calendar".into());
            v.set_cal(calendar_binding::to_data(&vm.state(), &vm.weekday_headers));
            let w = v.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            v.run()
        }
        "clipboard" => {
            use features::clipboard_panel::{clipboard_binding, clipboard_view_model::{ClipboardCard, ClipboardState, ClipboardViewModel}};
            let _ = ClipboardViewModel::new;
            let v = NotchPanelPreview::new()?;
            ThemeManager::install(&v);
            clipboard_binding::install_text(&v);
            let now = chrono::Local::now().fixed_offset();
            let item = |text: &str, kind, pinned, mins: i64| koy_core::clipboard::ClipboardItem {
                id: Default::default(),
                text: text.into(),
                kind,
                pinned,
                copied_at: now - chrono::Duration::minutes(mins),
            };
            use koy_core::clipboard::ClipboardKind as K;
            let mut cards: Vec<ClipboardCard> = [
                item("https://github.com/xdaxer/koy-rt", K::Link, true, 0),
                item("#3B82F6", K::Color, false, 2),
                item("fn main() {\n    println!(\"merhaba\");\n}", K::Code, false, 75),
                item("Toplantı saat 16:00'da, ikinci kattaki odada.", K::Text, false, 3000),
            ]
            .iter()
            .map(|i| ClipboardCard::new(i, now))
            .collect();
            cards[1].is_copied = true;
            let mut st = ClipboardState { items: cards, ..Default::default() };
            for (f, k) in [(koy_core::clipboard::ClipboardFilter::All, "ClipFilterAll"), (koy_core::clipboard::ClipboardFilter::Pinned, "ClipFilterPinned"), (koy_core::clipboard::ClipboardFilter::Link, "ClipFilterLink"), (koy_core::clipboard::ClipboardFilter::Color, "ClipFilterColor"), (koy_core::clipboard::ClipboardFilter::Code, "ClipFilterCode")] {
                st.filters.push(features::clipboard_panel::clipboard_view_model::ClipboardFilterChip { filter: f, label: resources::strings::Strings::get(k), is_selected: k == "ClipFilterAll" });
            }
            v.set_panel("clipboard".into());
            v.set_clip_filters(clipboard_binding::filters(&st));
            v.set_clip_items(clipboard_binding::cards(&st));
            let w = v.as_weak();
            let _t = infra::dispatch::delay(std::time::Duration::from_millis(700), move || {
                if let Some(v) = w.upgrade() {
                    infra::snapshot::save(v.window(), &path);
                }
                let _ = slint::quit_event_loop();
            });
            v.run()
        }
        "shelf" => {
            use features::shelf::shelf_binding;
            use koy_core::shelf::{ShelfItem, ShelfItemKind};
            let v = NotchPanelPreview::new()?;
            ThemeManager::install(&v);
            shelf_binding::install_text(&v);
            let now = chrono::Utc::now().fixed_offset();
            let items = vec![
                ShelfItem::new("1", "rapor.pdf", r"C:\x\rapor.pdf", ShelfItemKind::File, now),
                ShelfItem::new("2", "Fotoğraflar", r"C:\x\Fotoğraflar", ShelfItemKind::Directory, now),
                ShelfItem::new("3", "Toplantı notu", "Toplantı notu", ShelfItemKind::Text, now),
            ];
            v.set_panel("shelf".into());
            v.set_shelf_items(shelf_binding::to_items(&items));
            v.set_shelf_count(resources::strings::Strings::format("ShelfCount", &["3"]).into());
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

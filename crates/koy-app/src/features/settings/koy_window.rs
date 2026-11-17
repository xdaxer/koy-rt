//! Karşılığı: `koy/src/App/Features/Settings/KoyWindow.xaml.cs`.
//!
//! Koy (ayarlar) penceresi: çerçevesiz, kapatınca gizlenir (Koy kapanmaz). Görünürken settings.json izlenir; dışarıdan
//! yapılan değişiklik pencereye yansır. Görünüm modeline `Tr` (metinler), `S` (anahtarlı ayar satırları), `SL`
//! (listeler) ve `KoyWindowData` (sayfa, arama) küreselleriyle bağlanır.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::infra::native_window;
use crate::resources::strings::Strings;
use crate::resources::theme_manager::ThemeManager;
use crate::resources::user_style_watcher::UserStyleWatcher;
use crate::{DP, KoyWindow, KoyWindowData, MonitorCardData, OrderItem, S, SC, SL, SearchResultData, ShortcutRowData, ToggleItem, Tr};

use super::displays_view_model::DisplaysViewModel;
use super::settings_view_model::SettingsViewModel;
use super::shortcuts_view_model::ShortcutsViewModel;

thread_local! {
    static CURRENT: RefCell<Option<std::rc::Weak<KoyWindowHost>>> = const { RefCell::new(None) };
}

pub struct KoyWindowHost {
    window: KoyWindow,
    vm: Rc<SettingsViewModel>,
    shortcuts: Rc<ShortcutsViewModel>,
    displays: Rc<DisplaysViewModel>,
    watcher: RefCell<Option<UserStyleWatcher>>,
    revert_timer: slint::Timer,
    highlight_timer: slint::Timer,
}

fn ss(s: impl AsRef<str>) -> SharedString {
    SharedString::from(s.as_ref())
}

impl KoyWindowHost {
    pub fn new(vm: Rc<SettingsViewModel>) -> Result<Rc<Self>, slint::PlatformError> {
        let window = KoyWindow::new()?;
        ThemeManager::install(&window);
        let shortcuts = ShortcutsViewModel::new(Rc::clone(&vm));
        let displays = DisplaysViewModel::new(Rc::clone(&vm));
        let host = Rc::new(KoyWindowHost {
            window,
            vm,
            shortcuts,
            displays,
            watcher: RefCell::new(None),
            revert_timer: slint::Timer::default(),
            highlight_timer: slint::Timer::default(),
        });
        host.bind();
        host.sync();
        CURRENT.with(|c| *c.borrow_mut() = Some(Rc::downgrade(&host)));
        let weak = Rc::downgrade(&host);
        host.vm.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync();
            }
        });
        let weak = Rc::downgrade(&host);
        host.shortcuts.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_shortcuts();
            }
        });
        let weak = Rc::downgrade(&host);
        host.displays.changed.subscribe(move || {
            if let Some(h) = weak.upgrade() {
                h.sync_displays();
            }
        });
        host.sync_shortcuts();
        Ok(host)
    }

    pub fn window(&self) -> &KoyWindow {
        &self.window
    }

    pub fn view_model(&self) -> &Rc<SettingsViewModel> {
        &self.vm
    }

    pub fn shortcuts(&self) -> &Rc<ShortcutsViewModel> {
        &self.shortcuts
    }

    fn sync_shortcuts(&self) {
        let rows: Vec<ShortcutRowData> = self
            .shortcuts
            .rows()
            .into_iter()
            .map(|r| ShortcutRowData {
                title: ss(&r.title),
                tokens: ModelRc::new(VecModel::from(r.key_tokens().into_iter().map(SharedString::from).collect::<Vec<_>>())),
                capturing: r.is_capturing,
                error: ss(r.error_text.clone().unwrap_or_default()),
                is_default: r.is_default(),
                has_shortcut: r.has_shortcut(),
            })
            .collect();
        self.window.global::<SC>().set_rows(ModelRc::new(VecModel::from(rows)));
    }

    fn sync_displays(&self) {
        use koy_core::display::NewScreenMode;
        let dp = self.window.global::<DP>();
        let cards: Vec<MonitorCardData> = self
            .displays
            .cards()
            .into_iter()
            .map(|c| MonitorCardData {
                number: c.number,
                left: c.left as f32,
                top: c.top as f32,
                width: c.width as f32,
                height: c.height as f32,
                selected: c.is_selected,
                primary: c.monitor.is_primary,
                notch: c.shows_notch,
                dock: c.shows_dock,
            })
            .collect();
        dp.set_cards(ModelRc::new(VecModel::from(cards)));
        let (w, h) = self.displays.schema_size();
        dp.set_schema_width(w as f32);
        dp.set_schema_height(h as f32);
        match self.displays.selected() {
            Some((header, name, details, primary, notch, dock)) => {
                dp.set_has_selection(true);
                dp.set_header(ss(header));
                dp.set_name(ss(name));
                dp.set_details(ss(details));
                dp.set_primary(primary);
                dp.set_notch_on(notch);
                dp.set_dock_on(dock);
            }
            None => dp.set_has_selection(false),
        }
        dp.set_new_screen(match self.displays.new_screen_mode() {
            NewScreenMode::NotchAndDock => 0,
            NewScreenMode::NotchOnly => 1,
            NewScreenMode::None => 2,
        });
    }

    fn bind(self: &Rc<Self>) {
        let w = &self.window;

        let tr = w.global::<Tr>();
        tr.on_lookup(|k, _| ss(Strings::get(&k)));
        tr.on_format1(|k, a, _| ss(Strings::format(&k, &[&a])));

        let s = w.global::<S>();
        let vm = Rc::clone(&self.vm);
        s.on_value(move |k, _| ss(vm.value(&k)));
        let vm = Rc::clone(&self.vm);
        s.on_is_changed_of(move |k, _| vm.is_changed(&k));
        let vm = Rc::clone(&self.vm);
        s.on_flag_of(move |k, _| vm.flag(&k));
        let vm = Rc::clone(&self.vm);
        s.on_text_of(move |k, _| ss(vm.text(&k)));
        let vm = Rc::clone(&self.vm);
        s.on_list_of(move |k, _| {
            let items: Vec<SharedString> = vm.list(&k).into_iter().map(SharedString::from).collect();
            ModelRc::new(VecModel::from(items))
        });
        let vm = Rc::clone(&self.vm);
        s.on_choice_index(move |k, values, _| {
            let values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
            vm.choice_index(&k, &values)
        });
        // Yazma ve eylemler bağlama değerlendirmesi sırasında değil, sonraki turda yapılır (pencere yeniden çizilirken
        // görünüm modelinin ödünç alınmış olmaması için).
        let vm = Rc::downgrade(&self.vm);
        s.on_set(move |k, v| {
            let vm = vm.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(vm) = vm.upgrade() {
                    vm.set(&k, &v);
                }
            });
        });
        let vm = Rc::downgrade(&self.vm);
        s.on_action(move |name| {
            let vm = vm.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(vm) = vm.upgrade() {
                    vm.action(&name);
                }
            });
        });

        let sl = w.global::<SL>();
        let vm = Rc::clone(&self.vm);
        sl.on_tray_app_toggled(move |i, v| vm.set_tray_row(i as usize, v));
        let vm = Rc::clone(&self.vm);
        sl.on_search_source_toggled(move |i, v| vm.set_search_source(i as usize, v));
        let vm = Rc::clone(&self.vm);
        sl.on_home_card_toggled(move |i, v| vm.set_home_card_enabled(i as usize, v));
        let vm = Rc::clone(&self.vm);
        sl.on_home_card_move(move |i, step| vm.move_home_card(i as usize, step));
        let vm = Rc::clone(&self.vm);
        sl.on_tab_move(move |i, step| vm.move_tab(i as usize, step));

        let sc = w.global::<SC>();
        let m = Rc::downgrade(&self.shortcuts);
        sc.on_start_capture(move |i| {
            if let Some(m) = m.upgrade() {
                m.start_capture(i as usize);
            }
        });
        let m = Rc::downgrade(&self.shortcuts);
        sc.on_cancel_capture(move |i| {
            if let Some(m) = m.upgrade() {
                m.cancel_capture(i as usize);
            }
        });
        let m = Rc::downgrade(&self.shortcuts);
        sc.on_reset(move |i| {
            let m = m.clone();
            crate::infra::dispatch::post_local(move || {
                if let Some(m) = m.upgrade() {
                    m.reset_to_default(i as usize);
                }
            });
        });
        let m = Rc::downgrade(&self.shortcuts);
        sc.on_key(move |i, text, ctrl, alt, shift, win| m.upgrade().is_some_and(|m| m.handle_key(i as usize, &text, ctrl, alt, shift, win)));

        let dp = w.global::<DP>();
        let d = Rc::downgrade(&self.displays);
        dp.on_identify(move || {
            if let Some(d) = d.upgrade() {
                d.identify();
            }
        });
        let d = Rc::downgrade(&self.displays);
        dp.on_select(move |i| {
            if let Some(d) = d.upgrade() {
                d.select(Some(i as usize));
            }
        });
        let d = Rc::downgrade(&self.displays);
        dp.on_notch_toggled(move |v| {
            if let Some(d) = d.upgrade() {
                d.set_notch_on(v);
            }
        });
        let d = Rc::downgrade(&self.displays);
        dp.on_dock_toggled(move |v| {
            if let Some(d) = d.upgrade() {
                d.set_dock_on(v);
            }
        });
        let d = Rc::downgrade(&self.displays);
        dp.on_new_screen_changed(move |i| {
            use koy_core::display::NewScreenMode;
            if let Some(d) = d.upgrade() {
                d.set_new_screen_mode(match i {
                    1 => NewScreenMode::NotchOnly,
                    2 => NewScreenMode::None,
                    _ => NewScreenMode::NotchAndDock,
                });
            }
        });

        let data = w.global::<KoyWindowData>();
        let me = Rc::downgrade(self);
        data.on_select_page(move |i| {
            if let Some(h) = me.upgrade() {
                h.vm.clear_highlight();
                h.vm.select_page(i);
                h.update_page_activation();
                h.window.invoke_scroll_top();
            }
        });
        let vm = Rc::clone(&self.vm);
        data.on_select_sub_tab(move |page, sub| vm.select_sub_tab(page, sub));
        let vm = Rc::clone(&self.vm);
        data.on_search_changed(move |q| vm.set_search_query(&q));
        let me = Rc::downgrade(self);
        data.on_open_result(move |i| {
            if let Some(h) = me.upgrade() {
                h.vm.open_search_result(i as usize);
                h.window.invoke_scroll_top();
                // Vurgu bir süre sonra kalkar.
                let vm = Rc::downgrade(&h.vm);
                h.highlight_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(4), move || {
                    if let Some(vm) = vm.upgrade() {
                        vm.clear_highlight();
                    }
                });
            }
        });
        let vm = Rc::clone(&self.vm);
        data.on_changed_only_toggled(move |v| vm.set_show_changed_only(v));

        let me = Rc::downgrade(self);
        w.on_minimize(move || {
            if let Some(h) = me.upgrade() {
                h.window.window().set_minimized(true);
            }
        });
        let me = Rc::downgrade(self);
        w.on_hide_window(move || {
            if let Some(h) = me.upgrade() {
                h.hide();
            }
        });
        let me = Rc::downgrade(self);
        w.on_drag_title(move || {
            if let Some(h) = me.upgrade() {
                begin_drag(native_window::hwnd_of(h.window.window()));
            }
        });
        // Pencereyi kapatmak Koy'u kapatmaz; gizler.
        let me = Rc::downgrade(self);
        w.window().on_close_requested(move || {
            if let Some(h) = me.upgrade() {
                h.hide();
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }

    /// Görünüm modelinden pencereye.
    fn sync(&self) {
        let w = &self.window;
        let vm = &self.vm;
        w.global::<Tr>().set_rev(vm.lang_rev());
        let s = w.global::<S>();
        s.set_rev(vm.rev());
        s.set_changed_only(vm.show_changed_only());
        s.set_highlight(ss(vm.highlight()));

        let data = w.global::<KoyWindowData>();
        data.set_page(vm.selected_page());
        let subs: Vec<i32> = [0, 1, 2, 4].iter().map(|p| vm.sub_tab(*p)).collect();
        data.set_sub_tabs(ModelRc::new(VecModel::from(subs)));
        let results: Vec<SearchResultData> =
            vm.search_results().into_iter().map(|r| SearchResultData { title: ss(r.title), page: ss(r.page) }).collect();
        data.set_search_results(ModelRc::new(VecModel::from(results)));
        let (pct, level) = vm.memory();
        data.set_memory_pct(pct as f32);
        data.set_memory_level(level);

        let sl = w.global::<SL>();
        let toggles = |rows: Vec<(String, bool)>| -> ModelRc<ToggleItem> {
            ModelRc::new(VecModel::from(rows.into_iter().map(|(t, on)| ToggleItem { title: ss(t), on }).collect::<Vec<_>>()))
        };
        sl.set_tray_apps(toggles(vm.tray_rows().into_iter().map(|r| (r.exe_name, r.is_shown)).collect()));
        sl.set_search_sources(toggles(vm.search_sources().into_iter().map(|r| (r.name, r.is_enabled)).collect()));
        let order = |rows: Vec<(String, bool)>| -> ModelRc<OrderItem> {
            let n = rows.len();
            ModelRc::new(VecModel::from(
                rows.into_iter()
                    .enumerate()
                    .map(|(i, (t, on))| OrderItem { title: ss(t), on, can_up: i > 0, can_down: i + 1 < n })
                    .collect::<Vec<_>>(),
            ))
        };
        sl.set_home_cards(order(vm.home_cards().into_iter().map(|r| (r.label, r.is_enabled)).collect()));
        sl.set_tab_order(order(vm.tab_order().into_iter().map(|r| (r.label, true)).collect()));

        // Uygula / geri al geri sayımı: zamanlayıcı yalnız bekleyen değişiklik varken çalışır.
        if vm.is_apply_revert_pending() {
            if !self.revert_timer.running() {
                let vm = Rc::downgrade(vm);
                self.revert_timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || {
                    if let Some(vm) = vm.upgrade() {
                        vm.apply_revert_tick();
                    }
                });
            }
        } else {
            self.revert_timer.stop();
        }
    }

    /// Pencereyi gösterir; verilen sayfa (ve alt sekme) seçilir. Ayarlar diskten yeniden okunur, izleyici başlar.
    pub fn show(&self, page: Option<i32>) {
        self.vm.load_from_settings();
        self.shortcuts.load();
        if let Some(p) = page {
            self.vm.select_page(p);
        }
        let _ = self.window.show();
        self.window.window().set_minimized(false);
        self.update_page_activation();
        if self.watcher.borrow().is_none() {
            let folder = self.vm.store().path().parent().map(|p| p.to_path_buf()).unwrap_or_default();
            *self.watcher.borrow_mut() = Some(UserStyleWatcher::watch_folder(folder, on_settings_file_changed));
        }
        let weak = self.window.as_weak();
        // Yerel pencere olay döngüsü dönünce oluşur; öne getirme biraz sonra.
        slint::Timer::single_shot(Duration::from_millis(30), move || {
            if let Some(w) = weak.upgrade() {
                let hwnd = native_window::hwnd_of(w.window());
                if hwnd != 0 {
                    native_window::activate(hwnd);
                }
            }
        });
    }

    /// Ekranlar sayfası yalnız görünürken sistem olaylarını dinler.
    fn update_page_activation(&self) {
        if self.vm.selected_page() == 3 && self.is_visible() {
            self.displays.activate();
        } else {
            self.displays.deactivate();
        }
    }

    /// Gizler (kapatma düğmesi ve Alt+F4); izleyici bırakılır.
    pub fn hide(&self) {
        self.displays.deactivate();
        if let Some(w) = self.watcher.borrow_mut().take() {
            w.dispose();
        }
        let _ = self.window.hide();
    }

    pub fn is_visible(&self) -> bool {
        self.window.window().is_visible()
    }
}

fn on_settings_file_changed() {
    let host = CURRENT.with(|c| c.borrow().as_ref().and_then(std::rc::Weak::upgrade));
    if let Some(h) = host {
        h.vm.reload_from_disk_if_changed();
    }
}

/// Başlık çubuğundan sürükleme: sistem taşıma döngüsü (WM_NCLBUTTONDOWN + HTCAPTION).
fn begin_drag(hwnd: isize) {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, PostMessageW, WM_NCLBUTTONDOWN};
    if hwnd == 0 {
        return;
    }
    // SAFETY: kendi penceremize ileti gönderilir.
    unsafe {
        let _ = ReleaseCapture();
        let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(0));
    }
}

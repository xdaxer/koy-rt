//! Çentik penceresini sayfa görünüm modellerine bağlar: her modelin `changed` bildirimi kendi bölümünü yeniler,
//! pencerenin geri çağrıları ilgili komutları çalıştırır (C#'ta XAML bağlamaları ve komutlar).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use slint::{ComponentHandle, Global};

use koy_core::radio::RadioKind;

use super::notch_binding;
use super::notch_view_model::NotchViewModel;
use super::notch_window::NotchWindowHost;
use super::osd::osd_binding;
use crate::features::calendar::calendar_binding;
use crate::features::clipboard_panel::clipboard_binding;
use crate::features::control_center::control_center_binding;
use crate::features::focus::focus_binding;
use crate::features::home::home_binding;
use crate::features::search::search_binding::{self, GlyphFallback};
use crate::features::search::search_view_model::SearchViewModel;
use crate::features::shelf::shelf_binding;
use crate::infra::observable::Notifier;
use crate::resources::strings::Strings;
use crate::{Icons, NotchText, NotchWindow, SystemStatsData, SystemText};

/// Bağlamanın açtığı abonelikler; `dispose` hepsini kapatır.
pub struct NotchWindowBinding {
    subs: RefCell<Vec<(Weak<dyn HasNotifier>, u64)>>,
    search_sub: RefCell<Option<(Weak<SearchViewModel>, u64, u64, u64)>>,
    caret_serial: Cell<i32>,
}

/// `changed` bildirimi olan görünüm modelleri.
pub trait HasNotifier {
    fn notifier(&self) -> &Notifier;
}

macro_rules! has_notifier {
    ($($t:ty),*) => { $(impl HasNotifier for $t { fn notifier(&self) -> &Notifier { &self.changed } })* };
}
has_notifier!(
    NotchViewModel,
    crate::features::music::music_view_model::MusicViewModel,
    crate::features::music::lyrics_view_model::LyricsViewModel,
    crate::features::home::home_view_model::HomeViewModel,
    crate::features::home::quick_launch_view_model::QuickLaunchViewModel,
    crate::features::status::status_view_model::StatusViewModel,
    crate::features::focus::focus_view_model::FocusViewModel,
    crate::features::focus::tasks_view_model::TasksViewModel,
    crate::features::calendar::calendar_view_model::CalendarViewModel,
    crate::features::shelf::shelf_view_model::ShelfViewModel,
    crate::features::clipboard_panel::clipboard_view_model::ClipboardViewModel,
    crate::features::system_info::system_stats_view_model::SystemStatsViewModel,
    crate::features::control_center::control_center_view_model::ControlCenterViewModel,
    crate::features::control_center::wifi_page_view_model::WifiPageViewModel,
    crate::features::control_center::bluetooth_devices_view_model::BluetoothDevicesViewModel,
    super::osd::osd_indicator_view_model::OsdIndicatorViewModel
);

fn glyphs(win: &NotchWindow) -> GlyphFallback {
    let i = Icons::get(win);
    GlyphFallback { power: i.get_power(), gauge: i.get_gauge(), layers: i.get_layers(), app_window: i.get_app_window() }
}

impl NotchWindowBinding {
    pub fn bind(host: &Rc<NotchWindowHost>, vm: &Rc<NotchViewModel>) -> Rc<Self> {
        let b = Rc::new(Self { subs: RefCell::new(Vec::new()), search_sub: RefCell::new(None), caret_serial: Cell::new(0) });
        let win = &host.window;
        Self::install_text(win);
        b.wire_refresh(host, vm);
        Self::wire_callbacks(host, vm);
        b
    }

    fn install_text(win: &NotchWindow) {
        home_binding::install_text(win);
        control_center_binding::install_text(win);
        focus_binding::install_text(win);
        calendar_binding::install_text(win);
        shelf_binding::install_text(win);
        clipboard_binding::install_text(win);
        search_binding::install_text(win, crate::features::search::search_factory::glyph_path(koy_core::search::SearchGlyph::Help).unwrap_or(""));
        let s = SystemText::get(win);
        s.set_processor(Strings::get("SystemProcessor").into());
        s.set_memory(Strings::get("SystemMemory").into());
        s.set_koy_self(Strings::get("SystemKoySelf").into());
        let n = NotchText::get(win);
        n.set_pick_location(Strings::get("WeatherPickLocation").into());
        n.set_percent_format(if Strings::is_turkish() { "tr".into() } else { "en".into() });
        win.set_settings_label(Strings::get("Settings").into());
    }

    /// `source.changed` → `refresh(window)`; ilk değer hemen yazılır.
    fn watch<T: HasNotifier + 'static>(&self, source: &Rc<T>, win: &NotchWindow, refresh: impl Fn(&T, &NotchWindow) + 'static) {
        refresh(source, win);
        let ws = Rc::downgrade(source);
        let ww = win.as_weak();
        let id = source.notifier().subscribe(move || {
            if let (Some(s), Some(w)) = (ws.upgrade(), ww.upgrade()) {
                refresh(&s, &w);
            }
        });
        let weak: Weak<dyn HasNotifier> = Rc::downgrade(source) as Weak<dyn HasNotifier>;
        self.subs.borrow_mut().push((weak, id));
    }

    fn wire_refresh(self: &Rc<Self>, host: &Rc<NotchWindowHost>, vm: &Rc<NotchViewModel>) {
        let win = &host.window;
        // Çentik kabuğu: durum, sekmeler, boşta görünüm. Arama açılınca arama modeline de abone olunur.
        let me = Rc::downgrade(self);
        let hw = Rc::downgrade(host);
        self.watch(vm, win, move |vm, w| {
            w.set_d(notch_binding::to_data(vm));
            if let (Some(b), Some(h)) = (me.upgrade(), hw.upgrade()) {
                b.sync_search(&h, vm.search().filter(|_| vm.is_search_open()));
            }
        });
        // Müzik, kapak ve sözler (Ana sayfanın müzik yüzü ve kapalı çentik).
        let nv = Rc::downgrade(vm);
        let music_refresh = move |w: &NotchWindow| {
            if let Some(vm) = nv.upgrade() {
                w.set_music(home_binding::music_data(&vm.music));
                w.set_d(notch_binding::to_data(&vm));
            }
        };
        let r = music_refresh.clone();
        self.watch(&vm.music, win, move |_, w| r(w));
        let r = music_refresh.clone();
        self.watch(&vm.music.lyrics, win, move |_, w| r(w));
        {
            let ww = win.as_weak();
            let nv = Rc::downgrade(vm);
            vm.music.spectrum.updated.subscribe(move || {
                if let (Some(w), Some(vm)) = (ww.upgrade(), nv.upgrade()) {
                    w.set_music(home_binding::music_data(&vm.music));
                }
            });
        }
        // Ana yüz: durum, Odak satırı, sistem, hızlı başlat, kulaklık.
        let nv = Rc::downgrade(vm);
        let home_refresh = move |w: &NotchWindow| {
            if let Some(vm) = nv.upgrade() {
                w.set_home(home_binding::home_data(&vm.home));
            }
        };
        let r = home_refresh.clone();
        self.watch(&vm.home, win, move |_, w| r(w));
        let r = home_refresh.clone();
        self.watch(&vm.home.quick_launch, win, move |_, w| r(w));
        let nv = Rc::downgrade(vm);
        self.watch(&vm.status, win, move |_, w| {
            if let Some(vm) = nv.upgrade() {
                w.set_home(home_binding::home_data(&vm.home));
                w.set_d(notch_binding::to_data(&vm));
            }
        });
        let nv = Rc::downgrade(vm);
        self.watch(&vm.home.stats, win, move |s, w| {
            let st = s.state();
            w.set_sys(SystemStatsData {
                cpu_progress: st.cpu_progress as f32,
                cpu_text: st.cpu_text.clone().into(),
                top_process_text: st.top_process_text.clone().into(),
                ram_progress: st.ram_progress as f32,
                ram_text: st.ram_text.clone().into(),
                memory_detail_text: st.memory_detail_text.clone().into(),
                network_name: st.network_name.clone().into(),
                download_text: st.download_text.clone().into(),
                upload_text: st.upload_text.clone().into(),
                koy_memory_text: st.koy_memory_text.clone().into(),
            });
            drop(st);
            if let Some(vm) = nv.upgrade() {
                w.set_home(home_binding::home_data(&vm.home));
            }
        });
        // Odak ve görevler.
        let nv = Rc::downgrade(vm);
        self.watch(&vm.focus, win, move |f, w| {
            w.set_fdata(focus_binding::to_focus_data(&f.state(), &f.presets));
            if let Some(vm) = nv.upgrade() {
                w.set_home(home_binding::home_data(&vm.home));
                w.set_d(notch_binding::to_data(&vm));
            }
        });
        win.set_has_tasks(vm.focus.tasks.is_some());
        if let Some(tasks) = &vm.focus.tasks {
            self.watch(tasks, win, |t, w| w.set_tasks(focus_binding::to_tasks_data(&t.state())));
        }
        // Takvim, Raf, Pano.
        self.watch(&vm.calendar, win, |c, w| w.set_cal(calendar_binding::to_data(&c.state(), &c.weekday_headers)));
        self.watch(&vm.shelf, win, |s, w| {
            w.set_shelf_items(shelf_binding::to_items(&s.items()));
            w.set_shelf_count(s.count_text().into());
        });
        if let Some(clip) = &vm.clipboard {
            self.watch(clip, win, |c, w| {
                let s = c.state();
                w.set_clip_filters(clipboard_binding::filters(&s));
                w.set_clip_items(clipboard_binding::cards(&s));
                w.set_clip_empty(s.is_empty);
                w.set_clip_empty_text(s.empty_text.clone().into());
                w.set_clip_search(s.search_text.clone().into());
            });
        }
        // Kontrol Merkezi (sayfa, Wi-Fi listesi, Bluetooth cihazları).
        let cc_refresh = |c: &crate::features::control_center::control_center_view_model::ControlCenterViewModel, w: &NotchWindow| {
            w.set_control(control_center_binding::to_data(c));
        };
        self.watch(&vm.control_center, win, cc_refresh);
        let cv = Rc::downgrade(&vm.control_center);
        self.watch(&vm.control_center.wifi_page, win, move |_, w| {
            if let Some(c) = cv.upgrade() {
                w.set_control(control_center_binding::to_data(&c));
            }
        });
        let cv = Rc::downgrade(&vm.control_center);
        let nv = Rc::downgrade(vm);
        self.watch(&vm.control_center.bluetooth_devices, win, move |_, w| {
            if let Some(c) = cv.upgrade() {
                w.set_control(control_center_binding::to_data(&c));
            }
            if let Some(vm) = nv.upgrade() {
                w.set_home(home_binding::home_data(&vm.home));
            }
        });
        // Gösterge.
        let nv = Rc::downgrade(vm);
        self.watch(&vm.osd, win, move |o, w| {
            w.set_osd(osd_binding::to_data(&o.state()));
            if let Some(vm) = nv.upgrade() {
                w.set_d(notch_binding::to_data(&vm));
            }
        });
    }

    /// Arama modeli değişince (açıldı / kapandı) aboneliği yeniler.
    fn sync_search(&self, host: &Rc<NotchWindowHost>, search: Option<Rc<SearchViewModel>>) {
        let current = self.search_sub.borrow().as_ref().and_then(|(w, ..)| w.upgrade());
        let same = match (&current, &search) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        if let Some((w, a, b, c)) = self.search_sub.borrow_mut().take()
            && let Some(s) = w.upgrade()
        {
            s.changed.unsubscribe(a);
            s.caret_requested.unsubscribe(b);
            s.selection_changed.unsubscribe(c);
        }
        let Some(search) = search else { return };
        let win = &host.window;
        let fallback = Rc::new(glyphs(win));
        let scroll = Rc::new(Cell::new(-1));
        let serial = Rc::new(Cell::new(self.caret_serial.get()));
        let caret = Rc::new(Cell::new(0));
        let push = {
            let ww = win.as_weak();
            let sw = Rc::downgrade(&search);
            let (fallback, scroll, serial, caret) = (fallback.clone(), scroll.clone(), serial.clone(), caret.clone());
            Rc::new(move || {
                if let (Some(w), Some(s)) = (ww.upgrade(), sw.upgrade()) {
                    w.set_search(search_binding::to_data(&s, &fallback, scroll.get(), (caret.get(), serial.get())));
                    scroll.set(-1);
                }
            })
        };
        push();
        let p = push.clone();
        let a = search.changed.subscribe(move || p());
        let p = push.clone();
        let (cs, cc) = (serial.clone(), caret.clone());
        let b = search.caret_requested.subscribe(move |pos| {
            cc.set(*pos);
            cs.set(cs.get() + 1);
            p();
        });
        let p = push;
        let sc = scroll;
        let c = search.selection_changed.subscribe(move |i| {
            sc.set(*i);
            p();
        });
        *self.search_sub.borrow_mut() = Some((Rc::downgrade(&search), a, b, c));
        self.caret_serial.set(serial.get());
        host.focus_search();
    }

    fn wire_callbacks(host: &Rc<NotchWindowHost>, vm: &Rc<NotchViewModel>) {
        let win = &host.window;
        macro_rules! on {
            ($setter:ident, |$v:ident $(, $arg:ident)*| $body:expr) => {{
                let w = Rc::downgrade(vm);
                win.$setter(move |$($arg),*| {
                    if let Some($v) = w.upgrade() {
                        $body;
                    }
                });
            }};
        }
        // Üst satır.
        on!(on_refresh_weather, |vm| vm.status.refresh_weather());
        on!(on_choose_location, |vm| vm.home.choose_weather_location());
        on!(on_show_music, |vm| vm.show_home_page(koy_core::notch::NotchHomePage::Music));
        on!(on_show_home, |vm| vm.show_home_page(koy_core::notch::NotchHomePage::Home));
        on!(on_select_tab, |vm, p| vm.select_tab(notch_binding::panel_from_index(p)));
        on!(on_open_notifications, |vm| vm.home.open_notification_center());
        on!(on_toggle_pinned, |vm| vm.toggle_pinned());
        // Gösterge: sesli komut kutusu ve kart.
        on!(on_voice_input_edited, |vm, t| vm.osd.set_voice_input(t.as_str()));
        {
            let h = Rc::downgrade(host);
            win.on_voice_enter(move || {
                if let Some(h) = h.upgrade()
                    && let Some(f) = h.voice_confirm_requested.borrow().as_ref()
                {
                    f();
                }
            });
        }
        on!(on_card_action, |vm| vm.osd.request_primary_action());
        on!(on_card_dismiss, |vm| vm.osd.request_dismiss());
        // Ana.
        on!(on_home_toggle_lyrics, |vm| vm.music.lyrics.toggle_enabled());
        on!(on_home_seek_line, |vm, i| vm.music.lyrics.seek_to_line(i.max(0) as usize));
        on!(on_home_seek, |vm, f| vm.music.seek(f as f64));
        on!(on_home_previous, |vm| vm.music.previous());
        on!(on_home_play_pause, |vm| vm.music.play_pause());
        on!(on_home_next, |vm| vm.music.next());
        on!(on_home_open_event, |vm, i| {
            let rows = std::iter::once(vm.home.next_event()).flatten().chain(vm.home.later_events());
            if let Some(r) = rows.into_iter().nth(i.max(0) as usize) {
                vm.home.open_event(r.date);
            }
        });
        on!(on_home_open_focus, |vm| vm.home.open_focus());
        on!(on_home_toggle_focus, |vm| vm.focus.toggle_running());
        on!(on_home_skip_focus, |vm| vm.focus.skip());
        on!(on_home_open_system, |vm| vm.home.open_system());
        on!(on_home_launch, |vm, i| vm.home.quick_launch.launch(i.max(0) as usize));
        on!(on_home_remove_quick, |vm, i| vm.home.quick_launch.remove(i.max(0) as usize));
        on!(on_home_add_quick, |vm| vm.home.quick_launch.add());
        on!(on_home_add_quick_folder, |vm| vm.home.quick_launch.add_folder());
        // Odak.
        on!(on_focus_select_tab, |vm, k| vm.focus.select_tab(focus_binding::kind_from_index(k)));
        on!(on_focus_toggle, |vm| vm.focus.toggle());
        on!(on_focus_reset, |vm| vm.focus.reset());
        on!(on_focus_skip, |vm| vm.focus.skip());
        on!(on_focus_lap, |vm| vm.focus.lap());
        on!(on_focus_increase, |vm| vm.focus.increase_countdown());
        on!(on_focus_decrease, |vm| vm.focus.decrease_countdown());
        on!(on_focus_set_minutes, |vm, m| vm.focus.set_countdown_minutes(m));
        on!(on_tasks_set_scope, |vm, s| if let Some(t) = &vm.focus.tasks { t.set_scope(focus_binding::scope_from_index(s)) });
        on!(on_tasks_toggle, |vm, i| if let Some(t) = &vm.focus.tasks {
            let id = t.state().rows.get(i.max(0) as usize).map(|r| r.id);
            if let Some(id) = id {
                t.toggle(id);
            }
        });
        on!(on_tasks_remove, |vm, i| if let Some(t) = &vm.focus.tasks {
            let id = t.state().rows.get(i.max(0) as usize).map(|r| r.id);
            if let Some(id) = id {
                t.remove(id);
            }
        });
        on!(on_tasks_title_edited, |vm, s| if let Some(t) = &vm.focus.tasks { t.set_new_title(s.as_str()) });
        on!(on_tasks_time_edited, |vm, s| if let Some(t) = &vm.focus.tasks { t.set_new_time(s.as_str()) });
        on!(on_tasks_add, |vm| if let Some(t) = &vm.focus.tasks { t.add() });
        // Takvim.
        on!(on_cal_previous, |vm| vm.calendar.previous());
        on!(on_cal_next, |vm| vm.calendar.next());
        on!(on_cal_today, |vm| vm.calendar.go_today());
        on!(on_cal_set_view, |vm, m| vm.calendar.set_view(calendar_binding::mode_from_index(m)));
        on!(on_cal_select_cell, |vm, i| {
            let d = vm.calendar.state().cells.get(i.max(0) as usize).map(|c| c.date);
            if let Some(d) = d {
                vm.calendar.select_day(d);
            }
        });
        on!(on_cal_select_week_row, |vm, i| {
            let d = vm.calendar.state().week_days.get(i.max(0) as usize).map(|c| c.date);
            if let Some(d) = d {
                vm.calendar.select_day(d);
            }
        });
        on!(on_cal_delete_event, |vm, id| vm.calendar.delete_event(id.as_str()));
        on!(on_cal_title_edited, |vm, t| vm.calendar.set_new_event_title(t.as_str()));
        on!(on_cal_time_edited, |vm, t| vm.calendar.set_new_event_time(t.as_str()));
        on!(on_cal_add_event, |vm| vm.calendar.add_event());
        // Raf.
        on!(on_shelf_remove, |vm, id| vm.shelf.remove_item(id.as_str()));
        on!(on_shelf_clear, |vm| vm.shelf.clear_all());
        on!(on_shelf_open, |vm, i| vm.shelf.open_item(i.max(0) as usize));
        on!(on_shelf_drag, |vm, i| vm.shelf.drag_item(i.max(0) as usize));
        // Pano.
        on!(on_clip_search_edited, |vm, t| if let Some(c) = &vm.clipboard { c.set_search_text(t.as_str()) });
        on!(on_clip_set_filter, |vm, f| if let Some(c) = &vm.clipboard { c.set_filter(clipboard_binding::filter_from_index(f)) });
        on!(on_clip_clear, |vm| if let Some(c) = &vm.clipboard { c.clear() });
        on!(on_clip_copy_back, |vm, i| if let Some(c) = &vm.clipboard {
            let id = c.state().items.get(i.max(0) as usize).map(|x| x.id);
            if let Some(id) = id {
                c.copy_back(id);
            }
        });
        on!(on_clip_toggle_pin, |vm, i| if let Some(c) = &vm.clipboard {
            let id = c.state().items.get(i.max(0) as usize).map(|x| x.id);
            if let Some(id) = id {
                c.toggle_pin(id);
            }
        });
        on!(on_clip_remove, |vm, i| if let Some(c) = &vm.clipboard {
            let id = c.state().items.get(i.max(0) as usize).map(|x| x.id);
            if let Some(id) = id {
                c.remove(id);
            }
        });
        // Kontrol Merkezi.
        let radio = |k: i32| if k == 1 { RadioKind::Bluetooth } else { RadioKind::WiFi };
        on!(on_cc_activate_radio, |vm, k| vm.control_center.activate_radio(radio(k)));
        on!(on_cc_open_page, |vm, k| vm.control_center.open_page(radio(k)));
        on!(on_cc_back, |vm| vm.control_center.back());
        on!(on_cc_open_dnd, |vm| vm.control_center.open_do_not_disturb_settings());
        on!(on_cc_volume, |vm, v| vm.control_center.on_volume_slider_changed(v as f64));
        on!(on_cc_toggle_mute, |vm| vm.control_center.toggle_mute());
        on!(on_cc_brightness, |vm, v| vm.control_center.on_brightness_slider_changed(v as f64));
        on!(on_cc_move_tile, |vm, id, to| {
            let mut order = vm.control_center.state().tile_order.clone();
            if let Some(from) = order.iter().position(|t| t.as_str() == id.as_str()) {
                let item = order.remove(from);
                order.insert((to.max(0) as usize).min(order.len()), item);
                vm.control_center.save_tile_order(order);
            }
        });
        on!(on_cc_wifi_activate, |vm, s| vm.control_center.wifi_page.activate(s.as_str()));
        on!(on_cc_wifi_disconnect, |vm, _s| vm.control_center.wifi_page.disconnect());
        on!(on_cc_wifi_password, |vm, s, p| vm.control_center.wifi_page.set_password(s.as_str(), p.as_str()));
        on!(on_cc_wifi_connect, |vm, s| vm.control_center.wifi_page.connect_with_password(s.as_str()));
        on!(on_cc_wifi_location, |vm| vm.control_center.wifi_page.open_location_settings());
        on!(on_cc_wifi_settings, |vm| vm.control_center.wifi_page.open_settings());
        on!(on_cc_bt_activate, |vm, id| vm.control_center.bluetooth_devices.activate(id.as_str()));
        on!(on_cc_bt_settings, |vm| vm.control_center.bluetooth_devices.open_settings());
        // Arama.
        {
            let w = Rc::downgrade(vm);
            win.on_search_key(move |t, ctrl, shift, at_start, at_end| {
                w.upgrade()
                    .and_then(|vm| vm.search())
                    .is_some_and(|s| search_binding::handle_key(&s, t.as_str(), ctrl, shift, at_start, at_end))
            });
            let w = Rc::downgrade(vm);
            win.on_search_completion(move |t| {
                w.upgrade().and_then(|vm| vm.search()).map(|s| search_binding::completion(&s, t.as_str())).unwrap_or_default().into()
            });
        }
        on!(on_search_edited, |vm, t| if let Some(s) = vm.search() { s.set_query_text(t.as_str()) });
        on!(on_search_toggle_help, |vm| if let Some(s) = vm.search() { s.toggle_help() });
        on!(on_search_toggle_history, |vm| if let Some(s) = vm.search() { s.toggle_history() });
        on!(on_search_open_settings, |vm| if let Some(s) = vm.search() { s.open_settings() });
        on!(on_search_toggle_pin, |vm| if let Some(s) = vm.search() { s.toggle_pin() });
        on!(on_search_activate_row, |vm, i| if let Some(s) = vm.search() { s.activate_row(i) });
        on!(on_search_run_action, |vm, r, a| if let Some(s) = vm.search() { s.run_row_action(r, a) });
        on!(on_search_drag_row, |vm, i| if let Some(s) = vm.search() {
            // Satır sürüklenebilirse (dosya, adres, metin) OLE sürüklemesi başlar; dosya kopyalanır, diğerleri metin olur.
            let drag = s.state().rows.iter().find(|r| r.index == i).and_then(|r| r.result.drag.clone());
            if let Some(d) = drag {
                match d.kind {
                    koy_core::search::SearchDragKind::File => crate::infra::ole_dnd::drag_file(&d.value),
                    koy_core::search::SearchDragKind::Url | koy_core::search::SearchDragKind::Text => crate::infra::ole_dnd::drag_text(&d.value),
                    koy_core::search::SearchDragKind::None => {}
                }
            }
        });
    }

    pub fn dispose(&self) {
        for (w, id) in self.subs.borrow_mut().drain(..) {
            if let Some(s) = w.upgrade() {
                s.notifier().unsubscribe(id);
            }
        }
        if let Some((w, a, b, c)) = self.search_sub.borrow_mut().take()
            && let Some(s) = w.upgrade()
        {
            s.changed.unsubscribe(a);
            s.caret_requested.unsubscribe(b);
            s.selection_changed.unsubscribe(c);
        }
    }
}

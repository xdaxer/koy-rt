//! Çentiğin merkez görünüm modeli: durum (kapalı / göz atma / açık), sekmeler, boyut, boşta görünüm (saat, pil,
//! halka, radyo simgeleri), gizlilik göstergesi, malzeme ve sayfa servislerinin görünürlüğe göre açılıp kapanması.
//! Karşılığı: `Features/Notch/NotchViewModel.cs`, `NotchTabs.cs`, `NotchTabViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use koy_core::appearance::{MaterialLevel, MaterialSetting};
use koy_core::clock::{ClockFormatPolicy, SecondClock};
use koy_core::notch::{
    ClockTick, HomeCard, IdleDisplay, IdleDisplayFormat, NotchController, NotchHomePage, NotchHomePages, NotchLayout, NotchPanel, NotchState,
    NotchStyle, NotchSurface, NotchTabBarStyle, NotchVisualizer, NotchVisualizerColorSource,
};
use koy_core::power::BatteryAlertKind;
use koy_core::privacy::{CaptureUsagePolicy, PrivacyState};
use koy_core::visibility::{VisibilityController, VisibilityMode, VisibilityResult};

use super::live_background::live_background_state::LiveBackgroundState;
use super::osd::osd_indicator_view_model::{OsdIndicatorViewModel, OsdType};
use crate::features::calendar::calendar_view_model::CalendarViewModel;
use crate::features::clipboard_panel::clipboard_view_model::ClipboardViewModel;
use crate::features::control_center::control_center_view_model::ControlCenterViewModel;
use crate::features::focus::focus_view_model::FocusViewModel;
use crate::features::home::home_view_model::{HomeParts, HomeViewModel};
use crate::features::home::quick_launch_view_model::QuickLaunchViewModel;
use crate::features::music::music_view_model::MusicViewModel;
use crate::features::search::search_view_model::SearchViewModel;
use crate::features::shelf::shelf_view_model::ShelfViewModel;
use crate::features::status::status_view_model::StatusViewModel;
use crate::features::system_info::system_stats_view_model::SystemStatsViewModel;
use crate::infra::dispatch;
use crate::infra::observable::{LocalEvent, Notifier, SharedSettings};
use crate::infra::system_events::{self, SystemEvents};
use crate::infra::ui_ref::UiRef;
use crate::resources::material_state::MaterialState;
use crate::resources::strings::{self, Strings};
use crate::resources::theme_manager::ThemeManager;

/// Sekme çubuğunun bir sekmesi: yazı, simge anahtarı ve sayfa rengi anahtarı.
#[derive(Debug, Clone)]
pub struct NotchTab {
    pub panel: NotchPanel,
    pub label: String,
    /// `Icons` global'indeki ad (ör. "tab-home").
    pub icon: &'static str,
    /// Tema anahtarı (ör. "KoyNotchAccentHome").
    pub accent: &'static str,
    pub is_selected: bool,
}

/// Sekme çubuğunun tanımı (C# NotchTabs).
fn tab_definition(panel: NotchPanel) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match panel {
        NotchPanel::Home => ("TabHome", "tab-home", "KoyNotchAccentHome"),
        NotchPanel::Focus => ("TabFocus", "tab-focus", "KoyNotchAccentFocus"),
        NotchPanel::Calendar => ("TabCalendar", "tab-calendar", "KoyNotchAccentCalendar"),
        NotchPanel::Shelf => ("TabShelf", "tab-shelf", "KoyNotchAccentShelf"),
        NotchPanel::Clipboard => ("TabClipboard", "tab-clipboard", "KoyNotchAccentClipboard"),
        NotchPanel::System => ("TabSystem", "tab-system", "KoyNotchAccentSystem"),
        NotchPanel::ControlCenter => ("TabControlCenter", "tab-control", "KoyNotchAccentControl"),
        #[allow(unreachable_patterns)]
        _ => return None,
    })
}

pub fn create_tab(panel: NotchPanel, selected: bool) -> NotchTab {
    match tab_definition(panel) {
        Some((label, icon, accent)) => NotchTab { panel, label: Strings::get(label), icon, accent, is_selected: selected },
        None => NotchTab { panel, label: format!("{panel:?}"), icon: "", accent: "", is_selected: selected },
    }
}

/// Görünümün okuduğu durum.
#[derive(Clone)]
pub struct NotchVmState {
    pub state: NotchState,
    pub current_panel: NotchPanel,
    pub target_width: f64,
    pub target_height: f64,
    pub target_corner_radius: f64,
    pub is_notch_visible: bool,
    pub clock_time: String,
    /// Klasik kapalı çentikte saatin yanına eklenen kısa tarih (" · 17 Kas").
    pub clock_date_suffix: String,
    /// Klasik göz atma satırının sonuna eklenen ikinci saat.
    pub clock_peek_extras: String,
    pub battery_percent: i32,
    pub is_charging: bool,
    pub has_battery: bool,
    pub battery_fill_width: f64,
    /// Halkanın dolu kısmı (SVG yol verisi; 18×18 tuval); boşsa dolu kısım yok.
    pub battery_ring_path: String,
    pub battery_ring_low: bool,
    pub battery_percent_text: String,
    pub idle_day_time: String,
    pub idle_peek_date: String,
    pub tabs: Vec<NotchTab>,
    pub is_camera_active: bool,
    pub is_microphone_active: bool,
    pub privacy_tool_tip: String,
    /// Çentik şeklinin zemini (ARGB).
    pub surface: u32,
    /// Kapalı çentik ambiyans zemini: soldan sağa gradyan durakları (boşsa düz `surface`).
    pub surface_gradient: Vec<u32>,
}

pub struct NotchParts {
    pub settings: SharedSettings,
    pub controller: Arc<NotchController>,
    pub visibility: Arc<VisibilityController>,
    pub music: Rc<MusicViewModel>,
    pub control_center: Rc<ControlCenterViewModel>,
    pub status: Rc<StatusViewModel>,
    pub focus: Rc<FocusViewModel>,
    pub shelf: Rc<ShelfViewModel>,
    pub calendar: Rc<CalendarViewModel>,
    pub system_stats: Rc<SystemStatsViewModel>,
    pub quick_launch: Rc<QuickLaunchViewModel>,
    pub clipboard: Option<Rc<ClipboardViewModel>>,
    pub shell_actions: Option<Arc<dyn koy_core::shell::IShellActionService>>,
}

pub struct NotchViewModel {
    pub settings: SharedSettings,
    pub controller: Arc<NotchController>,
    pub visibility: Arc<VisibilityController>,
    pub music: Rc<MusicViewModel>,
    pub control_center: Rc<ControlCenterViewModel>,
    pub status: Rc<StatusViewModel>,
    pub focus: Rc<FocusViewModel>,
    pub shelf: Rc<ShelfViewModel>,
    pub calendar: Rc<CalendarViewModel>,
    pub clipboard: Option<Rc<ClipboardViewModel>>,
    pub home: Rc<HomeViewModel>,
    pub osd: Rc<OsdIndicatorViewModel>,
    pub live_background: Rc<LiveBackgroundState>,
    /// Kontrol paneli açıksa Wi-Fi / Bluetooth izlenir ve kapalı çentikte simgeleri gösterilebilir.
    pub show_radio_status: bool,
    search: RefCell<Option<Rc<SearchViewModel>>>,
    window_before_search: Cell<isize>,
    state: RefCell<NotchVmState>,
    clock_timer: RefCell<Option<slint::Timer>>,
    time_changed_sub: RefCell<Option<system_events::Subscription>>,
    ring_percent: Cell<i32>,
    opaque_surface: Cell<u32>,
    glass_surface: Cell<u32>,
    /// Ambiyans kipinde seçili önayarın okunabilir koyuluğa çekilmiş statik renkleri (video ya da zamanlayıcı yok).
    ambient_collapsed: Vec<u32>,
    core_subs: RefCell<Vec<Box<dyn FnOnce()>>>,
    disposed: Cell<bool>,
    pub changed: Notifier,
    /// Hedef boyut değişti (genişlik, yükseklik).
    pub target_size_changed: LocalEvent<(f64, f64)>,
    pub target_visibility_changed: LocalEvent<bool>,
    pub panel_transition_triggered: LocalEvent<i32>,
    pub shake_requested: LocalEvent<()>,
    /// Arama kapandı; değer odağın döneceği pencere (yoksa 0).
    pub search_closed: LocalEvent<isize>,
    /// Ayarlar düğmesi (App verir: Koy penceresini Çentik sayfasında açar).
    pub open_settings_handler: RefCell<Option<Box<dyn Fn()>>>,
}

const RING_NORMAL: u32 = 0xFF4A_DE80;
const RING_LOW: u32 = 0xFFFB_7185;
const DEFAULT_SURFACE: u32 = 0xFF0A_0A0B;

impl NotchViewModel {
    pub fn new(p: NotchParts) -> Rc<Self> {
        let ambient_collapsed = {
            let s = p.settings.borrow();
            let n = &s.notch;
            if n.collapsed_background == koy_core::notch::NotchCollapsedBackground::Ambient {
                koy_core::notch::CollapsedBackgroundPolicy::resolve_ambient_stops(Some(&n.live_background_preset))
                    .iter()
                    .map(|c| 0xFF00_0000 | ((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32)
                    .collect::<Vec<u32>>()
            } else {
                Vec::new()
            }
        };
        let (live_bg, home_cards) = {
            let s = p.settings.borrow();
            (LiveBackgroundState::new(&s.notch), s.notch.home_cards.iter().copied().collect::<Vec<_>>())
        };
        let controller = Arc::clone(&p.controller);
        let open_focus_ctrl = Arc::clone(&controller);
        let home = HomeViewModel::new(HomeParts {
            music: Rc::clone(&p.music),
            status: Rc::clone(&p.status),
            focus: Rc::clone(&p.focus),
            stats: p.system_stats,
            quick_launch: p.quick_launch,
            shell: p.shell_actions,
            open_focus: Some(Box::new(move || open_focus_ctrl.go_to_panel(NotchPanel::Focus))),
            bluetooth: Some(Rc::clone(&p.control_center.bluetooth_devices)),
            home_cards: Some(home_cards),
        });
        let is_visible = p.visibility.is_visible();
        let vm = Rc::new(NotchViewModel {
            settings: p.settings,
            controller,
            visibility: p.visibility,
            music: p.music,
            control_center: p.control_center,
            status: p.status,
            focus: p.focus,
            shelf: p.shelf,
            calendar: p.calendar,
            clipboard: p.clipboard,
            home,
            osd: OsdIndicatorViewModel::new(),
            live_background: Rc::new(live_bg),
            show_radio_status: false,
            search: RefCell::new(None),
            window_before_search: Cell::new(0),
            state: RefCell::new(NotchVmState {
                state: NotchState::Collapsed,
                current_panel: NotchPanel::Home,
                target_width: 236.0,
                target_height: 34.0,
                target_corner_radius: 18.0,
                is_notch_visible: is_visible,
                clock_time: String::new(),
                clock_date_suffix: String::new(),
                clock_peek_extras: String::new(),
                battery_percent: 100,
                is_charging: false,
                has_battery: false,
                battery_fill_width: 17.0,
                battery_ring_path: String::new(),
                battery_ring_low: false,
                battery_percent_text: String::new(),
                idle_day_time: String::new(),
                idle_peek_date: String::new(),
                tabs: Vec::new(),
                is_camera_active: false,
                is_microphone_active: false,
                privacy_tool_tip: String::new(),
                surface: DEFAULT_SURFACE,
                surface_gradient: Vec::new(),
            }),
            clock_timer: RefCell::new(None),
            time_changed_sub: RefCell::new(None),
            ring_percent: Cell::new(-1),
            opaque_surface: Cell::new(DEFAULT_SURFACE),
            glass_surface: Cell::new(DEFAULT_SURFACE),
            ambient_collapsed,
            core_subs: RefCell::new(Vec::new()),
            disposed: Cell::new(false),
            changed: Notifier::default(),
            target_size_changed: LocalEvent::default(),
            target_visibility_changed: LocalEvent::default(),
            panel_transition_triggered: LocalEvent::default(),
            shake_requested: LocalEvent::default(),
            search_closed: LocalEvent::default(),
            open_settings_handler: RefCell::new(None),
        });
        // Rc içinde alan değiştirmek için show_radio_status ayrı hesaplanıp yeniden kurulmaz: alan sabit.
        let show_radio = vm.controller.enabled_panels().contains(&NotchPanel::ControlCenter);
        // SAFETY yok: alan Cell değil; bu yüzden aşağıdaki kurucu yardımcıyla ayarlanır.
        let vm = Self::with_radio_status(vm, show_radio);
        vm.wire();
        vm
    }

    fn with_radio_status(vm: Rc<Self>, show: bool) -> Rc<Self> {
        match Rc::try_unwrap(vm) {
            Ok(mut inner) => {
                inner.show_radio_status = show;
                Rc::new(inner)
            }
            Err(rc) => rc,
        }
    }

    fn wire(self: &Rc<Self>) {
        let ctrl = Arc::clone(&self.controller);
        let weak = Rc::downgrade(self);
        let open_day = weak.clone();
        self.home.attach_calendar(
            Rc::clone(self.calendar.events()),
            Box::new(move |day| {
                if let Some(vm) = open_day.upgrade() {
                    vm.open_calendar_day(day);
                }
            }),
        );
        let c2 = Arc::clone(&ctrl);
        *self.home.open_system_page.borrow_mut() = Some(Box::new(move || c2.go_to_panel(NotchPanel::System)));
        let c3 = Arc::clone(&ctrl);
        *self.home.quick_launch.hold_open.borrow_mut() = Some(Box::new(move |hold| if hold { c3.hold_open() } else { c3.release_hold() }));

        // Core olayları zamanlayıcı iş parçacıklarından gelir; durum yalnız arayüz iş parçacığında değişir.
        let me = UiRef::new(self);
        let s = ctrl.state_changed().subscribe(move |st: &NotchState| {
            let st = *st;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && !vm.disposed.get()
                {
                    vm.apply_state(st);
                }
            });
        });
        let c = Arc::clone(&ctrl);
        self.core_subs.borrow_mut().push(Box::new(move || c.state_changed().unsubscribe(s)));
        let s = ctrl.panel_changed().subscribe(move |p: &NotchPanel| {
            let p = *p;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && !vm.disposed.get()
                {
                    vm.on_panel_changed(p);
                }
            });
        });
        let c = Arc::clone(&ctrl);
        self.core_subs.borrow_mut().push(Box::new(move || c.panel_changed().unsubscribe(s)));
        let s = ctrl.panel_transition_triggered().subscribe(move |d: &i32| {
            let d = *d;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    vm.panel_transition_triggered.invoke(&d);
                }
            });
        });
        let c = Arc::clone(&ctrl);
        self.core_subs.borrow_mut().push(Box::new(move || c.panel_transition_triggered().unsubscribe(s)));
        let s = ctrl.enabled_panels_changed().subscribe(move |_| {
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && !vm.disposed.get()
                {
                    vm.rebuild_tabs();
                    vm.changed.notify();
                }
            });
        });
        let c = Arc::clone(&ctrl);
        self.core_subs.borrow_mut().push(Box::new(move || c.enabled_panels_changed().unsubscribe(s)));
        let s = ctrl.pinned_changed().subscribe(move |_| {
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    vm.changed.notify();
                }
            });
        });
        let c = Arc::clone(&ctrl);
        self.core_subs.borrow_mut().push(Box::new(move || c.pinned_changed().unsubscribe(s)));
        let vis = Arc::clone(&self.visibility);
        let s = vis.changed().subscribe(move |r: &VisibilityResult| {
            let r = *r;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && !vm.disposed.get()
                {
                    vm.apply_visibility(r);
                }
            });
        });
        self.core_subs.borrow_mut().push(Box::new(move || vis.changed().unsubscribe(s)));

        // Görünüm modeli olayları (arayüz iş parçacığı).
        let w = Rc::downgrade(self);
        let id = self.music.track_changed.subscribe(move |_| {
            if let Some(vm) = w.upgrade() {
                vm.controller.on_track_changed();
                vm.visibility.on_media_activity();
            }
        });
        let m = Rc::clone(&self.music);
        self.core_subs.borrow_mut().push(Box::new(move || m.track_changed.unsubscribe(id)));
        let w = Rc::downgrade(self);
        let last_playing = Cell::new(false);
        let id = self.music.changed.subscribe(move || {
            if let Some(vm) = w.upgrade() {
                let playing = vm.is_music_playing();
                let state = vm.state.borrow().state;
                if playing != last_playing.replace(playing) && state == NotchState::Collapsed {
                    vm.update_target_size();
                    vm.update_active_panels();
                } else if state != NotchState::Expanded {
                    vm.update_surface();
                }
                vm.changed.notify();
            }
        });
        let m = Rc::clone(&self.music);
        self.core_subs.borrow_mut().push(Box::new(move || m.changed.unsubscribe(id)));
        let w = Rc::downgrade(self);
        let id = self.focus.timer_finished.subscribe(move |_| {
            if let Some(vm) = w.upgrade() {
                // Kısa süre göz atma kipine geçirir ve titrer.
                vm.controller.on_track_changed();
                vm.shake_requested.invoke(&());
            }
        });
        let f = Rc::clone(&self.focus);
        self.core_subs.borrow_mut().push(Box::new(move || f.timer_finished.unsubscribe(id)));
        let w = Rc::downgrade(self);
        let id = self.focus.changed.subscribe(move || {
            if let Some(vm) = w.upgrade() {
                vm.changed.notify();
            }
        });
        let f = Rc::clone(&self.focus);
        self.core_subs.borrow_mut().push(Box::new(move || f.changed.unsubscribe(id)));
        let w = Rc::downgrade(self);
        let last_on_battery = Cell::new(self.status.state().is_on_battery);
        let id = self.status.changed.subscribe(move || {
            if let Some(vm) = w.upgrade() {
                vm.update_battery_state();
                // Fişe takılıp çıkarılınca canlı arka plan hareket ↔ ilk kare arasında geçer.
                let on_battery = vm.status.state().is_on_battery;
                if on_battery != last_on_battery.replace(on_battery) {
                    vm.update_active_panels();
                }
                vm.changed.notify();
            }
        });
        let st = Rc::clone(&self.status);
        self.core_subs.borrow_mut().push(Box::new(move || st.changed.unsubscribe(id)));
        let w = Rc::downgrade(self);
        let id = self.home.changed.subscribe(move || {
            if let Some(vm) = w.upgrade() {
                vm.update_active_panels();
                vm.changed.notify();
            }
        });
        let h = Rc::clone(&self.home);
        self.core_subs.borrow_mut().push(Box::new(move || h.changed.unsubscribe(id)));

        // OSD.
        let w = Rc::downgrade(self);
        *self.osd.can_show.borrow_mut() =
            Some(Box::new(move || w.upgrade().is_some_and(|vm| !vm.disposed.get() && vm.state.borrow().state != NotchState::Expanded)));
        let w = Rc::downgrade(self);
        self.osd.activated.subscribe(move || {
            if let Some(vm) = w.upgrade() {
                // Akıllı/Fareyle modunda çentik gizli olabilir; gösterge süresince görünür kalsın.
                vm.visibility.on_media_activity();
                // Kart gösterildiği sürece çentik gizlenmez.
                vm.visibility.set_hold(vm.osd.kind() == OsdType::Card);
                // Kart görünürken imleç kartın ya da × düğmesinin üstüne gelince çentik kendiliğinden açılmasın. Ses /
                // parlaklık gibi düğmesiz göstergelerde bastırılmaz.
                vm.controller.set_hover_open_suppressed(vm.is_card_shown());
                vm.update_target_size();
                vm.update_active_panels();
            }
        });
        let w = Rc::downgrade(self);
        self.osd.dismissed.subscribe(move || {
            if let Some(vm) = w.upgrade() {
                vm.visibility.set_hold(false);
                vm.controller.set_hover_open_suppressed(vm.is_card_shown());
                vm.update_target_size();
                vm.update_active_panels();
            }
        });
        let w = Rc::downgrade(self);
        self.osd.changed.subscribe(move || {
            if let Some(vm) = w.upgrade() {
                vm.changed.notify();
            }
        });

        self.rebuild_tabs();
        // Windows tepsisi gizli olduğu için Wi-Fi ve Bluetooth'un durumunu gösteren tek yer çentik.
        if self.show_radio_status && IdleDisplay::show_radio(self.settings.borrow().notch.idle_display) {
            self.control_center.start_radio_status();
        }
        self.update_clock();
        self.update_battery_state();
        self.update_glass_surface();
        let w = Rc::downgrade(self);
        MaterialState::on_changed(move || {
            if let Some(vm) = w.upgrade()
                && !vm.disposed.get()
            {
                vm.on_material_changed();
            }
        });
        let w = Rc::downgrade(self);
        ThemeManager::on_applied(move || {
            if let Some(vm) = w.upgrade()
                && !vm.disposed.get()
            {
                vm.on_material_changed();
            }
        });
        self.update_target_size();
        self.update_active_panels();
        if self.state.borrow().is_notch_visible {
            self.start_clock_timer();
        }
    }

    pub fn state(&self) -> std::cell::Ref<'_, NotchVmState> {
        self.state.borrow()
    }

    pub fn search(&self) -> Option<Rc<SearchViewModel>> {
        self.search.borrow().clone()
    }

    /// Arama açık: durum satırı, sayfa ve sekmeler gizlidir.
    pub fn is_search_open(&self) -> bool {
        self.search.borrow().as_ref().is_some_and(|s| s.is_open())
    }

    fn idle_mode(&self) -> koy_core::notch::IdleDisplayMode {
        self.settings.borrow().notch.idle_display
    }

    // ---- Türetilmiş özellikler ----

    pub fn show_idle_clock(&self) -> bool {
        IdleDisplay::show_clock(self.idle_mode())
    }
    pub fn is_idle_ring(&self) -> bool {
        IdleDisplay::is_ring(self.idle_mode())
    }
    pub fn show_idle_battery(&self) -> bool {
        self.state.borrow().has_battery && IdleDisplay::show_battery(self.idle_mode()) && !self.is_idle_ring() && !self.focus.state().has_running
    }
    pub fn show_idle_classic_clock(&self) -> bool {
        self.show_idle_clock() && !self.is_idle_ring()
    }
    pub fn show_idle_ring(&self) -> bool {
        self.state.borrow().has_battery && self.is_idle_ring() && !self.focus.state().has_running
    }
    pub fn show_idle_peek_date(&self) -> bool {
        self.is_idle_ring()
            && NotchLayout::show_peek_details(self.state.borrow().state, self.music.state().has_track)
            && IdleDisplay::show_peek_details(self.idle_mode())
    }
    pub fn show_idle_timer(&self) -> bool {
        self.focus.state().has_running
    }
    pub fn idle_timer_text(&self) -> String {
        self.focus.state().running_text.clone()
    }
    pub fn is_floating(&self) -> bool {
        NotchStyle::is_floating(self.settings.borrow().notch.style)
    }
    pub fn ear_radius(&self) -> f64 {
        NotchStyle::ear_radius(self.settings.borrow().notch.style)
    }
    pub fn top_offset_dip(&self) -> f64 {
        NotchStyle::top_offset_dip(self.settings.borrow().notch.style)
    }
    pub fn reduce_motion(&self) -> bool {
        self.settings.borrow().appearance.battery_saver_reduce_motion && self.status.state().is_on_battery
    }
    pub fn show_music_waveform(&self) -> bool {
        self.settings.borrow().music.show_waveform
    }
    /// Dalga simgesinin rengi: albüm rengi seçiliyse ve varsa kapaktan; yoksa yeşil gösterge rengi.
    pub fn waveform_color(&self) -> u32 {
        if self.settings.borrow().music.visualizer_color_source == NotchVisualizerColorSource::Artwork
            && let Some(c) = self.music.state().paint.spectrum
        {
            return c.as_argb_encoded();
        }
        // Kaynak yoksa (tema tanımlamıyor) C#'taki gibi yeşil gösterge rengi; 0 saydam olurdu ve çubuklar görünmezdi.
        match ThemeManager::resource("KoyNotchAccentMusic") {
            Some(crate::resources::theme_manager::ResValue::Color(c)) => c,
            _ => 0xFF4A_DE80,
        }
    }
    pub fn is_music_page_selected(&self) -> bool {
        self.state.borrow().current_panel == NotchPanel::Home && self.home.is_music_page()
    }
    pub fn is_home_page_selected(&self) -> bool {
        self.state.borrow().current_panel == NotchPanel::Home && self.home.is_home_page()
    }
    pub fn show_tab_bar(&self) -> bool {
        self.settings.borrow().notch.tab_bar_style != NotchTabBarStyle::Hidden
    }
    pub fn show_tab_labels(&self) -> bool {
        self.settings.borrow().notch.tab_bar_style == NotchTabBarStyle::IconAndLabel
    }
    pub fn show_idle_radio(&self) -> bool {
        let mode = self.idle_mode();
        self.show_radio_status
            && !self.focus.state().has_running
            && (IdleDisplay::show_radio_when_collapsed(mode)
                || (IdleDisplay::show_radio(mode) && NotchLayout::show_peek_details(self.state.borrow().state, self.music.state().has_track)))
    }
    pub fn is_pinned(&self) -> bool {
        self.controller.is_pinned()
    }
    pub fn has_privacy_activity(&self) -> bool {
        let st = self.state.borrow();
        st.is_camera_active || st.is_microphone_active
    }
    pub fn clock_extras(&self) -> String {
        let st = self.state.borrow();
        if NotchLayout::show_peek_details(st.state, self.music.state().has_track) && IdleDisplay::show_peek_details(self.idle_mode()) {
            format!("{}{}", st.clock_date_suffix, st.clock_peek_extras)
        } else {
            st.clock_date_suffix.clone()
        }
    }
    pub fn clock_display(&self) -> String {
        format!("{}{}", self.state.borrow().clock_time, self.clock_extras())
    }
    pub fn show_collapsed_layout(&self) -> bool {
        NotchLayout::show_collapsed_layout(self.state.borrow().state, self.music.state().has_track)
    }
    pub fn show_peek_layout(&self) -> bool {
        NotchLayout::show_peek_layout(self.state.borrow().state, self.music.state().has_track)
    }
    pub fn is_music_playing(&self) -> bool {
        let m = self.music.state();
        m.has_track && m.is_playing
    }

    // ---- Gizlilik göstergesi ----

    pub fn update_privacy_state(&self, state: &PrivacyState) {
        let enabled = self.settings.borrow().notch.show_privacy_indicator;
        {
            let mut st = self.state.borrow_mut();
            if !enabled {
                st.is_camera_active = false;
                st.is_microphone_active = false;
                st.privacy_tool_tip.clear();
            } else {
                st.is_camera_active = state.is_camera_in_use;
                st.is_microphone_active = state.is_microphone_in_use;
                st.privacy_tool_tip = CaptureUsagePolicy::format_tool_tip(
                    state,
                    &Strings::get("PrivacyTooltipCamAndMic"),
                    &Strings::get("PrivacyTooltipCam"),
                    &Strings::get("PrivacyTooltipMic"),
                    &Strings::get("PrivacyUnknownApp"),
                );
            }
        }
        self.changed.notify();
    }

    // ---- Pil ----

    fn update_battery_state(&self) {
        let (pct, charging, has) = {
            let s = self.status.state();
            (s.battery_percent, s.is_charging, s.has_battery)
        };
        {
            let mut st = self.state.borrow_mut();
            st.battery_percent = pct;
            st.is_charging = charging;
            st.has_battery = has;
            // Dış gövde 22 DIP, iç dolgu alanı 17 DIP.
            st.battery_fill_width = (17.0 * pct.clamp(0, 100) as f64 / 100.0).max(2.0);
        }
        if self.is_idle_ring() {
            self.update_battery_ring(pct.clamp(0, 100));
        }
    }

    fn update_battery_ring(&self, pct: i32) {
        let low = IdleDisplayFormat::is_low(pct, self.state.borrow().is_charging);
        self.state.borrow_mut().battery_ring_low = low;
        if pct == self.ring_percent.get() {
            return;
        }
        self.ring_percent.set(pct);
        let culture = strings::culture_for(&self.settings.borrow().general.language);
        let mut st = self.state.borrow_mut();
        st.battery_percent_text = IdleDisplayFormat::percent(pct, &culture);
        st.battery_ring_path = IdleDisplayFormat::arc_path(pct, IdleDisplayFormat::RING_SIZE, IdleDisplayFormat::RING_RADIUS);
    }

    pub fn battery_ring_color(&self) -> u32 {
        if self.state.borrow().battery_ring_low { RING_LOW } else { RING_NORMAL }
    }

    // ---- Saat ----

    fn start_clock_timer(self: &Rc<Self>) {
        if self.clock_timer.borrow().is_some() {
            return;
        }
        self.update_clock();
        self.schedule_clock();
        // Saat ya da saat dilimi elle değişirse bir sonraki dakikayı beklemeden düzelt.
        let w = Rc::downgrade(self);
        *self.time_changed_sub.borrow_mut() = Some(SystemEvents::on_time_changed(move || {
            if let Some(vm) = w.upgrade() {
                vm.on_clock_tick();
            }
        }));
    }

    fn schedule_clock(self: &Rc<Self>) {
        let delay = self.next_clock_delay();
        let w = Rc::downgrade(self);
        *self.clock_timer.borrow_mut() = Some(dispatch::delay(delay, move || {
            if let Some(vm) = w.upgrade()
                && vm.clock_timer.borrow().is_some()
            {
                vm.on_clock_tick();
            }
        }));
    }

    fn stop_clock_timer(&self) {
        *self.clock_timer.borrow_mut() = None;
        *self.time_changed_sub.borrow_mut() = None;
    }

    fn on_clock_tick(self: &Rc<Self>) {
        self.update_clock();
        self.changed.notify();
        if self.clock_timer.borrow().is_some() {
            self.schedule_clock();
        }
    }

    fn next_clock_delay(&self) -> Duration {
        if self.settings.borrow().notch.clock_show_seconds {
            Duration::from_secs(1)
        } else {
            let d = ClockTick::delay_to_next_minute(Local::now().naive_local());
            Duration::from_millis(d.total_milliseconds().max(0.0) as u64)
        }
    }

    fn update_clock(&self) {
        let now = Local::now();
        let naive = now.naive_local();
        let s = self.settings.borrow();
        let clock = &s.notch;
        let culture = strings::culture();
        let pattern = ClockFormatPolicy::resolve_time_pattern(clock.clock_hour_format, clock.clock_show_seconds, &culture);
        let time = koy_core::clock::format_date_time(naive, None, &pattern, &culture).unwrap_or_default();
        // İkinci saat ve tarih aynı saat tikinde güncellenir; ek zamanlayıcı yok.
        let zone = Some(clock.clock_second_time_zone_id.as_str()).filter(|z| !z.is_empty());
        let second = SecondClock::read_by_id(
            zone,
            now.fixed_offset(),
            &ClockFormatPolicy::resolve_time_pattern(clock.clock_hour_format, false, &culture),
            &culture,
            None,
        )
        .map(|r| r.text());
        let custom = Some(clock.clock_custom_date_format.as_str()).filter(|f| !f.is_empty());
        let short_date = IdleDisplay::show_collapsed_date(clock.idle_display, clock.clock_show_date)
            .then(|| IdleDisplayFormat::short_date(naive, &culture, custom));
        let is_ring = IdleDisplay::is_ring(clock.idle_display);
        let (day_time, peek_date) = if is_ring {
            let c = strings::culture_for(&s.general.language);
            let dt = IdleDisplayFormat::day_time(naive, &c, &ClockFormatPolicy::resolve_time_pattern(clock.clock_hour_format, clock.clock_show_seconds, &c));
            let mut pd = format!("· {}", IdleDisplayFormat::short_date(naive, &c, custom));
            if let Some(sc) = &second {
                pd += &format!(" · {sc}");
            }
            (Some(dt), Some(pd))
        } else {
            (None, None)
        };
        drop(s);
        let mut st = self.state.borrow_mut();
        st.clock_time = time;
        st.clock_date_suffix = ClockFormatPolicy::compose_peek_line("", short_date.as_deref(), None);
        st.clock_peek_extras = ClockFormatPolicy::compose_peek_line("", None, second.as_deref());
        if let (Some(dt), Some(pd)) = (day_time, peek_date) {
            st.idle_day_time = dt;
            st.idle_peek_date = pd;
        }
    }

    // ---- Durum ----

    fn apply_state(self: &Rc<Self>, new_state: NotchState) {
        let old = self.state.borrow().state;
        // Kullanıcının seçtiği Ana/Müzik yüzü korunur; parça yoksa ya da müzik kartı kapalıysa Ana yüzüne düşer.
        if new_state == NotchState::Expanded && old != NotchState::Expanded {
            let page = NotchHomePages::on_open(self.home.page(), self.music.state().has_track, self.home.cards.contains(&HomeCard::Music));
            self.home.set_page(page);
        }
        self.state.borrow_mut().state = new_state;
        // Kart çentik açılınca kalkar.
        if new_state == NotchState::Expanded && self.osd.kind() == OsdType::Card {
            self.osd.dismiss();
        }
        self.update_target_size();
        self.update_active_panels();
        self.visibility.set_expanded(new_state == NotchState::Expanded);
        self.changed.notify();
    }

    fn on_panel_changed(self: &Rc<Self>, panel: NotchPanel) {
        {
            let mut st = self.state.borrow_mut();
            for t in st.tabs.iter_mut() {
                t.is_selected = t.panel == panel;
            }
            st.current_panel = panel;
        }
        self.update_target_size();
        self.update_active_panels();
        self.changed.notify();
    }

    fn update_live_background(&self) {
        let st = self.state.borrow();
        self.live_background.update(st.state == NotchState::Expanded, st.is_notch_visible, self.reduce_motion());
    }

    // Sekmeler açık panellerden ve onların sırasından türer; liste değişince baştan kurulur.
    fn rebuild_tabs(&self) {
        let current = self.controller.current_panel();
        let tabs = self.controller.enabled_panels().into_iter().map(|p| create_tab(p, p == current)).collect();
        self.state.borrow_mut().tabs = tabs;
    }

    /// Üst satırın sağındaki sekmeler: Ana dışındaki açık paneller.
    pub fn side_tabs(&self) -> Vec<NotchTab> {
        self.state.borrow().tabs.iter().filter(|t| t.panel != NotchPanel::Home).cloned().collect()
    }

    /// Ortadaki anahtar: Ana sekmesine geçer ve istenen yüzü gösterir.
    pub fn show_home_page(&self, page: NotchHomePage) {
        self.home.set_page(page);
        self.controller.go_to_panel(NotchPanel::Home);
    }

    // Ana'daki etkinliğe tıklanınca Takvim o günde açılır.
    fn open_calendar_day(&self, day: chrono::NaiveDate) {
        self.calendar.select_day(day);
        self.calendar.set_view(koy_core::calendar::CalendarViewMode::Month);
        self.controller.go_to_panel(NotchPanel::Calendar);
    }

    fn update_active_panels(self: &Rc<Self>) {
        self.update_live_background();
        let (state, panel, visible) = {
            let st = self.state.borrow();
            (st.state, st.current_panel, st.is_notch_visible)
        };
        // Arama açıkken sayfalar görünmez; servisleri de çalışmaz.
        let is_expanded = state == NotchState::Expanded && !self.is_search_open();
        let (show_waveform, lyrics) = {
            let s = self.settings.borrow();
            (s.music.show_waveform, s.music.lyrics_enabled)
        };
        // Ses çubukları "kapak ve dalga" ayarına bağlı; pil tasarrufu hareketi kısarken hiç yakalanmaz.
        self.music.set_spectrum_allowed(show_waveform && !self.reduce_motion());
        if self.music.lyrics.is_enabled() != lyrics || !lyrics {
            self.music.lyrics.set_allowed(lyrics);
        }
        let on_home = panel == NotchPanel::Home;
        let shown = is_expanded && visible;
        self.music.set_active(shown && on_home && self.home.is_music_page());
        self.music.set_collapsed_spectrum_active(visible && state == NotchState::Collapsed && !self.osd.is_active());
        // Saat, hava ve pil her sekmede üstteki durum satırında görünür.
        self.status.set_active(is_expanded);
        self.control_center.set_active(is_expanded && panel == NotchPanel::ControlCenter);
        self.focus.set_viewing(is_expanded && panel == NotchPanel::Focus, visible);
        // Her servis yalnız sayfası görünürken çalışır (boşta %0 CPU).
        let home_face = shown && on_home && self.home.is_home_page();
        self.home.stats.set_active(home_face || (shown && panel == NotchPanel::System));
        self.home.quick_launch.set_active(home_face);
        self.home.set_active(home_face);
        self.control_center.bluetooth_devices.set_viewing("home", home_face);
        if let Some(t) = &self.focus.tasks {
            t.set_active(shown && panel == NotchPanel::Focus);
        }
        self.calendar.set_active(shown && panel == NotchPanel::Calendar);
        if let Some(c) = &self.clipboard {
            c.set_active(shown && panel == NotchPanel::Clipboard);
        }
    }

    fn apply_visibility(self: &Rc<Self>, result: VisibilityResult) {
        self.state.borrow_mut().is_notch_visible = result.is_visible;
        self.update_active_panels();
        if result.is_visible {
            self.start_clock_timer();
        } else {
            self.stop_clock_timer();
        }
        self.target_visibility_changed.invoke(&result.is_visible);
        self.changed.notify();
    }

    pub fn update_target_size(&self) {
        let style = self.settings.borrow().notch.style;
        let (w, h, r) = if self.osd.is_active() {
            let is_card = self.osd.kind() == OsdType::Card;
            let (w, h) = if is_card { (NotchLayout::CARD_WIDTH, NotchLayout::CARD_HEIGHT) } else { (260.0, 40.0) };
            let r = NotchStyle::corner_radius(style, if is_card { NotchSurface::Card } else { NotchSurface::Closed }, h, if is_card { 28.0 } else { 20.0 });
            (w, h, r)
        } else {
            let state = self.state.borrow().state;
            let (w, h, r) = match state {
                NotchState::Collapsed => (if self.is_music_playing() { 280.0 } else { 236.0 }, 34.0, 18.0),
                NotchState::Peek => (300.0, 38.0, 20.0),
                // Her sekmede aynı sabit gövde: sekme değişince çentik boyut değiştirmez.
                NotchState::Expanded => (NotchLayout::EXPANDED_WIDTH, NotchLayout::EXPANDED_HEIGHT, 34.0),
                #[allow(unreachable_patterns)]
                _ => (236.0, 34.0, 18.0),
            };
            let surface = if state == NotchState::Expanded { NotchSurface::Expanded } else { NotchSurface::Closed };
            (w, h, NotchStyle::corner_radius(style, surface, h, r))
        };
        {
            let mut st = self.state.borrow_mut();
            st.target_width = w;
            st.target_height = h;
            st.target_corner_radius = r;
        }
        self.update_surface();
        self.target_size_changed.invoke(&(w, h));
        self.changed.notify();
    }

    // ---- Malzeme (Katı / Buzlu / Cam) ----
    // Kapalı çentik, göz atma ve HUD hep opak taban renk; açık çentik ve kart etkin malzemenin opaklığında.

    fn update_glass_surface(&self) {
        let level = MaterialState::for_notch(&self.settings.borrow());
        let base = ThemeManager::color("KoyNotchSurface");
        let base = if base == 0 { DEFAULT_SURFACE } else { base };
        let rgb = base & 0x00FF_FFFF;
        let opaque = 0xFF00_0000 | rgb;
        self.opaque_surface.set(opaque);
        let alpha = MaterialSetting::layered_alpha(level);
        self.glass_surface.set(if level == MaterialLevel::Solid { opaque } else { ((alpha as u32) << 24) | rgb });
        // Albüm rengi müzik kartı da aynı opaklıkta.
        self.music.set_card_alpha(alpha);
    }

    fn is_card_shown(&self) -> bool {
        self.osd.is_active() && self.osd.kind() == OsdType::Card
    }

    fn update_surface(&self) {
        let (surface, gradient) = if self.osd.is_active() {
            (if self.osd.kind() == OsdType::Card { self.glass_surface.get() } else { self.opaque_surface.get() }, Vec::new())
        } else if self.state.borrow().state == NotchState::Expanded {
            (self.glass_surface.get(), Vec::new())
        } else {
            self.resolve_collapsed_surface()
        };
        let mut st = self.state.borrow_mut();
        st.surface = surface;
        st.surface_gradient = gradient;
    }

    /// Kapalı çentik zemini: Varsayılan, Albüm rengi (yalnız çalarken; duraklatılınca varsayılana döner) ya da Ambiyans.
    fn resolve_collapsed_surface(&self) -> (u32, Vec<u32>) {
        use koy_core::notch::NotchCollapsedBackground as B;
        match self.settings.borrow().notch.collapsed_background {
            B::AlbumColor => {
                let m = self.music.state();
                if m.has_track && m.is_playing && let Some(c) = m.paint.collapsed_album {
                    return (c.as_argb_encoded(), Vec::new());
                }
            }
            B::Ambient if !self.ambient_collapsed.is_empty() => {
                if self.ambient_collapsed.len() == 1 {
                    return (self.ambient_collapsed[0], Vec::new());
                }
                return (self.ambient_collapsed[0], self.ambient_collapsed.clone());
            }
            _ => {}
        }
        (self.opaque_surface.get(), Vec::new())
    }

    /// Kapalı çentikteki canlı ses çubuklarının yükseklikleri (px, 14 px kutu; en az 3 px). Yakalama yokken hepsi en
    /// küçük çubuktur. C# `SpectrumBars` (Bars).
    pub fn collapsed_bar_heights(&self) -> Vec<f32> {
        let count = NotchVisualizer::clamp_bar_count(self.settings.borrow().music.visualizer_bar_count).max(0) as usize;
        let mut levels = self.music.spectrum.sampled(count);
        levels.resize(count, 0.0);
        levels.into_iter().map(|l| (l * 14.0).round().max(3.0)).collect()
    }

    fn on_material_changed(&self) {
        self.update_glass_surface();
        self.update_surface();
        self.changed.notify();
    }

    // ---- Göstergeler ----

    pub fn show_volume(&self, volume: f32, muted: bool, me: &Rc<Self>) {
        let _ = self;
        me.osd.show_volume(volume, muted);
    }

    pub fn show_battery(&self, kind: BatteryAlertKind, percent: i32) {
        self.osd.show_battery(kind, percent);
    }

    pub fn show_bluetooth_device(&self, name: &str, connected: bool, battery: Option<i32>) {
        self.osd.show_bluetooth_device(name, connected, battery);
    }

    // ---- Arama ----

    /// Çentiği açıp arama panelini gösterir. `factory` yalnız ilk açılışta çalışır.
    pub fn open_search(self: &Rc<Self>, factory: impl FnOnce() -> Rc<SearchViewModel>, previous_window: isize, initial_query: Option<&str>) {
        if self.search.borrow().is_none() {
            let search = factory();
            let w = Rc::downgrade(self);
            search.close_requested.subscribe(move |restore: &bool| {
                if let Some(vm) = w.upgrade() {
                    vm.close_search(*restore);
                }
            });
            *self.search.borrow_mut() = Some(search);
        }
        self.window_before_search.set(previous_window);
        self.controller.expand(None);
        self.controller.hold_open();
        if let Some(s) = self.search() {
            s.open(initial_query);
        }
        self.update_active_panels();
        self.changed.notify();
    }

    pub fn close_search(self: &Rc<Self>, restore_focus: bool) {
        if !self.is_search_open() {
            return;
        }
        if let Some(s) = self.search() {
            s.close();
        }
        self.controller.release_hold();
        if !self.controller.is_pinned() {
            self.controller.on_click();
        }
        self.update_active_panels();
        self.changed.notify();
        self.search_closed.invoke(&if restore_focus { self.window_before_search.get() } else { 0 });
    }

    pub fn expand_to_shelf(&self) {
        self.controller.expand(Some(NotchPanel::Shelf));
    }

    pub fn next_panel(&self) {
        self.controller.next_panel();
    }

    pub fn previous_panel(&self) {
        self.controller.previous_panel();
    }

    pub fn select_tab(&self, panel: NotchPanel) {
        self.controller.go_to_panel(panel);
    }

    /// Kapalı çentikteki Wi-Fi / Bluetooth simgesine tıklanınca çentik Kontrol sekmesinde açılır.
    pub fn open_control_center(&self) {
        self.controller.expand(Some(NotchPanel::ControlCenter));
    }

    /// "Çentiği açık tut" düğmesi.
    pub fn toggle_pinned(&self) {
        self.controller.toggle_pinned();
    }

    pub fn open_settings(&self) {
        if let Some(f) = self.open_settings_handler.borrow().as_ref() {
            f();
        }
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.disposed.replace(true) {
            return;
        }
        self.stop_clock_timer();
        for unsub in self.core_subs.borrow_mut().drain(..) {
            unsub();
        }
        self.osd.dispose();
        self.controller.dispose();
        self.visibility.dispose();
        self.music.dispose();
        self.control_center.dispose();
        self.status.dispose();
        self.focus.dispose();
        self.shelf.dispose();
        self.calendar.dispose();
        if let Some(c) = &self.clipboard {
            c.dispose();
        }
        if let Some(s) = self.search() {
            s.dispose();
        }
        self.home.dispose();
    }
}

impl NotchViewModel {
    /// Görünürlük kipi (kenar gözetleme zamanlayıcısı için).
    pub fn visibility_mode(&self) -> VisibilityMode {
        self.visibility.mode()
    }
}


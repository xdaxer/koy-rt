//! Tam genişlik dock'un sağ bölümündeki durum: ağ, ses, pil, saat. Karşılığı: `DockStatusViewModel.cs`.
//! Saat dakika başına hizalı tek uyanmayla güncellenir; dock gizliyken zamanlayıcı durur.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, Timelike};

use koy_core::audio::IAudioService;
use koy_core::dock::TrayGlyphs;
use koy_core::dock::items::DockClockTick;
use koy_core::event::Subscription;
use koy_core::network::{INetworkService, NetworkKind, NetworkStatus};
use koy_core::power::IPowerService;
use koy_core::shell::IShellActionService;

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::{self, Strings};

#[derive(Debug, Clone, Default)]
pub struct DockStatusState {
    pub network_glyph: String,
    pub wifi_level_glyph: String,
    pub network_tooltip: String,
    pub volume_glyph: String,
    pub volume_tooltip: String,
    pub has_battery: bool,
    pub battery_glyph: String,
    pub battery_tooltip: String,
    pub time_text: String,
    pub date_text: String,
}

pub struct DockStatusViewModel {
    network: Arc<dyn INetworkService>,
    audio: Arc<dyn IAudioService>,
    power: Arc<dyn IPowerService>,
    shell: Arc<dyn IShellActionService>,
    state: RefCell<DockStatusState>,
    clock_timer: slint::Timer,
    active: Cell<bool>,
    disposed: Cell<bool>,
    subs: RefCell<Vec<(u8, Subscription)>>,
    me: RefCell<Weak<Self>>,
    pub changed: Notifier,
}

impl DockStatusViewModel {
    pub fn new(
        network: Arc<dyn INetworkService>,
        audio: Arc<dyn IAudioService>,
        power: Arc<dyn IPowerService>,
        shell: Arc<dyn IShellActionService>,
    ) -> Rc<Self> {
        let vm = Rc::new(DockStatusViewModel {
            network,
            audio,
            power,
            shell,
            state: RefCell::new(DockStatusState { network_glyph: TrayGlyphs::OFFLINE.into(), ..Default::default() }),
            clock_timer: slint::Timer::default(),
            active: Cell::new(false),
            disposed: Cell::new(false),
            subs: RefCell::new(Vec::new()),
            me: RefCell::new(Weak::new()),
            changed: Notifier::default(),
        });
        *vm.me.borrow_mut() = Rc::downgrade(&vm);
        // Olaylar servis iş parçacıklarından gelir: arayüze taşınır.
        let ui = UiRef::new(&vm);
        let n = vm.network.changed().subscribe(move |s| {
            let (ui, s) = (ui.clone(), s.clone());
            dispatch::post(move || {
                if let Some(vm) = ui.upgrade() {
                    vm.apply_network(&s);
                }
            });
        });
        let ui = UiRef::new(&vm);
        let a = vm.audio.state_changed().subscribe(move |e| {
            let (ui, v, m) = (ui.clone(), e.volume, e.is_muted);
            dispatch::post(move || {
                if let Some(vm) = ui.upgrade() {
                    vm.apply_audio(v, m);
                }
            });
        });
        let ui = UiRef::new(&vm);
        let p = vm.power.power_changed().subscribe(move |e| {
            let (ui, b, pc, c) = (ui.clone(), e.has_battery, e.remaining_percent, e.is_charging);
            dispatch::post(move || {
                if let Some(vm) = ui.upgrade() {
                    vm.apply_power(b, pc, c);
                }
            });
        });
        vm.subs.borrow_mut().extend([(0, n), (1, a), (2, p)]);
        vm.apply_network(&vm.network.current());
        vm.apply_audio(vm.audio.volume(), vm.audio.is_muted());
        vm.apply_power(vm.power.has_battery(), vm.power.remaining_percent(), vm.power.is_charging());
        vm.set_active(true);
        vm
    }

    pub fn state(&self) -> std::cell::Ref<'_, DockStatusState> {
        self.state.borrow()
    }

    pub fn notifications_glyph(&self) -> &'static str {
        TrayGlyphs::NOTIFICATIONS
    }

    pub fn set_active(&self, active: bool) {
        if self.active.get() == active || self.disposed.get() {
            return;
        }
        self.active.set(active);
        if active {
            self.update_clock();
            self.schedule_next_tick();
        } else {
            self.clock_timer.stop();
        }
    }

    /// Tekerlek: %2 adımla ses.
    pub fn adjust_volume(&self, wheel_delta: i32) {
        let step = if wheel_delta > 0 { 0.02f32 } else { -0.02f32 };
        self.audio.set_volume((self.audio.volume() + step).clamp(0.0, 1.0));
    }

    pub fn open_quick_settings(&self) {
        self.shell.open_quick_settings();
    }

    pub fn open_notification_center(&self) {
        self.shell.open_notification_center();
    }

    // Saniyede bir değil, bir sonraki dakika başında uyanır.
    fn schedule_next_tick(&self) {
        let now = Local::now();
        let ms = (60 - now.second() as u64) * 1000 - (now.timestamp_subsec_millis() as u64).min(999) + 50;
        let me = self.me.borrow().clone();
        self.clock_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(ms), move || {
            if let Some(vm) = me.upgrade() {
                vm.update_clock();
                vm.schedule_next_tick();
            }
        });
    }

    fn update_clock(&self) {
        let now = Local::now().naive_local();
        let culture = strings::culture();
        {
            let mut st = self.state.borrow_mut();
            st.time_text = DockClockTick::format_time(now, &culture);
            st.date_text = DockClockTick::format_date(now, &culture);
        }
        self.changed.notify();
    }

    fn apply_network(&self, status: &NetworkStatus) {
        {
            let mut st = self.state.borrow_mut();
            st.network_glyph = TrayGlyphs::network(status).into();
            st.wifi_level_glyph = TrayGlyphs::wifi_level(status).into();
            st.network_tooltip = Strings::get(match status {
                s if s.kind == NetworkKind::None => "NetworkDisconnected",
                s if !s.has_internet => "NetworkNoInternet",
                s if s.kind == NetworkKind::WiFi => "NetworkWiFi",
                s if s.kind == NetworkKind::Cellular => "NetworkCellular",
                _ => "NetworkEthernet",
            });
        }
        self.changed.notify();
    }

    fn apply_audio(&self, volume: f32, muted: bool) {
        {
            let mut st = self.state.borrow_mut();
            st.volume_glyph = TrayGlyphs::volume(volume, muted).into();
            st.volume_tooltip = if muted {
                Strings::get("VolumeMuted")
            } else {
                Strings::format("VolumeLevel", &[&((volume * 100.0).round() as i32).to_string()])
            };
        }
        self.changed.notify();
    }

    fn apply_power(&self, has_battery: bool, percent: i32, charging: bool) {
        {
            let mut st = self.state.borrow_mut();
            st.has_battery = has_battery;
            st.battery_glyph = TrayGlyphs::battery(percent, charging);
            st.battery_tooltip = Strings::format(if charging { "BatteryCharging" } else { "BatteryLevel" }, &[&percent.to_string()]);
        }
        self.changed.notify();
    }

    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        self.clock_timer.stop();
        for (kind, sub) in self.subs.borrow_mut().drain(..) {
            match kind {
                0 => self.network.changed().unsubscribe(sub),
                1 => self.audio.state_changed().unsubscribe(sub),
                _ => self.power.power_changed().unsubscribe(sub),
            }
        }
    }
}

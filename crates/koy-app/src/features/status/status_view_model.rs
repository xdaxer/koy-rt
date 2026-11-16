//! Saat, tarih, pil, ağ ve hava durumu. Ana görünümün durum sütunu ve ayrıntılı Durum paneli bunu paylaşır.
//! Saat zamanlayıcısı yalnız panel görünürken çalışır ve dakika başına bir kez tikler.
//! Karşılığı: `Features/Status/StatusViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, Timelike};
use koy_core::cancel::CancellationToken;
use koy_core::clock::{ClockFormatPolicy, SecondClock, format_date_time};
use koy_core::event::Subscription;
use koy_core::network::{INetworkService, NetworkDisplay, NetworkStatus, NetworkSummary};
use koy_core::notch::{DayPart, Greeting};
use koy_core::power::{BatterySaverPolicy, BatteryStatus, IPowerService, PowerChangedEventArgs};
use koy_core::system_stats::SystemFormat;
use koy_core::units::{TemperatureUnit, UnitConversion, WindSpeedFormatter};
use koy_core::weather::{IPlaceNameService, IWeatherService, PlaceNameParser, WeatherInfo, WeatherLocationPolicy, WeatherLocationSource};

use crate::infra::dispatch;
use crate::infra::observable::{Notifier, SharedSettings};
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::{self, Strings};

/// Görünümün okuduğu durum (C#'taki gözlemlenebilir özellikler).
#[derive(Debug, Clone, Default)]
pub struct StatusState {
    pub current_time: String,
    pub current_date: String,
    /// Kısa tarih ("6 Kas Cum"): ana görünümün durum sütunu için.
    pub short_date: String,
    /// "Perşembe, 6 Kasım": açık çentiğin durum satırında ve boşta Ana kartında saatin altında.
    pub day_date: String,
    /// Tarihin ardındaki ikinci saat (" · UTC+9 21:00"); ikinci saat kapalıysa boş.
    pub second_clock_suffix: String,
    /// Günün dilimine göre selamlama.
    pub greeting: String,
    /// Uzun tarih ("6 Kasım 2026 Cuma").
    pub full_date: String,
    pub has_battery: bool,
    pub battery_percent: i32,
    pub is_charging: bool,
    pub battery_text: String,
    /// Cihaz şu an pille (şarj olmadan) çalışıyor mu.
    pub is_on_battery: bool,
    /// Hava durumu için bir konum var mı.
    pub has_weather_location: bool,
    /// Hava verisi geldi mi.
    pub has_weather: bool,
    pub temperature_text: String,
    pub temperature_short: String,
    pub wind_tool_tip: String,
    pub weather_description: String,
    pub weather_icon_kind: String,
    pub city: String,
    pub network_display: NetworkDisplay,
    pub network_text: String,
    pub network_is_connected: bool,
    pub has_network_info: bool,
}

impl StatusState {
    /// Sıcaklığın üstünde küçük yazılan yer adı: şehir ya da "Konumum".
    pub fn place_name(&self) -> String {
        if self.city.is_empty() { Strings::get("WeatherPlaceDevice") } else { self.city.clone() }
    }
}

pub struct StatusViewModel {
    power: Arc<dyn IPowerService>,
    weather: Arc<dyn IWeatherService>,
    network: Option<Arc<dyn INetworkService>>,
    place_names: Option<Arc<dyn IPlaceNameService>>,
    settings: SharedSettings,
    state: RefCell<StatusState>,
    pub changed: Notifier,
    clock_timer: RefCell<Option<slint::Timer>>,
    is_active: Cell<bool>,
    disposed: Cell<bool>,
    // Cihaz konumunun çözülen yer adı ve hangi koordinat için çözüldüğü.
    device_place: RefCell<Option<(String, (f64, f64))>>,
    subs: RefCell<Vec<Box<dyn FnOnce()>>>,
}

impl StatusViewModel {
    pub fn new(
        power: Arc<dyn IPowerService>,
        weather: Arc<dyn IWeatherService>,
        settings: SharedSettings,
        network: Option<Arc<dyn INetworkService>>,
        place_names: Option<Arc<dyn IPlaceNameService>>,
    ) -> Rc<Self> {
        let vm = Rc::new(StatusViewModel {
            power,
            weather,
            network,
            place_names,
            settings,
            state: RefCell::new(StatusState { temperature_short: "--°".into(), weather_icon_kind: "Cloud".into(), battery_percent: 100, ..Default::default() }),
            changed: Notifier::default(),
            clock_timer: RefCell::new(None),
            is_active: Cell::new(false),
            disposed: Cell::new(false),
            device_place: RefCell::new(None),
            subs: RefCell::new(Vec::new()),
        });
        vm.state.borrow_mut().has_network_info = vm.network.is_some();
        vm.update_location_state();
        vm.update_clock();
        vm.update_power_state();
        vm.update_weather_state(vm.weather.current_weather());

        let me = UiRef::new(&vm);
        let power = Arc::clone(&vm.power);
        let sub: Subscription = power.power_changed().subscribe(move |e: &PowerChangedEventArgs| {
            let e = *e;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    vm.on_power_changed(e);
                }
            });
        });
        vm.subs.borrow_mut().push(Box::new(move || power.power_changed().unsubscribe(sub)));

        let weather = Arc::clone(&vm.weather);
        let sub = weather.weather_changed().subscribe(move |w: &Option<WeatherInfo>| {
            let w = w.clone();
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    vm.update_weather_state(w);
                    vm.changed.notify();
                }
            });
        });
        vm.subs.borrow_mut().push(Box::new(move || weather.weather_changed().unsubscribe(sub)));

        if let Some(network) = vm.network.clone() {
            vm.apply_network(&network.current());
            let sub = network.changed().subscribe(move |s: &NetworkStatus| {
                let s = s.clone();
                // Olay Windows'un iş parçacığından gelir.
                dispatch::post(move || {
                    if let Some(vm) = me.upgrade()
                        && !vm.disposed.get()
                    {
                        vm.apply_network(&s);
                        vm.changed.notify();
                    }
                });
            });
            vm.subs.borrow_mut().push(Box::new(move || network.changed().unsubscribe(sub)));
        }
        // Hava durumu panel açılınca istenir (set_active): panel hiç açılmayabilir.
        vm
    }

    pub fn state(&self) -> std::cell::Ref<'_, StatusState> {
        self.state.borrow()
    }

    pub fn set_active(self: &Rc<Self>, active: bool) {
        if self.is_active.get() == active {
            return;
        }
        self.is_active.set(active);
        if active {
            self.update_clock();
            self.schedule_next_clock_tick();
            // Panel açıldığında hava durumunu tazele (önbellek kontrolü servis içindedir).
            self.request_weather();
            self.changed.notify();
        } else {
            *self.clock_timer.borrow_mut() = None;
        }
    }

    /// Konum ayarı değişti: hava durumunu yeniden kurar.
    pub fn on_location_changed(self: &Rc<Self>) {
        if self.disposed.get() {
            return;
        }
        self.update_location_state();
        self.update_weather_state(self.weather.current_weather());
        if self.is_active.get() {
            self.request_weather();
        }
        self.changed.notify();
    }

    fn update_location_state(&self) {
        let s = self.settings.borrow();
        let has = WeatherLocationPolicy::has_location(&s);
        let city = match s.weather.location_source {
            WeatherLocationSource::City => city_short_name(&s.weather.city),
            WeatherLocationSource::Device => match &*self.device_place.borrow() {
                Some((name, at)) if *at == PlaceNameParser::round(s.weather.latitude, s.weather.longitude) => name.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        let mut st = self.state.borrow_mut();
        st.has_weather_location = has;
        st.city = city;
    }

    /// Cihaz konumu kullanılıyorsa yer adını çözer (servis aynı koordinat için tek istek atar).
    fn request_device_place_name(self: &Rc<Self>) {
        let Some(service) = self.place_names.clone() else { return };
        let (lat, lon, language) = {
            let s = self.settings.borrow();
            if s.weather.location_source != WeatherLocationSource::Device {
                return;
            }
            (s.weather.latitude, s.weather.longitude, s.general.language.clone())
        };
        let coordinate = PlaceNameParser::round(lat, lon);
        if self.device_place.borrow().as_ref().is_some_and(|(_, at)| *at == coordinate) {
            return;
        }
        let me = UiRef::new(self);
        std::thread::spawn(move || {
            let name = service.get_place_name(lat, lon, &language, &CancellationToken::none()).ok().flatten();
            dispatch::post(move || {
                let Some(vm) = me.upgrade() else { return };
                let Some(name) = name.filter(|n| !n.is_empty()) else { return };
                {
                    let s = vm.settings.borrow();
                    if vm.disposed.get()
                        || s.weather.location_source != WeatherLocationSource::Device
                        || PlaceNameParser::round(s.weather.latitude, s.weather.longitude) != coordinate
                    {
                        return;
                    }
                }
                *vm.device_place.borrow_mut() = Some((name, coordinate));
                vm.update_location_state();
                vm.changed.notify();
            });
        });
    }

    fn request_weather(self: &Rc<Self>) {
        if !self.state.borrow().has_weather_location {
            return;
        }
        let (lat, lon) = {
            let s = self.settings.borrow();
            (s.weather.latitude, s.weather.longitude)
        };
        let weather = Arc::clone(&self.weather);
        std::thread::spawn(move || {
            let _ = weather.get_current_weather(lat, lon, &CancellationToken::none());
        });
        self.request_device_place_name();
    }

    /// Saat yalnız HH:mm gösterir: bir sonraki dakika başına kurulan tek atımlık zamanlayıcı.
    fn schedule_next_clock_tick(self: &Rc<Self>) {
        let now = Local::now();
        let until = Duration::from_secs(60 - now.second() as u64) - Duration::from_millis((now.nanosecond() / 1_000_000).min(999) as u64)
            + Duration::from_millis(50);
        let weak = Rc::downgrade(self);
        let timer = dispatch::delay(until, move || {
            if let Some(vm) = weak.upgrade() {
                vm.update_clock();
                vm.changed.notify();
                if vm.is_active.get() {
                    vm.schedule_next_clock_tick();
                }
            }
        });
        *self.clock_timer.borrow_mut() = Some(timer);
    }

    fn update_clock(&self) {
        let now = Local::now();
        let s = self.settings.borrow();
        let culture = strings::culture_for(&s.general.language);
        // 12/24 saat ayarı; saniye yalnız kapalı çentik saatinde.
        let pattern = ClockFormatPolicy::resolve_time_pattern(s.notch.clock_hour_format, false, &culture);
        let naive = now.naive_local();
        let fmt = |f: &str| format_date_time(naive, None, f, &culture).unwrap_or_default();
        let second = SecondClock::read_by_id(Some(s.notch.clock_second_time_zone_id.as_str()).filter(|z| !z.is_empty()), now.fixed_offset(), &pattern, &culture, None)
            .map(|r| r.text());
        let mut st = self.state.borrow_mut();
        st.current_time = fmt(&pattern);
        st.current_date = fmt("d MMMM dddd");
        st.short_date = fmt("d MMM ddd");
        st.full_date = fmt("d MMMM yyyy dddd");
        st.day_date = fmt("dddd, d MMMM");
        st.second_clock_suffix = ClockFormatPolicy::compose_peek_line("", None, second.as_deref());
        st.greeting = Strings::get(match Greeting::for_hour(now.hour() as i32) {
            DayPart::Morning => "GreetMorning",
            DayPart::Afternoon => "GreetAfternoon",
            DayPart::Evening => "GreetEvening",
            _ => "GreetNight",
        });
    }

    fn battery_text(percent: i32, charging: bool) -> String {
        let p = SystemFormat::percent(percent as f64, &strings::culture());
        if charging { format!("{p} ({})", Strings::get("Charging")) } else { p }
    }

    fn on_power_changed(&self, e: PowerChangedEventArgs) {
        {
            let mut st = self.state.borrow_mut();
            st.has_battery = e.has_battery;
            st.battery_percent = e.remaining_percent;
            st.is_charging = e.is_charging;
            st.battery_text = Self::battery_text(e.remaining_percent, e.is_charging);
        }
        self.set_battery_status(e.status);
        self.changed.notify();
    }

    fn update_power_state(&self) {
        {
            let mut st = self.state.borrow_mut();
            st.has_battery = self.power.has_battery();
            st.battery_percent = self.power.remaining_percent();
            st.is_charging = self.power.is_charging();
            st.battery_text = Self::battery_text(st.battery_percent, st.is_charging);
        }
        self.set_battery_status(self.power.status());
    }

    fn set_battery_status(&self, status: BatteryStatus) {
        let mut st = self.state.borrow_mut();
        st.is_on_battery = BatterySaverPolicy::should_reduce_motion(true, st.has_battery, status);
    }

    fn apply_network(&self, status: &NetworkStatus) {
        let display = NetworkSummary::describe(status);
        let mut st = self.state.borrow_mut();
        st.network_display = display;
        st.network_is_connected = NetworkSummary::is_connected(status);
        st.network_text = match display {
            NetworkDisplay::WiFi => NetworkSummary::wifi_name(status).unwrap_or_else(|| Strings::get("NetworkWiFi")),
            NetworkDisplay::Ethernet => Strings::get("NetworkEthernetShort"),
            NetworkDisplay::Cellular => Strings::get("NetworkCellularShort"),
            NetworkDisplay::NoInternet => Strings::get("NetworkNoInternet"),
            _ => Strings::get("NetworkDisconnected"),
        };
    }

    fn temperature_suffix(&self) -> String {
        Strings::get(if self.settings.borrow().general.temperature_unit == TemperatureUnit::Fahrenheit { "UnitFahrenheitSuffix" } else { "UnitCelsiusSuffix" })
    }

    fn update_weather_state(&self, weather: Option<WeatherInfo>) {
        let suffix = self.temperature_suffix();
        let (unit, wind_unit) = {
            let s = self.settings.borrow();
            (s.general.temperature_unit, s.general.wind_speed_unit)
        };
        let mut st = self.state.borrow_mut();
        let Some(w) = weather else {
            st.has_weather = false;
            st.temperature_text = format!("--{suffix}");
            st.temperature_short = "--°".into();
            st.weather_description = Strings::get("Loading");
            st.weather_icon_kind = "Cloud".into();
            st.wind_tool_tip = String::new();
            return;
        };
        st.has_weather = true;
        // Hava servisi Celsius verir; seçilen birime Core çevirir (C# Math.Round: bankacı).
        let degrees = koy_core::text::round(UnitConversion::temperature(w.temperature, unit));
        st.temperature_text = format!("{degrees}{suffix}");
        st.temperature_short = format!("{degrees}°");
        st.weather_description = Strings::get(&w.description_key);
        st.weather_icon_kind = w.icon_kind.clone();
        let wind = w.wind_speed_kmh.and_then(|kmh| WindSpeedFormatter::format(kmh, wind_unit, &strings::culture()));
        st.wind_tool_tip = wind.map(|w| Strings::format("WeatherWindTooltip", &[&w])).unwrap_or_default();
    }

    /// "Hava durumunu yenile" komutu.
    pub fn refresh_weather(&self) {
        if !self.state.borrow().has_weather_location {
            return;
        }
        let (lat, lon) = {
            let s = self.settings.borrow();
            (s.weather.latitude, s.weather.longitude)
        };
        let weather = Arc::clone(&self.weather);
        std::thread::spawn(move || {
            let _ = weather.refresh(lat, lon, &CancellationToken::none());
        });
    }

    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        self.is_active.set(false);
        *self.clock_timer.borrow_mut() = None;
        for unsub in self.subs.borrow_mut().drain(..) {
            unsub();
        }
    }
}

// "Ankara, Türkiye" -> "Ankara": dar sütunda yalnız şehir adı.
fn city_short_name(display_name: &str) -> String {
    match display_name.find(',') {
        Some(i) if i > 0 => display_name[..i].to_string(),
        _ => display_name.to_string(),
    }
}

//! Open-Meteo anlık hava durumu; konum başına önbellekli.

use std::sync::{Arc, Mutex, MutexGuard};

use koy_core::cancel::{CancellationToken, OperationCanceled};
use koy_core::event::Event;
use koy_core::time::{DateTimeOffset, SystemTimeProvider, TimeProvider, TimeSpan, span_between};
use koy_core::weather::{IWeatherService, OpenMeteoParser, WeatherCodeMapper, WeatherInfo};

use crate::net::HttpClient;

struct State {
    cached_weather: Option<WeatherInfo>,
    cached_latitude: Option<f64>,
    cached_longitude: Option<f64>,
    // None = DateTimeOffset.MinValue.
    last_fetch_time: Option<DateTimeOffset>,
}

pub struct WeatherService {
    cache_duration: TimeSpan,
    http: &'static HttpClient,
    time_provider: Arc<dyn TimeProvider>,
    state: Mutex<State>,
    weather_changed: Event<Option<WeatherInfo>>,
}

impl WeatherService {
    /// `cache_duration`: önbellek süresi (ayar: hava durumunu yenileme sıklığı); verilmezse 30 dakika.
    pub fn new(http: Option<&'static HttpClient>, time_provider: Option<Arc<dyn TimeProvider>>, cache_duration: Option<TimeSpan>) -> Self {
        WeatherService {
            cache_duration: cache_duration.unwrap_or(TimeSpan::from_mins(30)),
            http: http.unwrap_or_else(HttpClient::shared),
            time_provider: time_provider.unwrap_or_else(|| Arc::new(SystemTimeProvider)),
            state: Mutex::new(State { cached_weather: None, cached_latitude: None, cached_longitude: None, last_fetch_time: None }),
            weather_changed: Event::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fetch(&self, latitude: f64, longitude: f64, ct: &CancellationToken) -> Option<String> {
        // ~1 km: tahmin ızgarası zaten km ölçeğinde, tam konum gitmez. C# "F2" (kültürsüz).
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={latitude:.2}&longitude={longitude:.2}&current=temperature_2m,weather_code,wind_speed_10m"
        );
        let mut response = self.http.get(&url, ct).ok()?;
        if !response.is_success() {
            return None;
        }
        response.text().ok()
    }
}

impl IWeatherService for WeatherService {
    fn current_weather(&self) -> Option<WeatherInfo> {
        self.lock().cached_weather.clone()
    }

    fn weather_changed(&self) -> &Event<Option<WeatherInfo>> {
        &self.weather_changed
    }

    fn get_current_weather(&self, latitude: f64, longitude: f64, ct: &CancellationToken) -> Result<Option<WeatherInfo>, OperationCanceled> {
        let now = self.time_provider.utc_now();
        let cleared;
        {
            let mut st = self.lock();
            let is_same_location = matches!((st.cached_latitude, st.cached_longitude),
                (Some(la), Some(lo)) if (la - latitude).abs() < 0.001 && (lo - longitude).abs() < 0.001);

            if st.cached_weather.is_some()
                && is_same_location
                && st.last_fetch_time.is_some_and(|t| span_between(now, t) < self.cache_duration)
            {
                return Ok(st.cached_weather.clone());
            }

            // Konum değiştiyse eski konumun havası yeni konum gibi gösterilmesin.
            cleared = !is_same_location && st.cached_weather.is_some();
            if cleared {
                st.cached_weather = None;
            }
        }
        if cleared {
            self.weather_changed.invoke(&None);
        }

        // Ağ hatası veya zaman aşımında önbellekteki değer döner.
        let Some(json) = self.fetch(latitude, longitude, ct) else { return Ok(self.current_weather()) };
        let Some((temperature, code)) = OpenMeteoParser::parse_current(Some(&json)) else { return Ok(self.current_weather()) };

        let info = WeatherInfo::new(
            temperature,
            code,
            WeatherCodeMapper::to_description_key(code),
            WeatherCodeMapper::to_icon_kind(code),
            now,
            OpenMeteoParser::parse_wind_speed(Some(&json)),
        );
        {
            let mut st = self.lock();
            st.cached_weather = Some(info.clone());
            st.cached_latitude = Some(latitude);
            st.cached_longitude = Some(longitude);
            st.last_fetch_time = Some(now);
        }
        self.weather_changed.invoke(&Some(info.clone()));
        Ok(Some(info))
    }

    fn refresh(&self, latitude: f64, longitude: f64, ct: &CancellationToken) -> Result<(), OperationCanceled> {
        // Önbellek süresini sıfırlayarak taze veri al.
        self.lock().last_fetch_time = None;
        self.get_current_weather(latitude, longitude, ct).map(|_| ())
    }

    fn dispose(&self) {}
}

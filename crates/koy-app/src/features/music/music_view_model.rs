//! Müzik paneli görünüm modeli: medya servisi ile arayüz arasındaki köprü. İlerleme zamanlayıcısı yalnız panel
//! görünürken ve müzik çalarken çalışır (%0 boşta CPU). Kapak rengi (albüm rengi / kapak resmi stili) arka planda
//! hesaplanır. Karşılığı: `Features/Music/MusicViewModel.cs` ve `CoverSampler.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use koy_core::appearance::Rgb;
use koy_core::event::Subscription;
use koy_core::lyrics::ILyricsProvider;
use koy_core::media::{
    CoverFingerprint, CoverPalette, CoverPaletteExtractor, IMediaService, MediaAppName, MediaInfo, MediaSessionInfo, MusicCardStyle,
    PlaybackClock, PlaybackTimeline,
};
use koy_core::notch::{NotchVisualizer, NotchVisualizerColorSource, NotchVisualizerStyle};
use koy_core::time::TimeSpan;
use koy_platform::audio::SpectrumCaptureService;

use super::lyrics_view_model::LyricsViewModel;
use super::spectrum_feed::{SpectrumFeed, SpectrumPusher};
use crate::infra::dispatch;
use crate::infra::image_decode::{self, Bgra};
use crate::infra::observable::{LocalEvent, Notifier};
use crate::infra::ui_ref::UiRef;

/// Büyük müzik kartında kapak 172 DIP; %150 ölçekte ~260 piksel gerekir. Daha büyük kaynak bu genişliğe indirilir.
const MAX_COVER_PIXELS: u32 = 320;
/// Palet ve kapak resmi zemini için ikinci çözüm genişliği.
pub const SAMPLE_PIXELS: u32 = 32;

/// Kart boyası (C#'taki dondurulmuş fırçaların karşılığı; görünüm bunlardan çizer).
#[derive(Clone, Default)]
pub struct CardPaint {
    /// Kart kapaktan boyanıyor (stil Düz değil ve kapak renkli).
    pub has_tint: bool,
    /// Kartın zemini küçültülmüş kapak (Kapak resmi stili, renkli kapak).
    pub has_backdrop: bool,
    /// Kart zemini (alfa çentik malzemesine göre).
    pub card: Option<slint::Color>,
    /// Çok renkli kapakta köşelerden yayılan renkler (en çok 4; alfa uygulanmış).
    pub blend: Vec<slint::Color>,
    pub sub_text: Option<slint::Color>,
    pub spectrum: Option<slint::Color>,
    /// Kapak arka planının altındaki hafif ton (Card, alfa 0,30).
    pub tint_overlay: Option<slint::Color>,
    /// 32 px kapak; kart büyütür.
    pub backdrop: Option<slint::Image>,
}

#[derive(Clone, Default)]
pub struct MusicState {
    pub has_track: bool,
    pub title: String,
    pub artist: String,
    pub source_app_id: String,
    pub is_playing: bool,
    pub thumbnail: Option<slint::Image>,
    /// Kapak yatay mı (video küçük resmi); yataysa kare alana sığdırılır, arkası bulanık kapakla dolar.
    pub is_wide_cover: bool,
    pub position: TimeSpan,
    pub duration: TimeSpan,
    pub progress: f64,
    pub position_text: String,
    pub duration_text: String,
    pub sessions: Vec<MediaSessionInfo>,
    pub show_spectrum: bool,
    pub paint: CardPaint,
}

impl MusicState {
    /// Süresi olmayan oynatım (canlı yayın): ilerleme çubuğu yerine CANLI rozeti.
    pub fn is_live(&self) -> bool {
        self.has_track && self.duration <= TimeSpan::ZERO
    }
    pub fn has_multiple_sessions(&self) -> bool {
        self.sessions.len() > 1
    }
    pub fn show_collapsed_media(&self) -> bool {
        self.has_track && self.is_playing
    }
}

pub type SharedMedia = Arc<dyn IMediaService>;

pub struct MusicOptions {
    pub spectrum_acquire: Option<Box<dyn Fn() -> Arc<SpectrumCaptureService>>>,
    pub spectrum_release: Option<Box<dyn Fn()>>,
    pub lyrics_provider: Option<Arc<dyn ILyricsProvider>>,
    pub card_style: MusicCardStyle,
    pub visualizer_style: NotchVisualizerStyle,
    pub visualizer_color_source: NotchVisualizerColorSource,
    pub visualizer_bar_count: i32,
}

impl Default for MusicOptions {
    fn default() -> Self {
        MusicOptions {
            spectrum_acquire: None,
            spectrum_release: None,
            lyrics_provider: None,
            card_style: MusicCardStyle::Tinted,
            visualizer_style: NotchVisualizerStyle::Bars,
            visualizer_color_source: NotchVisualizerColorSource::Accent,
            visualizer_bar_count: 5,
        }
    }
}

pub struct MusicViewModel {
    media: SharedMedia,
    state: RefCell<MusicState>,
    pub changed: Notifier,
    pub track_changed: LocalEvent<()>,
    pub lyrics: Rc<LyricsViewModel>,
    pub spectrum: Rc<SpectrumFeed>,
    pub visualizer_style: NotchVisualizerStyle,
    pub visualizer_color_source: NotchVisualizerColorSource,
    pub visualizer_bar_count: i32,
    spectrum_acquire: Option<Box<dyn Fn() -> Arc<SpectrumCaptureService>>>,
    spectrum_release: Option<Box<dyn Fn()>>,
    spectrum_service: RefCell<Option<(Arc<SpectrumCaptureService>, Subscription, SpectrumPusher)>>,
    spectrum_allowed: Cell<bool>,
    card_style: Cell<MusicCardStyle>,
    cover_key: RefCell<Option<CoverFingerprint>>,
    palette_generation: Cell<u64>,
    card_alpha: Cell<u8>,
    palette: RefCell<Option<CoverPalette>>,
    progress_timer: RefCell<Option<slint::Timer>>,
    is_active: Cell<bool>,
    disposed: Cell<bool>,
    subs: RefCell<Vec<Box<dyn FnOnce()>>>,
}

impl MusicViewModel {
    pub fn new(media: SharedMedia, options: MusicOptions) -> Rc<Self> {
        let lyrics = LyricsViewModel::new(Arc::clone(&media), options.lyrics_provider);
        let vm = Rc::new(MusicViewModel {
            media: Arc::clone(&media),
            state: RefCell::new(MusicState { position_text: "0:00".into(), duration_text: "0:00".into(), ..Default::default() }),
            changed: Notifier::default(),
            track_changed: LocalEvent::default(),
            lyrics,
            spectrum: SpectrumFeed::new(koy_platform::audio::BAND_COUNT as usize),
            visualizer_style: options.visualizer_style,
            visualizer_color_source: options.visualizer_color_source,
            visualizer_bar_count: NotchVisualizer::clamp_bar_count(options.visualizer_bar_count),
            spectrum_acquire: options.spectrum_acquire,
            spectrum_release: options.spectrum_release,
            spectrum_service: RefCell::new(None),
            spectrum_allowed: Cell::new(false),
            card_style: Cell::new(options.card_style),
            cover_key: RefCell::new(None),
            palette_generation: Cell::new(0),
            card_alpha: Cell::new(255),
            palette: RefCell::new(None),
            progress_timer: RefCell::new(None),
            is_active: Cell::new(false),
            disposed: Cell::new(false),
            subs: RefCell::new(Vec::new()),
        });

        // Sözler çubukların yerini alır: görünmeyen çubuk için ses yakalanmaz.
        let weak = Rc::downgrade(&vm);
        let lyr_sub = vm.lyrics.changed.subscribe(move || {
            if let Some(vm) = weak.upgrade() {
                let was = vm.state.borrow().show_spectrum;
                vm.set_spectrum_allowed(vm.spectrum_allowed.get());
                if was != vm.state.borrow().show_spectrum {
                    vm.changed.notify();
                }
            }
        });
        let lyrics = Rc::clone(&vm.lyrics);
        vm.subs.borrow_mut().push(Box::new(move || lyrics.changed.unsubscribe(lyr_sub)));

        let me = UiRef::new(&vm);
        let m = Arc::clone(&media);
        let s = m.changed().subscribe(move |info: &Option<MediaInfo>| {
            let info = info.clone();
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    let old = vm.state.borrow().title.clone();
                    vm.apply_media_info(info.as_ref());
                    let (has, title) = {
                        let st = vm.state.borrow();
                        (st.has_track, st.title.clone())
                    };
                    vm.changed.notify();
                    if has && old != title {
                        vm.track_changed.invoke(&());
                    }
                }
            });
        });
        vm.subs.borrow_mut().push(Box::new(move || m.changed().unsubscribe(s)));
        let m = Arc::clone(&media);
        let s = m.timeline_changed().subscribe(move |t: &Option<PlaybackTimeline>| {
            let t = *t;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    vm.apply_timeline(t.as_ref());
                    vm.changed.notify();
                }
            });
        });
        vm.subs.borrow_mut().push(Box::new(move || m.timeline_changed().unsubscribe(s)));
        let m = Arc::clone(&media);
        let s = m.sessions_changed().subscribe(move |list: &Vec<MediaSessionInfo>| {
            let list = list.clone();
            dispatch::post(move || {
                if let Some(vm) = me.upgrade() {
                    vm.state.borrow_mut().sessions = list;
                    vm.changed.notify();
                }
            });
        });
        vm.subs.borrow_mut().push(Box::new(move || m.sessions_changed().unsubscribe(s)));

        if let Some(info) = media.current() {
            vm.apply_media_info(Some(&info));
        }
        if let Some(t) = media.timeline() {
            vm.apply_timeline(Some(&t));
        }
        vm.state.borrow_mut().sessions = media.sessions();
        vm
    }

    pub fn state(&self) -> std::cell::Ref<'_, MusicState> {
        self.state.borrow()
    }

    pub fn set_active(self: &Rc<Self>, active: bool) {
        self.is_active.set(active);
        self.lyrics.set_panel_active(active);
        self.update_progress_timer();
    }

    /// Ses çubukları izinli mi (Çentik ayarındaki "kapak ve dalga" açık ve pil tasarrufu hareketi kısmıyor).
    pub fn set_spectrum_allowed(self: &Rc<Self>, allowed: bool) {
        self.spectrum_allowed.set(allowed);
        self.state.borrow_mut().show_spectrum = allowed && self.spectrum_acquire.is_some() && !self.lyrics.is_shown();
        self.update_spectrum();
    }

    // Yakalama yalnız kart görünür (çentik açık, Ana sekmesi) ve müzik çalarken sürer; aksi halde %0 CPU.
    fn update_spectrum(self: &Rc<Self>) {
        let want = {
            let st = self.state.borrow();
            !self.disposed.get() && self.is_active.get() && st.has_track && st.is_playing && st.show_spectrum
        };
        let has = self.spectrum_service.borrow().is_some();
        if want && !has {
            if let Some(acquire) = &self.spectrum_acquire {
                let service = acquire();
                let pusher = self.spectrum.pusher();
                let p = pusher.clone();
                let sub = service.levels_updated().subscribe(move |levels: &Vec<f32>| p.push(levels));
                *self.spectrum_service.borrow_mut() = Some((service, sub, pusher));
            }
        } else if !want && has {
            if let Some((service, sub, pusher)) = self.spectrum_service.borrow_mut().take() {
                service.levels_updated().unsubscribe(sub);
                drop(service);
                if let Some(release) = &self.spectrum_release {
                    release();
                }
                pusher.clear();
            }
        }
    }

    fn update_progress_timer(self: &Rc<Self>) {
        self.update_spectrum();
        let run = {
            let st = self.state.borrow();
            self.is_active.get() && st.has_track && st.is_playing
        };
        if run {
            if self.progress_timer.borrow().is_none() {
                let weak = Rc::downgrade(self);
                let timer = slint::Timer::default();
                timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || {
                    if let Some(vm) = weak.upgrade() {
                        vm.on_progress_tick();
                    }
                });
                *self.progress_timer.borrow_mut() = Some(timer);
            }
        } else {
            *self.progress_timer.borrow_mut() = None;
        }
    }

    fn on_progress_tick(&self) {
        {
            let mut st = self.state.borrow_mut();
            if !st.is_playing || st.duration <= TimeSpan::ZERO {
                return;
            }
            let mut next = st.position + TimeSpan::from_secs(1);
            if next > st.duration {
                next = st.duration;
            }
            st.position = next;
            st.progress = (next.total_seconds() / st.duration.total_seconds()).clamp(0.0, 1.0);
            st.position_text = format_time(next);
        }
        self.changed.notify();
    }

    // ---- Komutlar (servis çağrıları arayüzü bekletmesin) ----

    fn run(&self, f: impl FnOnce(&dyn IMediaService) + Send + 'static) {
        let media = Arc::clone(&self.media);
        std::thread::spawn(move || f(&*media));
    }

    pub fn play_pause(&self) {
        self.run(|m| m.play_pause());
    }

    pub fn next(&self) {
        self.run(|m| m.next());
    }

    pub fn previous(&self) {
        self.run(|m| m.previous());
    }

    pub fn seek(&self, fraction: f64) {
        let target = {
            let mut st = self.state.borrow_mut();
            if st.duration <= TimeSpan::ZERO {
                return;
            }
            let clamped = fraction.clamp(0.0, 1.0);
            let target = TimeSpan::from_ticks((st.duration.ticks() as f64 * clamped) as i64);
            st.position = target;
            st.progress = clamped;
            st.position_text = format_time(target);
            target
        };
        self.changed.notify();
        self.run(move |m| m.seek(target));
    }

    pub fn select_session(&self, session_id: &str) {
        let id = session_id.to_string();
        self.run(move |m| m.select_session(&id));
    }

    // ---- Medya bilgisi ----

    fn apply_media_info(self: &Rc<Self>, info: Option<&MediaInfo>) {
        match info {
            Some(info) if !info.title.trim().is_empty() => {
                let thumb = info.thumbnail.as_deref().and_then(decode_thumbnail);
                {
                    let mut st = self.state.borrow_mut();
                    st.has_track = true;
                    st.title = info.title.clone();
                    st.artist = info.artist.clone();
                    st.source_app_id = MediaAppName::from_source_app_id(Some(&info.source_app_id));
                    st.is_playing = info.is_playing;
                    st.is_wide_cover = thumb.as_ref().is_some_and(|b| b.height > 0 && b.width as f64 > b.height as f64 * 1.15);
                    st.thumbnail = thumb.map(|b| b.to_image());
                }
                self.update_cover_palette(info.thumbnail.clone());
            }
            _ => {
                {
                    let mut st = self.state.borrow_mut();
                    st.has_track = false;
                    st.title.clear();
                    st.artist.clear();
                    st.source_app_id.clear();
                    st.is_playing = false;
                    st.thumbnail = None;
                    st.is_wide_cover = false;
                }
                self.update_cover_palette(None);
            }
        }
        self.update_progress_timer();
    }

    fn apply_timeline(&self, timeline: Option<&PlaybackTimeline>) {
        let mut st = self.state.borrow_mut();
        let Some(t) = timeline else {
            st.position = TimeSpan::ZERO;
            st.duration = TimeSpan::ZERO;
            st.progress = 0.0;
            st.position_text = "0:00".into();
            st.duration_text = "0:00".into();
            return;
        };
        st.duration = PlaybackClock::duration(t);
        st.position = PlaybackClock::position_at(t, st.is_playing, Local::now().fixed_offset());
        st.progress =
            if st.duration.total_seconds() > 0.0 { (st.position.total_seconds() / st.duration.total_seconds()).clamp(0.0, 1.0) } else { 0.0 };
        st.position_text = format_time(st.position);
        st.duration_text = format_time(st.duration);
    }

    // ---- Kapak rengi ----

    /// Müzik kartı stili; çentik kurulurken ayardan verilir.
    pub fn set_card_style(self: &Rc<Self>, style: MusicCardStyle) {
        if style == self.card_style.get() {
            return;
        }
        self.card_style.set(style);
        *self.cover_key.borrow_mut() = None;
        self.update_cover_palette(self.media.current().and_then(|c| c.thumbnail));
    }

    /// Albüm rengi kart zemininin alfası; çentik malzemesi değişince çentik verir (Katı 255, Buzlu 219, Cam 184).
    pub fn set_card_alpha(&self, alpha: u8) {
        if alpha == self.card_alpha.get() {
            return;
        }
        self.card_alpha.set(alpha);
        if let Some(p) = self.palette.borrow().as_ref() {
            let (card, blend) = card_fill(p, alpha);
            let mut st = self.state.borrow_mut();
            st.paint.card = Some(card);
            st.paint.blend = blend;
        }
        self.changed.notify();
    }

    fn update_cover_palette(self: &Rc<Self>, bytes: Option<Vec<u8>>) {
        let bytes = bytes.filter(|b| !b.is_empty());
        let Some(bytes) = bytes.filter(|_| self.card_style.get() != MusicCardStyle::Neutral) else {
            self.palette_generation.set(self.palette_generation.get() + 1);
            *self.cover_key.borrow_mut() = None;
            self.apply_palette(None, None);
            return;
        };
        let key = CoverFingerprint::of(&bytes);
        if self.cover_key.borrow().as_ref() == Some(&key) {
            return; // aynı kapak: yeniden hesaplanmaz
        }
        *self.cover_key.borrow_mut() = Some(key);
        let generation = self.palette_generation.get() + 1;
        self.palette_generation.set(generation);
        let want_backdrop = self.card_style.get() == MusicCardStyle::Artwork;
        let me = UiRef::new(self);
        // 32 px çözüm ve palet arka planda; sonuç arayüz iş parçacığında uygulanır (parça değiştiyse atılır).
        std::thread::spawn(move || {
            let (palette, small) = sample_cover(&bytes, want_backdrop);
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && !vm.disposed.get()
                    && generation == vm.palette_generation.get()
                {
                    vm.apply_palette(palette, small);
                    vm.changed.notify();
                }
            });
        });
    }

    fn apply_palette(&self, palette: Option<CoverPalette>, small: Option<Bgra>) {
        let mut paint = CardPaint::default();
        if let Some(p) = &palette {
            // Çok renkli kapak çoğunlukla uygulama simgesidir: büyütülmüş simge zemini yerine renk karışımı.
            let small = if p.blend.is_some() { None } else { small };
            let (card, blend) = card_fill(p, self.card_alpha.get());
            paint.card = Some(card);
            paint.blend = blend;
            paint.sub_text = Some(color(p.sub_text, 255));
            paint.spectrum = (self.visualizer_color_source == NotchVisualizerColorSource::Artwork).then(|| color(p.spectrum, 0xBF));
            if let Some(small) = small {
                paint.tint_overlay = Some(color(p.card, 0x4D));
                paint.backdrop = Some(small.to_image());
                paint.has_backdrop = true;
            }
            paint.has_tint = true;
        }
        *self.palette.borrow_mut() = palette;
        self.state.borrow_mut().paint = paint;
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.disposed.replace(true) {
            return;
        }
        self.palette_generation.set(self.palette_generation.get() + 1);
        self.lyrics.dispose();
        self.update_spectrum();
        *self.progress_timer.borrow_mut() = None;
        for unsub in self.subs.borrow_mut().drain(..) {
            unsub();
        }
    }
}

fn color(c: Rgb, alpha: u8) -> slint::Color {
    slint::Color::from_argb_u8(alpha, c.r, c.g, c.b)
}

/// Kart zemini ve çok renkli kapakta köşelerden yayılan renkler (alfa 0x59 · zemin alfası).
fn card_fill(p: &CoverPalette, alpha: u8) -> (slint::Color, Vec<slint::Color>) {
    let blend = p
        .blend
        .as_ref()
        .map(|cs| cs.iter().take(4).map(|c| color(*c, (0x59u32 * alpha as u32 / 255) as u8)).collect())
        .unwrap_or_default();
    (color(p.card, alpha), blend)
}

/// "1:02:03" ya da "3:07".
pub fn format_time(t: TimeSpan) -> String {
    let total = t.total_seconds().max(0.0) as i64;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h >= 1 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

fn decode_thumbnail(bytes: &[u8]) -> Option<Bgra> {
    if bytes.is_empty() {
        return None;
    }
    // Kaynak daha büyükse MAX_COVER_PIXELS'e indirilir; küçükse olduğu gibi çözülür.
    let width = image_decode::pixel_width(bytes);
    image_decode::decode(bytes, (width > MAX_COVER_PIXELS).then_some(MAX_COVER_PIXELS))
}

/// Kapaktan 32 px genişlikte ikinci çözüm: pikseller palet çıkarıcıya gider; Kapak resmi stilinde aynı küçük
/// resim kart arka planı olur.
fn sample_cover(bytes: &[u8], keep_bitmap: bool) -> (Option<CoverPalette>, Option<Bgra>) {
    let Some(small) = image_decode::decode(bytes, Some(SAMPLE_PIXELS)) else { return (None, None) };
    let stride = (small.width * 4) as i32;
    let palette = CoverPaletteExtractor::extract(&small.pixels, small.width as i32, small.height as i32, stride);
    if palette.is_none() || !keep_bitmap {
        return (palette, None);
    }
    (palette, Some(small))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format() {
        assert_eq!(format_time(TimeSpan::from_secs(187)), "3:07");
        assert_eq!(format_time(TimeSpan::from_secs(3723)), "1:02:03");
        assert_eq!(format_time(TimeSpan::ZERO), "0:00");
    }
}

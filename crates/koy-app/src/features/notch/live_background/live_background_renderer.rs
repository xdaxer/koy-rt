//! Karşılığı: `Features/Notch/LiveBackground/LiveBackgroundHost.cs` ve `LiveBackgroundLoader.cs`.
//!
//! Canlı arka plan yüzeyi: ambiyans geçişi ya da görsel; üstünde koyu perde. Çentik zeminine ve müzik kartının zeminine
//! konur (Slint `LiveBg` küreseli), içeriğin altında kalır. Kararı vermez: oynatma kipi Core'dan gelir
//! (`LiveBackgroundPlaybackPolicy::decide`, `LiveBackgroundState`). Kip `Off` iken zamanlayıcı durur, görsel bırakılır:
//! boşta CPU ve bellek sıfır.
//!
//! Ambiyans: her renk durağı dizideki sıradaki renklere geçip başa döner (C# `ColorAnimationUsingKeyFrames`), 15 kare/sn.
//! Sapmalar (Slint yazılım çiziciyle): video oynatılamaz — yerleşik videolu önayar ambiyans renkleriyle, dosyadan video
//! hiç gösterilmez; GIF'in yalnız ilk karesi gösterilir.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use koy_core::notch::{LiveBackgroundContentKind, LiveBackgroundLimits, LiveBackgroundPlaybackMode, LiveBackgroundSelection};
use slint::{Color, ComponentHandle};

use crate::infra::{dispatch, ui_ref::UiRef};
use crate::{LiveBg, NotchWindow};

use super::live_background_state::LiveBackgroundState;

// Yazılımla çizimde her animasyon karesi bölgeyi yeniden çizer; yavaş geçişler için 15 fps yeter.
const FRAME: Duration = Duration::from_millis(1000 / 15);

pub struct LiveBackgroundRenderer {
    window: slint::Weak<NotchWindow>,
    state: Rc<LiveBackgroundState>,
    timer: slint::Timer,
    started: Cell<Instant>,
    colors: Vec<Color>,
    cycle: f64,
    image: RefCell<Option<slint::Image>>,
    image_requested: Cell<bool>,
}

fn parse_hex(hex: &str) -> Color {
    let h = hex.trim().trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0x202020);
    if h.len() == 8 { Color::from_argb_encoded(v) } else { Color::from_argb_encoded(0xFF00_0000 | v) }
}

fn lerp(a: Color, b: Color, t: f32) -> Color {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color::from_argb_u8(m(a.alpha(), b.alpha()), m(a.red(), b.red()), m(a.green(), b.green()), m(a.blue(), b.blue()))
}

/// n duraklı gradyanı `pos` (0..1) noktasında örnekler.
fn sample(stops: &[Color], pos: f32) -> Color {
    if stops.len() == 1 {
        return stops[0];
    }
    let seg = pos.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let i = (seg.floor() as usize).min(stops.len() - 2);
    lerp(stops[i], stops[i + 1], seg - i as f32)
}

impl LiveBackgroundRenderer {
    pub fn attach(window: &NotchWindow, state: Rc<LiveBackgroundState>) -> Rc<Self> {
        let (colors, cycle) = match &state.selection.preset {
            Some(p) => (p.colors.iter().map(|c| parse_hex(c)).collect(), p.cycle_seconds.max(1) as f64),
            None => (Vec::new(), 1.0),
        };
        let me = Rc::new(LiveBackgroundRenderer {
            window: window.as_weak(),
            state,
            timer: slint::Timer::default(),
            started: Cell::new(Instant::now()),
            colors,
            cycle,
            image: RefCell::new(None),
            image_requested: Cell::new(false),
        });
        let weak = Rc::downgrade(&me);
        me.state.changed.subscribe(move || {
            if let Some(m) = weak.upgrade() {
                m.apply();
            }
        });
        me.apply();
        me
    }

    fn selection(&self) -> &LiveBackgroundSelection {
        &self.state.selection
    }

    /// Kip değişti: yüzeyler kurulur ya da bırakılır.
    fn apply(self: &Rc<Self>) {
        let Some(w) = self.window.upgrade() else { return };
        let g = w.global::<LiveBg>();
        let notch = self.state.notch_mode();
        let card = self.state.music_card_mode();
        let off = |m: LiveBackgroundPlaybackMode| m == LiveBackgroundPlaybackMode::Off;
        let kind = self.content_kind();
        if kind == 0 || (off(notch) && off(card)) {
            self.timer.stop();
            g.set_notch_kind(0);
            g.set_card_kind(0);
            // Kareler bırakılır.
            self.image.borrow_mut().take();
            self.image_requested.set(false);
            g.set_img(slint::Image::default());
            return;
        }
        g.set_scrim(self.state.scrim_opacity as f32);
        g.set_notch_kind(if off(notch) { 0 } else { kind });
        g.set_card_kind(if off(card) { 0 } else { kind });
        if kind == 1 {
            let animate = notch == LiveBackgroundPlaybackMode::Animate || card == LiveBackgroundPlaybackMode::Animate;
            self.paint_ambient(0.0);
            if animate && self.colors.len() >= 2 {
                if !self.timer.running() {
                    self.started.set(Instant::now());
                    let weak = Rc::downgrade(self);
                    self.timer.start(slint::TimerMode::Repeated, FRAME, move || {
                        if let Some(m) = weak.upgrade() {
                            m.paint_ambient(m.started.get().elapsed().as_secs_f64());
                        }
                    });
                }
            } else {
                self.timer.stop();
            }
        } else {
            self.timer.stop();
            self.load_image();
        }
    }

    /// 0 yok, 1 gradyan (ambiyans), 2 görsel.
    fn content_kind(&self) -> i32 {
        let s = self.selection();
        match s.kind {
            LiveBackgroundContentKind::Ambient if !self.colors.is_empty() => 1,
            LiveBackgroundContentKind::AnimatedImage | LiveBackgroundContentKind::StillImage if s.file_path.is_some() => 2,
            _ => 0,
        }
    }

    /// Zaman `t` (sn) anındaki dört gradyan durağı.
    fn paint_ambient(&self, t: f64) {
        let Some(w) = self.window.upgrade() else { return };
        let n = self.colors.len();
        if n == 0 {
            return;
        }
        // Her durak i, döngü boyunca colors[i], colors[i+1], ... colors[i+n] = colors[i] üzerinden geçer.
        let phase = ((t / self.cycle).fract() * n as f64) as f32;
        let k = phase.floor() as usize;
        let frac = phase - k as f32;
        let stops: Vec<Color> = (0..n).map(|i| lerp(self.colors[(i + k) % n], self.colors[(i + k + 1) % n], frac)).collect();
        let g = w.global::<LiveBg>();
        g.set_c1(sample(&stops, 0.0));
        g.set_c2(sample(&stops, 1.0 / 3.0));
        g.set_c3(sample(&stops, 2.0 / 3.0));
        g.set_c4(sample(&stops, 1.0));
    }

    /// Görsel arka planda çözülür (çentik genişliğine küçültülür); çözülemeyen dosya "arka plan yok" demektir.
    fn load_image(self: &Rc<Self>) {
        if let Some(img) = self.image.borrow().as_ref() {
            if let Some(w) = self.window.upgrade() {
                w.global::<LiveBg>().set_img(img.clone());
            }
            return;
        }
        if self.image_requested.replace(true) {
            return;
        }
        let Some(path) = self.selection().file_path.clone() else { return };
        let ui = UiRef::new(self);
        std::thread::spawn(move || {
            let decoded = std::fs::read(&path)
                .ok()
                .and_then(|bytes| crate::infra::image_decode::decode(&bytes, Some(LiveBackgroundLimits::DECODE_PIXEL_WIDTH as u32)));
            dispatch::post(move || {
                let Some(me) = ui.upgrade() else { return };
                let Some(bgra) = decoded else {
                    if let Some(w) = me.window.upgrade() {
                        let g = w.global::<LiveBg>();
                        g.set_notch_kind(0);
                        g.set_card_kind(0);
                    }
                    return;
                };
                let img = bgra.to_image();
                // Bu arada kip kapandıysa görsel tutulmaz.
                if me.state.notch_mode() == LiveBackgroundPlaybackMode::Off && me.state.music_card_mode() == LiveBackgroundPlaybackMode::Off {
                    me.image_requested.set(false);
                    return;
                }
                *me.image.borrow_mut() = Some(img.clone());
                if let Some(w) = me.window.upgrade() {
                    w.global::<LiveBg>().set_img(img);
                }
            });
        });
    }
}

//! Çentik OSD (ses, parlaklık, pil, Bluetooth, sesli komut ve kart) görünüm modeli: 260×40 gösterge, 1,5 sn
//! hareketsizlik sonrası kendiliğinden kapanır. Karşılığı: `Features/Notch/OSD/OsdIndicatorViewModel.cs`, `OsdType.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use koy_core::audio::{AudioStateChangedEventArgs, IAudioService};
use koy_core::display::IBrightnessService;
use koy_core::event::Subscription;
use koy_core::notch::NotchCard;
use koy_core::notifications::IconBlankCheck;
use koy_core::power::BatteryAlertKind;
use koy_core::system_stats::SystemFormat;
use koy_core::voice::VoiceState;

use crate::infra::dispatch;
use crate::infra::image_decode::{self, Bgra};
use crate::infra::observable::{LocalEvent, Notifier};
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::{self, Strings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OsdType {
    #[default]
    None,
    Volume,
    Brightness,
    Battery,
    Bluetooth,
    Voice,
    Card,
}

#[derive(Clone)]
pub struct OsdState {
    pub is_active: bool,
    pub kind: OsdType,
    pub percent: i32,
    pub is_muted: bool,
    pub percent_text: String,
    pub battery_kind: BatteryAlertKind,
    pub battery_status_text: String,
    pub bluetooth_device_name: String,
    pub bluetooth_connected: bool,
    pub bluetooth_status_text: String,
    pub voice_prompt: String,
    pub voice_status_text: String,
    pub voice_state: VoiceState,
    /// Komut kutusunun metni; Windows sesle yazma da kullanıcı da buraya yazar.
    pub voice_input: String,
    /// Tek satıra sığmayan tam açıklama.
    pub voice_detail: Option<String>,
    pub card: Option<NotchCard>,
    /// Kartta gösterilecek uygulama simgesi (bildirim kartı).
    pub card_icon: Option<slint::Image>,
}

impl Default for OsdState {
    fn default() -> Self {
        OsdState {
            is_active: false,
            kind: OsdType::None,
            percent: 0,
            is_muted: false,
            percent_text: String::new(),
            battery_kind: BatteryAlertKind::None,
            battery_status_text: String::new(),
            bluetooth_device_name: String::new(),
            bluetooth_connected: false,
            bluetooth_status_text: String::new(),
            voice_prompt: String::new(),
            voice_status_text: String::new(),
            voice_state: VoiceState::Idle,
            voice_input: String::new(),
            voice_detail: None,
            card: None,
            card_icon: None,
        }
    }
}

impl OsdState {
    pub fn is_voice_editing(&self) -> bool {
        self.voice_state == VoiceState::Editing
    }
    pub fn has_card_icon(&self) -> bool {
        self.card_icon.is_some()
    }
}

pub struct OsdIndicatorViewModel {
    state: RefCell<OsdState>,
    timer: RefCell<Option<slint::Timer>>,
    audio: RefCell<Option<(Arc<dyn IAudioService>, Subscription)>>,
    brightness: RefCell<Option<(Arc<dyn IBrightnessService>, Subscription)>>,
    disposed: Cell<bool>,
    /// Göstergenin şimdi çıkıp çıkamayacağı (ör. çentik açıkken çıkmasın). Atanmamışsa her zaman çıkar.
    pub can_show: RefCell<Option<Box<dyn Fn() -> bool>>>,
    pub changed: Notifier,
    pub activated: Notifier,
    pub dismissed: Notifier,
    /// Kutudaki metin değişti (yalnız kutu açıkken).
    pub voice_input_changed: LocalEvent<String>,
    /// Kartın birincil düğmesine basıldı; kapanan kartı verir.
    pub primary_action_requested: LocalEvent<NotchCard>,
    /// Kartın × düğmesine basıldı; kapanan kartı verir.
    pub dismiss_requested: LocalEvent<NotchCard>,
}

impl OsdIndicatorViewModel {
    pub fn new() -> Rc<Self> {
        Rc::new(OsdIndicatorViewModel {
            state: RefCell::new(OsdState::default()),
            timer: RefCell::new(None),
            audio: RefCell::new(None),
            brightness: RefCell::new(None),
            disposed: Cell::new(false),
            can_show: RefCell::new(None),
            changed: Notifier::default(),
            activated: Notifier::default(),
            dismissed: Notifier::default(),
            voice_input_changed: LocalEvent::default(),
            primary_action_requested: LocalEvent::default(),
            dismiss_requested: LocalEvent::default(),
        })
    }

    pub fn state(&self) -> std::cell::Ref<'_, OsdState> {
        self.state.borrow()
    }

    pub fn is_active(&self) -> bool {
        self.state.borrow().is_active
    }

    pub fn kind(&self) -> OsdType {
        self.state.borrow().kind
    }

    fn can_show_now(&self) -> bool {
        !self.disposed.get() && self.can_show.borrow().as_ref().is_none_or(|f| f())
    }

    fn reset_timer(self: &Rc<Self>, interval: Duration) {
        let weak = Rc::downgrade(self);
        *self.timer.borrow_mut() = Some(dispatch::delay(interval, move || {
            if let Some(vm) = weak.upgrade() {
                *vm.timer.borrow_mut() = None;
                vm.dismiss();
            }
        }));
    }

    fn activate(self: &Rc<Self>, interval: Option<Duration>) {
        self.state.borrow_mut().is_active = true;
        match interval {
            Some(i) => self.reset_timer(i),
            None => *self.timer.borrow_mut() = None,
        }
        self.changed.notify();
        self.activated.notify();
    }

    /// Ses seviyesi ve sessize alma durumunu gösterir.
    pub fn show_volume(self: &Rc<Self>, volume: f32, is_muted: bool) {
        let pct = (volume.clamp(0.0, 1.0) as f64 * 100.0).round() as i32;
        {
            let mut st = self.state.borrow_mut();
            st.kind = OsdType::Volume;
            st.percent = pct;
            st.is_muted = is_muted;
            st.percent_text = pct.to_string();
        }
        self.activate(Some(Duration::from_millis(1500)));
    }

    /// Ekran parlaklık seviyesini gösterir.
    pub fn show_brightness(self: &Rc<Self>, brightness: i32) {
        let pct = brightness.clamp(0, 100);
        {
            let mut st = self.state.borrow_mut();
            st.kind = OsdType::Brightness;
            st.percent = pct;
            st.is_muted = false;
            st.percent_text = pct.to_string();
        }
        self.activate(Some(Duration::from_millis(1500)));
    }

    /// Pil bildirimini gösterir (şarja takıldı, şarjdan çıktı, düşük pil, tam şarj).
    pub fn show_battery(self: &Rc<Self>, kind: BatteryAlertKind, percent: i32) {
        if !self.can_show_now() {
            return;
        }
        let pct = percent.clamp(0, 100);
        {
            let mut st = self.state.borrow_mut();
            st.kind = OsdType::Battery;
            st.battery_kind = kind;
            st.percent = pct;
            st.percent_text = format!("{pct}%");
            st.is_muted = false;
            st.battery_status_text = match kind {
                BatteryAlertKind::PluggedIn => Strings::get("BatteryCharging"),
                BatteryAlertKind::Unplugged => Strings::get("BatteryOnBattery"),
                BatteryAlertKind::LowBattery => Strings::get("BatteryLow"),
                BatteryAlertKind::FullyCharged => Strings::get("BatteryFull"),
                _ => format!("{pct}%"),
            };
        }
        self.activate(Some(Duration::from_millis(1500)));
    }

    /// Bluetooth ses cihazı bağlantı / kesilme göstergesi (2 sn).
    pub fn show_bluetooth_device(self: &Rc<Self>, name: &str, connected: bool, battery: Option<i32>) {
        if !self.can_show_now() {
            return;
        }
        {
            let mut st = self.state.borrow_mut();
            st.kind = OsdType::Bluetooth;
            st.bluetooth_device_name = name.to_string();
            st.bluetooth_connected = connected;
            st.percent = battery.unwrap_or(0);
            st.percent_text = match battery {
                Some(b) if (0..=100).contains(&b) => SystemFormat::percent(b as f64, &strings::culture()),
                _ => String::new(),
            };
            st.bluetooth_status_text = Strings::get(if connected { "BluetoothConnected" } else { "BluetoothDisconnected" });
            st.is_muted = false;
        }
        self.activate(Some(Duration::from_secs(2)));
    }

    /// Harici ses servisini dinler: ses değişince gösterge çıkar.
    pub fn attach_audio(self: &Rc<Self>, audio: Arc<dyn IAudioService>) {
        self.detach_audio();
        let me = UiRef::new(self);
        let sub = audio.state_changed().subscribe(move |e: &AudioStateChangedEventArgs| {
            let e = *e;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && vm.can_show_now()
                {
                    vm.show_volume(e.volume, e.is_muted);
                }
            });
        });
        *self.audio.borrow_mut() = Some((audio, sub));
    }

    /// Harici parlaklık servisini dinler.
    pub fn attach_brightness(self: &Rc<Self>, brightness: Arc<dyn IBrightnessService>) {
        self.detach_brightness();
        let me = UiRef::new(self);
        let sub = brightness.brightness_changed().subscribe(move |b: &i32| {
            let b = *b;
            dispatch::post(move || {
                if let Some(vm) = me.upgrade()
                    && vm.can_show_now()
                {
                    vm.show_brightness(b);
                }
            });
        });
        *self.brightness.borrow_mut() = Some((brightness, sub));
    }

    /// `detail`: tek satıra sığmayan tam açıklama. `linger`: tıklanınca açılacak yanıt var, daha uzun görünür.
    pub fn show_voice(self: &Rc<Self>, state: VoiceState, status_text: &str, prompt: &str, detail: Option<&str>, linger: bool) {
        if !self.can_show_now() {
            return;
        }
        if state == VoiceState::Idle {
            self.dismiss();
            return;
        }
        {
            let mut st = self.state.borrow_mut();
            // Kutu yeni açıldıysa boş başlar; açıkken gelen güncellemeler yazılanı silmez.
            if state == VoiceState::Editing && st.voice_state != VoiceState::Editing {
                st.voice_input.clear();
            }
            st.kind = OsdType::Voice;
            st.voice_state = state;
            st.voice_status_text = status_text.to_string();
            st.voice_prompt = prompt.to_string();
            st.voice_detail = detail.filter(|d| !d.is_empty()).map(str::to_string);
            st.percent_text = match state {
                VoiceState::Editing => "Enter ↵".into(),
                VoiceState::Running => "...".into(),
                _ => String::new(),
            };
        }
        let interval = match state {
            VoiceState::Completed | VoiceState::Failed => {
                Some(Duration::from_secs(if linger { 10 } else if state == VoiceState::Failed { 6 } else { 3 }))
            }
            _ => None,
        };
        self.activate(interval);
    }

    /// Komut kutusuna yazıldı (görünümden).
    pub fn set_voice_input(&self, text: &str) {
        let editing = {
            let mut st = self.state.borrow_mut();
            if st.voice_input == text {
                return;
            }
            st.voice_input = text.to_string();
            st.voice_state == VoiceState::Editing
        };
        if editing {
            self.voice_input_changed.invoke(&text.to_string());
        }
    }

    /// Düğmeli kart gösterir. Çentik açıkken gösterilmez ve false döner; `lifetime` sonunda kendiliğinden kapanır.
    pub fn show_card(self: &Rc<Self>, card: NotchCard, lifetime: Duration) -> bool {
        if !self.can_show_now() {
            return false;
        }
        // Simge yalnız bu kartın kendi baytlarından çözülür; boş/saydam/bozuksa kabuk simgesi, o da yoksa görünüm zil çizer.
        let icon = decode_icon(card.icon.as_deref()).or_else(|| decode_icon(card.fallback_icon.as_deref()));
        {
            let mut st = self.state.borrow_mut();
            st.card = Some(card);
            st.card_icon = icon.map(|b| b.to_image());
            st.kind = OsdType::Card;
        }
        self.activate(Some(lifetime));
        true
    }

    /// Kartın birincil düğmesi: kart kapanır, sahibi eylemi yürütür.
    pub fn request_primary_action(&self) {
        let card = {
            let st = self.state.borrow();
            if st.kind != OsdType::Card {
                return;
            }
            st.card.clone()
        };
        let Some(card) = card else { return };
        self.dismiss();
        self.primary_action_requested.invoke(&card);
    }

    /// Kartın × düğmesi: kart kapanır, sahibi bunu "bu oturumda gösterme" diye kaydeder.
    pub fn request_dismiss(&self) {
        let card = {
            let st = self.state.borrow();
            if st.kind != OsdType::Card {
                return;
            }
            st.card.clone()
        };
        let Some(card) = card else { return };
        self.dismiss();
        self.dismiss_requested.invoke(&card);
    }

    pub fn dismiss(&self) {
        if !self.state.borrow().is_active {
            return;
        }
        *self.timer.borrow_mut() = None;
        *self.state.borrow_mut() = OsdState::default();
        self.changed.notify();
        self.dismissed.notify();
    }

    fn detach_audio(&self) {
        if let Some((a, s)) = self.audio.borrow_mut().take() {
            a.state_changed().unsubscribe(s);
        }
    }

    fn detach_brightness(&self) {
        if let Some((b, s)) = self.brightness.borrow_mut().take() {
            b.brightness_changed().unsubscribe(s);
        }
    }

    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        *self.timer.borrow_mut() = None;
        self.detach_audio();
        self.detach_brightness();
    }
}

/// Simge 48 px'e çözülür; tamamen saydamsa yok sayılır, saydam kenar boşluğu kırpılır.
fn decode_icon(data: Option<&[u8]>) -> Option<Bgra> {
    let data = data.filter(|d| !d.is_empty())?;
    let img = image_decode::decode(data, Some(48))?;
    crop_to_visible(img)
}

fn crop_to_visible(img: Bgra) -> Option<Bgra> {
    if img.width == 0 || img.height == 0 {
        return None;
    }
    let b = IconBlankCheck::get_visible_bounds(&img.pixels, img.width as i32, img.height as i32)?;
    if b.x == 0 && b.y == 0 && b.width as u32 == img.width && b.height as u32 == img.height {
        return Some(img);
    }
    let stride = img.width as usize * 4;
    let mut pixels = Vec::with_capacity(b.width as usize * b.height as usize * 4);
    for row in b.y..b.y + b.height {
        let start = row as usize * stride + b.x as usize * 4;
        pixels.extend_from_slice(&img.pixels[start..start + b.width as usize * 4]);
    }
    Some(Bgra { width: b.width as u32, height: b.height as u32, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_removes_transparent_border() {
        // 4x4, ortadaki 2x2 opak.
        let mut px = vec![0u8; 4 * 4 * 4];
        for y in 1..3 {
            for x in 1..3 {
                let i = (y * 4 + x) * 4;
                px[i..i + 4].copy_from_slice(&[10, 20, 30, 255]);
            }
        }
        let out = crop_to_visible(Bgra { width: 4, height: 4, pixels: px }).expect("görünür");
        assert_eq!((out.width, out.height), (2, 2));
        assert!(out.pixels.chunks(4).all(|p| p == [10, 20, 30, 255]));
        assert!(crop_to_visible(Bgra { width: 2, height: 2, pixels: vec![0; 16] }).is_none());
    }
}

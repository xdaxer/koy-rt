//! Karşılığı: `Shortcuts/KeyboardHookEngine.cs`.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};

use super::hotkey::{Hotkey, HotkeyModifiers};

/// Bir klavye olayının sonucu.
/// - `swallow`: olay Windows'a iletilmesin.
/// - `mask_modifier`: Win ya da Alt bırakılmadan önce boş tuş gönderilsin; yoksa Başlat ya da menü çubuğu açılır.
/// - `fired_id`: tetiklenen kısayolun kimliği.
/// - `captured`: yakalama sırasında basılan kombinasyon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookOutcome {
    pub swallow: bool,
    pub mask_modifier: bool,
    pub fired_id: Option<i32>,
    pub captured: Option<Hotkey>,
}

impl HookOutcome {
    pub const PASS: HookOutcome = HookOutcome { swallow: false, mask_modifier: false, fired_id: None, captured: None };
    pub const EAT: HookOutcome = HookOutcome { swallow: true, mask_modifier: false, fired_id: None, captured: None };
}

/// Kullanıcı yakalamayı yarıda bırakırsa klavye süresiz yutulmasın.
const CAPTURE_TIMEOUT_MS: i64 = 15_000;

/// Kanca iş parçacığından işlenen durum (C#'ta yalnız kanca iş parçacığından dokunulan alanlar).
#[derive(Default)]
struct ChordState {
    held_modifiers: HashSet<i32>,
    swallowed_keys: HashSet<i32>,
    chord_peak: HotkeyModifiers,
    chord_had_other_key: bool,
    fired_in_chord: bool,
    swallowing_chord: bool,
}

/// Düşük düzey klavye kancasının karar mantığı (Win32 yok, testli). Yalnız değiştiricilerden oluşan kısayolları
/// (tek başına Win) ve Windows'un kendi kısayollarını yakalar; Ayarlar'daki kısayol yakalamayı da yürütür.
/// `process` yalnız kanca iş parçacığından çağrılır; ayar ve yakalama değişimleri başka iş parçacığından gelebilir
/// (C#'taki `Volatile` okuma/yazma: burada `Mutex` / `AtomicI64`).
pub struct KeyboardHookEngine {
    state: Mutex<ChordState>,
    bindings: Mutex<HashMap<Hotkey, i32>>,
    capture_started_ms: AtomicI64,
}

impl Default for KeyboardHookEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardHookEngine {
    pub fn new() -> Self {
        KeyboardHookEngine {
            state: Mutex::new(ChordState::default()),
            bindings: Mutex::new(HashMap::new()),
            capture_started_ms: AtomicI64::new(-1),
        }
    }

    /// Aynı kombinasyon birden çok kimliğe bağlıysa C#'taki gibi sonuncusu kazanır; C# sözlüğü ekleme sırasıyla
    /// dolaşır, burada sıra belirsiz olmasın diye kimliklerin artan sırası kullanılır.
    pub fn set_bindings(&self, bindings: &HashMap<i32, Hotkey>) {
        let mut ids: Vec<i32> = bindings.keys().copied().collect();
        ids.sort_unstable();
        let mut map: HashMap<Hotkey, i32> = HashMap::new();
        for id in ids {
            map.insert(bindings[&id], id);
        }

        *self.bindings.lock().unwrap() = map;
    }

    pub fn begin_capture(&self, now_ms: i64) {
        self.capture_started_ms.store(now_ms, Ordering::SeqCst);
    }

    pub fn end_capture(&self) {
        self.capture_started_ms.store(-1, Ordering::SeqCst);
    }

    /// - `vk`: sanal tuş kodu.
    /// - `is_down`: basıldı mı, bırakıldı mı.
    /// - `now_ms`: geçen süre (ms).
    /// - `physically_down`: tuş gerçekten basılı mı. Bırakılma olayı kaçarsa (kilit ekranı, UAC) değiştirici basılı
    ///   sanılmasın diye.
    pub fn process(&self, vk: i32, is_down: bool, now_ms: i64, physically_down: Option<&dyn Fn(i32) -> bool>) -> HookOutcome {
        let capturing = self.is_capturing(now_ms);
        let mut st = self.state.lock().unwrap();

        if is_down && !st.swallowing_chord {
            if let Some(physically_down) = physically_down {
                st.held_modifiers.retain(|&held| held == vk || physically_down(held));
            }
        }

        match modifier_of(vk) {
            Some(flag) => self.process_modifier(&mut st, vk, flag, is_down, capturing),
            None => self.process_key(&mut st, vk, is_down, capturing),
        }
    }

    fn is_capturing(&self, now_ms: i64) -> bool {
        let started = self.capture_started_ms.load(Ordering::SeqCst);
        if started < 0 {
            return false;
        }

        if now_ms - started <= CAPTURE_TIMEOUT_MS {
            return true;
        }

        self.end_capture();
        false
    }

    fn process_modifier(&self, st: &mut ChordState, vk: i32, flag: HotkeyModifiers, is_down: bool, capturing: bool) -> HookOutcome {
        if is_down {
            // Basılı tutunca gelen tekrar.
            if !st.held_modifiers.insert(vk) {
                return if st.swallowing_chord { HookOutcome::EAT } else { HookOutcome::PASS };
            }

            if st.held_modifiers.len() == 1 && st.swallowed_keys.is_empty() {
                st.chord_peak = HotkeyModifiers::None;
                st.chord_had_other_key = false;
                st.fired_in_chord = false;
                st.swallowing_chord = capturing;
            }

            let current = current_modifiers(st);
            st.chord_peak |= current;
            return if st.swallowing_chord { HookOutcome::EAT } else { HookOutcome::PASS };
        }

        // Basılışı kancadan önce olan tuşun bırakılışı.
        if !st.held_modifiers.remove(&vk) {
            return HookOutcome::PASS;
        }

        let chord_ended = st.held_modifiers.is_empty();
        let modifier_only = Hotkey::new(st.chord_peak, 0);

        if st.swallowing_chord {
            let captured = if chord_ended && capturing && !st.chord_had_other_key { Some(modifier_only) } else { None };
            finish_swallow_if_done(st);
            return HookOutcome { swallow: true, mask_modifier: false, fired_id: None, captured };
        }

        let modifier_only_id = if !st.chord_had_other_key { self.bindings.lock().unwrap().get(&modifier_only).copied() } else { None };
        let ours = st.fired_in_chord || modifier_only_id.is_some();
        let mask = ours && (flag == HotkeyModifiers::Win || flag == HotkeyModifiers::Alt);
        HookOutcome { swallow: false, mask_modifier: mask, fired_id: if chord_ended { modifier_only_id } else { None }, captured: None }
    }

    fn process_key(&self, st: &mut ChordState, vk: i32, is_down: bool, capturing: bool) -> HookOutcome {
        if !is_down {
            if !st.swallowed_keys.remove(&vk) {
                return HookOutcome::PASS;
            }

            finish_swallow_if_done(st);
            return HookOutcome::EAT;
        }

        // Basılı tutunca gelen tekrar: eylem bir kez çalışır, tuş Windows'a sızmaz.
        if st.swallowed_keys.contains(&vk) {
            return HookOutcome::EAT;
        }

        st.chord_had_other_key = true;
        let hotkey = Hotkey::new(current_modifiers(st), vk);

        if capturing || st.swallowing_chord {
            st.swallowing_chord = true;
            st.swallowed_keys.insert(vk);
            return HookOutcome { swallow: true, mask_modifier: false, fired_id: None, captured: if capturing { Some(hotkey) } else { None } };
        }

        let Some(id) = self.bindings.lock().unwrap().get(&hotkey).copied() else {
            return HookOutcome::PASS;
        };

        st.fired_in_chord = true;
        st.swallowed_keys.insert(vk);
        HookOutcome { swallow: true, mask_modifier: false, fired_id: Some(id), captured: None }
    }
}

fn finish_swallow_if_done(st: &mut ChordState) {
    if st.held_modifiers.is_empty() && st.swallowed_keys.is_empty() {
        st.swallowing_chord = false;
    }
}

fn current_modifiers(st: &ChordState) -> HotkeyModifiers {
    let mut modifiers = HotkeyModifiers::None;
    for &vk in &st.held_modifiers {
        modifiers |= modifier_of(vk).unwrap_or(HotkeyModifiers::None);
    }

    modifiers
}

fn modifier_of(vk: i32) -> Option<HotkeyModifiers> {
    match vk {
        0x5B | 0x5C => Some(HotkeyModifiers::Win),
        0x10 | 0xA0 | 0xA1 => Some(HotkeyModifiers::Shift),
        0x11 | 0xA2 | 0xA3 => Some(HotkeyModifiers::Control),
        0x12 | 0xA4 | 0xA5 => Some(HotkeyModifiers::Alt),
        _ => None,
    }
}

//! Görünüm ayarlarını (tema, vurgu, malzeme, köşe, hareket) uygulama kaynaklarına uygular (C# `ThemeManager`).
//!
//! Katmanlar (C#'taki birleşik sözlük sırası): tema paleti (Koyu/Açık) → temadan bağımsız belirteçler (Tokens) →
//! ezmeler (vurgu tonları, köşe yarıçapları, Buzlu / Cam yüzeyler, yazı tipi) → kullanıcı stil dosyası. Çözülmüş tablo
//! arayüz iş parçacığındadır; her Slint penceresi `install` ile Theme global'ini bu tabloya bağlar, `apply` sürümü
//! artırır ve bütün açık pencereler yenilenir (WPF DynamicResource davranışı).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use koy_core::appearance::{
    AccentPalette, AppTheme, CornerRadii, CornerSetting, FontScalePolicy, MaterialLevel, MaterialSetting, ResolvedTheme, ThemeSetting,
    UserStyleFile, UserStyleFileStamp, UserStyleKind, UserStyleParseResult,
};
use koy_core::settings::{AppSettings, AppearanceSettings};
use windows::Win32::Foundation::LPARAM;
use windows::Win32::Graphics::Dwm::DwmGetColorizationColor;
use windows::Win32::Graphics::Gdi::{DEFAULT_CHARSET, EnumFontFamiliesExW, GetDC, LOGFONTW, ReleaseDC, TEXTMETRICW};

use super::material_state::MaterialState;
use super::motion::Motion;
use super::palette;
use crate::infra::dispatch;
use crate::infra::system_events::{Subscription, SystemEvents};

/// Çözülmüş kaynak değeri.
#[derive(Debug, Clone, PartialEq)]
pub enum ResValue {
    Color(u32),
    Number(f64),
    Radius([f64; 4]),
    Font(String),
    Weight(i32),
}

type Listener = Rc<dyn Fn()>;

struct State {
    map: HashMap<String, ResValue>,
    rev: i32,
    current: ResolvedTheme,
    last: Option<AppSettings>,
    user_style_result: Option<UserStyleParseResult>,
    user_style_stamp: UserStyleFileStamp,
    user_style_path: std::path::PathBuf,
    installed_fonts: Option<Vec<String>>,
    watching_material: bool,
    system_theme_sub: Option<Subscription>,
    applied: Vec<Listener>,
    user_style_applied: Vec<Listener>,
    windows: Vec<Box<dyn Fn(i32) -> bool>>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        map: base_map(ResolvedTheme::Dark),
        rev: 0,
        current: ResolvedTheme::Dark,
        last: None,
        user_style_result: None,
        user_style_stamp: UserStyleFileStamp::default(),
        user_style_path: UserStyleFile::default_path(),
        installed_fonts: None,
        watching_material: false,
        system_theme_sub: None,
        applied: Vec::new(),
        user_style_applied: Vec::new(),
        windows: Vec::new(),
    });
    static APPLYING: Cell<bool> = const { Cell::new(false) };
}

fn base_map(theme: ResolvedTheme) -> HashMap<String, ResValue> {
    let mut map = HashMap::new();
    let pal = if theme == ResolvedTheme::Light { palette::LIGHT } else { palette::DARK };
    for (k, v) in pal.iter().chain(palette::TOKEN_COLORS) {
        map.insert((*k).to_string(), ResValue::Color(*v));
    }
    for (k, v) in palette::TOKEN_NUMBERS {
        map.insert((*k).to_string(), ResValue::Number(*v));
    }
    for (k, v) in palette::TOKEN_RADII {
        map.insert((*k).to_string(), ResValue::Radius(*v));
    }
    for (k, v) in palette::TOKEN_FONTS {
        map.insert((*k).to_string(), ResValue::Font((*v).to_string()));
    }
    map.insert(palette::FONT_WEIGHT_KEY.to_string(), ResValue::Weight(400));
    map
}

fn argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32
}

fn parse_hex(hex: &str) -> Option<u32> {
    let h = hex.trim().trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).ok()?;
    match h.len() {
        6 => Some(0xFF00_0000 | v),
        8 => Some(v),
        _ => None,
    }
}

fn to_hex(c: u32) -> String {
    format!("#{:02X}{:02X}{:02X}", (c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF)
}

/// Windows vurgu rengi (C# `SystemParameters.WindowGlassColor`).
fn windows_accent_hex() -> Option<String> {
    let mut color = 0u32;
    let mut opaque = windows::core::BOOL(0);
    // SAFETY: çıktılar yerel değişkenlere yazılır.
    unsafe { DwmGetColorizationColor(&mut color, &mut opaque) }.ok()?;
    Some(to_hex(color))
}

/// Windows uygulama teması açık mı (kayıt defteri: AppsUseLightTheme).
pub fn system_uses_light_theme() -> bool {
    koy_platform::registry::get_dword(
        koy_platform::registry::Hive::CurrentUser,
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "AppsUseLightTheme",
    )
    .is_some_and(|v| v != 0)
}

/// Yüklü yazı tipi aileleri (C# `Fonts.SystemFontFamilies`), sıralı ve tekil.
fn enumerate_fonts() -> Vec<String> {
    unsafe extern "system" fn visit(lf: *const LOGFONTW, _: *const TEXTMETRICW, _: u32, lparam: LPARAM) -> i32 {
        // SAFETY: lparam bizim vektörümüzü gösterir; lf çağrı süresince geçerli.
        unsafe {
            let names = &mut *(lparam.0 as *mut Vec<String>);
            let face = &(*lf).lfFaceName;
            let len = face.iter().position(|&c| c == 0).unwrap_or(face.len());
            let name = String::from_utf16_lossy(&face[..len]);
            // Dikey yazı tipleri (@ ile başlar) WPF listesinde yoktur.
            if !name.starts_with('@') {
                names.push(name);
            }
        }
        1
    }
    let mut names: Vec<String> = Vec::new();
    // SAFETY: ekran DC'si alınıp bırakılır; geri çağrı yalnız çağrı süresince vektöre yazar.
    unsafe {
        let hdc = GetDC(None);
        let lf = LOGFONTW { lfCharSet: DEFAULT_CHARSET, ..Default::default() };
        EnumFontFamiliesExW(hdc, &lf, Some(visit), LPARAM(&mut names as *mut _ as isize), 0);
        ReleaseDC(None, hdc);
    }
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

const NOTCH_PAGE_CARDS: [&str; 8] = [
    "KoyNotchWeatherCard",
    "KoyNotchFocusCard",
    "KoyNotchShelfCard",
    "KoyNotchControlCard",
    "KoyNotchCalendarCard",
    "KoyNotchClipCard",
    "KoyNotchSystemCard",
    "KoyNotchNetCard",
];

const NOTCH_PAGE_CHIPS: [&str; 5] = ["KoyNotchFocusButton", "KoyNotchShelfChip", "KoyNotchControlTile", "KoyNotchCalendarChip", "KoyNotchClipChip"];

// Tokens'taki yazı ölçüleri (taban 13 DIP'te); taban boyu değişince oranla ölçeklenir.
const FONT_TOKENS: [(&str, f64); 8] = [
    ("KoyFontPageTitle", 20.0),
    ("KoyFontSubtitle", 12.5),
    ("KoyFontSection", 11.5),
    ("KoyFontRowTitle", 13.5),
    ("KoyFontBody", 13.0),
    ("KoyFontControl", 12.5),
    ("KoyFontCaption", 12.0),
    ("KoyFontKey", 11.5),
];

fn color_of(map: &HashMap<String, ResValue>, key: &str) -> u32 {
    match map.get(key) {
        Some(ResValue::Color(c)) => *c,
        _ => 0xFF00_0000,
    }
}

fn apply_typography(st: &mut State, o: &mut HashMap<String, ResValue>, a: &AppearanceSettings) {
    let size = FontScalePolicy::clamp_base_font_size(a.base_font_size);
    if (size - FontScalePolicy::DEFAULT_BASE_FONT_SIZE).abs() > 0.01 {
        for (key, at_default) in FONT_TOKENS {
            o.insert(key.to_string(), ResValue::Number(FontScalePolicy::scale_font_token(at_default, size)));
        }
    }
    let weight = FontScalePolicy::clamp_font_weight(a.font_weight);
    if weight != FontScalePolicy::DEFAULT_FONT_WEIGHT {
        o.insert(palette::FONT_WEIGHT_KEY.to_string(), ResValue::Weight(weight));
    }
    if a.font_family.trim().is_empty() && a.clock_font_family.trim().is_empty() {
        return;
    }
    let installed = st.installed_fonts.get_or_insert_with(enumerate_fonts).clone();
    // Boş ya da kaldırılmış yazı tipi: ezme yok, Tokens varsayılanı kalır.
    let ui = FontScalePolicy::resolve_font_family_with_fallback(Some(&a.font_family), &installed, "");
    if !ui.is_empty() {
        for k in ["KoyFont", "KoyNotchFont", "KoyNotchDisplayFont"] {
            o.insert(k.to_string(), ResValue::Font(ui.clone()));
        }
    }
    let clock = FontScalePolicy::resolve_clock_font_family(Some(&a.clock_font_family), &ui, &installed);
    if !clock.is_empty() {
        o.insert("KoyClockFont".to_string(), ResValue::Font(clock));
    }
}

fn build_overrides(st: &mut State, settings: &AppSettings, theme: ResolvedTheme, palette_map: &HashMap<String, ResValue>) -> HashMap<String, ResValue> {
    let mut o = HashMap::new();
    apply_typography(st, &mut o, &settings.appearance);

    // Köşeler.
    let r: CornerRadii = CornerRadii::r#for(CornerSetting::resolve(settings.appearance.corner_radius));
    let w1 = (r.window - 1.0).max(0.0);
    o.insert("KoyRadiusWindow".into(), ResValue::Radius([r.window; 4]));
    o.insert("KoyRadiusWindowTop".into(), ResValue::Radius([w1, w1, 0.0, 0.0]));
    o.insert("KoyRadiusWindowBottomLeft".into(), ResValue::Radius([0.0, 0.0, 0.0, w1]));
    o.insert("KoyRadiusRow".into(), ResValue::Radius([r.row; 4]));
    o.insert("KoyRadiusControl".into(), ResValue::Radius([r.control; 4]));
    o.insert("KoyRadiusSmall".into(), ResValue::Radius([r.small; 4]));

    // Vurgu: Windows vurgusu ya da seçilen renk; varsayılan Koy mavisi tema paletinden gelir.
    let accent_hex = if settings.appearance.use_windows_accent { windows_accent_hex() } else { settings.appearance.accent_color.clone() };
    if let Some(hex) = accent_hex.filter(|h| !h.trim().is_empty()) {
        let surface = to_hex(color_of(palette_map, "KoyBgRow"));
        let p = AccentPalette::derive(Some(&hex), theme, &surface);
        let set = |o: &mut HashMap<String, ResValue>, k: &str, h: &str| {
            if let Some(c) = parse_hex(h) {
                o.insert(k.to_string(), ResValue::Color(c));
            }
        };
        set(&mut o, "KoyAccent", &p.base);
        set(&mut o, "KoyAccentHover", &p.hover);
        set(&mut o, "KoyAccentPressed", &p.pressed);
        set(&mut o, "KoyOnAccent", &p.on_accent);
        set(&mut o, "KoyAccentText", &p.text);
        set(&mut o, "KoyFocusRing", &p.focus);
        // Çentik de aynı vurguyu kullanır (Kontrol sekmesi, anahtarlar, kaydırıcılar); çentik zemini siyah kalır.
        set(&mut o, "KoyNotchAccentControl", &p.base);
        if let Some(c) = parse_hex(&p.base) {
            let (r, g, b) = ((c >> 16) as u8, (c >> 8) as u8, c as u8);
            o.insert("KoyNotchControlIconBg".into(), ResValue::Color(argb(0x38, r, g, b)));
            o.insert("KoyAccentSubtle".into(), ResValue::Color(argb(if theme == ResolvedTheme::Dark { 0x26 } else { 0x22 }, r, g, b)));
        }
    }

    // Saydam malzeme (Buzlu %86, Cam %72): pencere yüzeyleri seviyenin opaklığında, satırlar duvar kağıdını hafifçe
    // gösteren beyaz katman (koyu %5, açık %62).
    let window_level = MaterialState::for_window(settings, theme);
    if window_level != MaterialLevel::Solid {
        let alpha = MaterialSetting::surface_alpha(window_level);
        let src = color_of(palette_map, "KoyBgWindow");
        let surface = (alpha as u32) << 24 | (src & 0x00FF_FFFF);
        o.insert("KoyBgWindow".into(), ResValue::Color(surface));
        o.insert("KoyBgShell".into(), ResValue::Color(surface));
        o.insert(
            "KoyBgRow".into(),
            ResValue::Color(if theme == ResolvedTheme::Dark { argb(0x0D, 0xFF, 0xFF, 0xFF) } else { argb(0x9E, 0xFF, 0xFF, 0xFF) }),
        );
        // Segment kutusu: opak gri, saydam satırın üstünde delik gibi durmasın.
        o.insert(
            "KoyBgSegment".into(),
            ResValue::Color(if theme == ResolvedTheme::Dark { argb(0x38, 0, 0, 0) } else { argb(0x0D, 0, 0, 0) }),
        );
    }

    // Açık çentik camlaşınca kartlar opak gri yerine beyaz katman olur (kart %6, vurgulu %9).
    if MaterialState::for_notch(settings) != MaterialLevel::Solid {
        let card = ResValue::Color(argb(0x0F, 0xFF, 0xFF, 0xFF));
        let chip = ResValue::Color(argb(0x17, 0xFF, 0xFF, 0xFF));
        o.insert("KoyNotchCard".into(), card.clone());
        o.insert("KoyNotchChip".into(), chip.clone());
        o.insert("KoyNotchMusicCard".into(), card.clone());
        for k in NOTCH_PAGE_CARDS {
            o.insert(k.into(), card.clone());
        }
        for k in NOTCH_PAGE_CHIPS {
            o.insert(k.into(), chip.clone());
        }
    }
    o
}

/// Kullanıcı stil değerlerini kaynaklara çevirir (C# `UserStyleResources.Fill`). Kalınlık (Thickness) ve Renk (Color)
/// türleri Slint'te ayrı tür değildir: renk renge, kalınlık dört kenar olarak köşe tablosuna gider.
fn user_style_values(result: &UserStyleParseResult) -> HashMap<String, ResValue> {
    let mut out = HashMap::new();
    for (key, v) in &result.valid_entries {
        let value = match v.kind {
            UserStyleKind::Brush | UserStyleKind::Color => ResValue::Color(argb(v.color.a, v.color.r, v.color.g, v.color.b)),
            UserStyleKind::Number => ResValue::Number(v.number),
            UserStyleKind::CornerRadius | UserStyleKind::Thickness => ResValue::Radius([v.quad.first, v.quad.second, v.quad.third, v.quad.fourth]),
            UserStyleKind::FontFamily => ResValue::Font(v.font_family.clone()),
            #[allow(unreachable_patterns)]
            _ => continue,
        };
        out.insert(key.clone(), value);
    }
    out
}

fn notify(list: Vec<Listener>) {
    for f in list {
        f();
    }
}

fn push_rev() {
    let (rev, windows) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.rev += 1;
        (s.rev, std::mem::take(&mut s.windows))
    });
    let alive: Vec<_> = windows.into_iter().filter(|w| w(rev)).collect();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let added = std::mem::take(&mut s.windows);
        s.windows = alive;
        s.windows.extend(added);
    });
}

pub struct ThemeManager;

impl ThemeManager {
    pub fn current() -> ResolvedTheme {
        STATE.with(|s| s.borrow().current)
    }

    /// Son uygulanan ayarlar (pencere çerçeveleri okur); henüz uygulanmadıysa `None`.
    pub fn last_settings() -> Option<AppSettings> {
        STATE.with(|s| s.borrow().last.clone())
    }

    pub fn on_applied(f: impl Fn() + 'static) {
        STATE.with(|s| s.borrow_mut().applied.push(Rc::new(f)));
    }

    pub fn on_user_style_applied(f: impl Fn() + 'static) {
        STATE.with(|s| s.borrow_mut().user_style_applied.push(Rc::new(f)));
    }

    pub fn user_style_result() -> UserStyleParseResult {
        STATE.with(|s| s.borrow().user_style_result.clone().unwrap_or_else(UserStyleParseResult::empty))
    }

    pub fn user_style_path() -> std::path::PathBuf {
        STATE.with(|s| s.borrow().user_style_path.clone())
    }

    pub fn set_user_style_path(path: std::path::PathBuf) {
        STATE.with(|s| s.borrow_mut().user_style_path = path);
    }

    /// Kaynak değeri (Rust tarafındaki okuyucular için; ör. dock yüzeyi rengi).
    pub fn resource(key: &str) -> Option<ResValue> {
        STATE.with(|s| s.borrow().map.get(key).cloned())
    }

    pub fn color(key: &str) -> u32 {
        STATE.with(|s| color_of(&s.borrow().map, key))
    }

    /// Yüklü yazı tipi adları (sıralı, bir kez okunur). Ayarlar sayfasının listesi de bunu kullanır.
    pub fn installed_fonts() -> Vec<String> {
        STATE.with(|s| s.borrow_mut().installed_fonts.get_or_insert_with(enumerate_fonts).clone())
    }

    /// Ayarları uygular. "Sistem" seçiliyken Windows uygulama teması izlenir.
    pub fn apply(settings: &AppSettings) {
        Self::apply_with(settings, None);
    }

    pub fn apply_with(settings: &AppSettings, system_uses_light_override: Option<bool>) {
        if APPLYING.with(|a| a.replace(true)) {
            return;
        }
        let theme = ThemeSetting::resolve(settings.appearance.theme, system_uses_light_override.unwrap_or_else(system_uses_light_theme));
        let palette_map = base_map(theme);
        let mut map = palette_map.clone();
        let overrides = STATE.with(|s| build_overrides(&mut s.borrow_mut(), settings, theme, &palette_map));
        map.extend(overrides);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.current = theme;
            s.last = Some(settings.clone());
            s.map = map;
        });
        let user_style_listeners = apply_user_style(settings, false);
        Motion::refresh(settings);

        // Duvar kağıdı okununca ya da değişince okunabilirlik koruması yeniden değerlendirilir.
        let first_material = STATE.with(|s| !std::mem::replace(&mut s.borrow_mut().watching_material, true));
        if first_material {
            MaterialState::on_changed(|| {
                if let Some(s) = ThemeManager::last_settings() {
                    ThemeManager::apply(&s);
                }
            });
        }
        if settings.appearance.theme == AppTheme::System || settings.appearance.use_windows_accent {
            watch_system_theme();
        }
        APPLYING.with(|a| a.set(false));
        push_rev();
        notify(user_style_listeners);
        notify(STATE.with(|s| s.borrow().applied.clone()));
    }

    /// Dosyayı yeniden okuyup ezmeleri uygular ("Yeniden yükle" düğmesi).
    pub fn reload_user_style() {
        let Some(settings) = Self::last_settings() else { return };
        let listeners = apply_user_style(&settings, true);
        push_rev();
        notify(listeners);
        notify(STATE.with(|s| s.borrow().applied.clone()));
    }

    /// Dosya son okunduğundan beri değiştiyse yeniden okur (dosya izleyici ve ayarlar penceresi açılışı çağırır).
    pub fn reload_user_style_if_changed() {
        let Some(settings) = Self::last_settings() else { return };
        if !settings.appearance.user_style_overrides_enabled {
            return;
        }
        let (path, stamp) = STATE.with(|s| {
            let s = s.borrow();
            (s.user_style_path.clone(), s.user_style_stamp)
        });
        if UserStyleFile::get_stamp(&path) == stamp {
            return;
        }
        Self::reload_user_style();
    }

    /// Slint penceresinin Theme global'ini çözülmüş tabloya bağlar; tema her uygulandığında pencere yenilenir.
    pub fn install<C>(component: &C)
    where
        C: slint::ComponentHandle + 'static,
        for<'a> crate::Theme<'a>: slint::Global<'a, C>,
    {
        let theme = component.global::<crate::Theme>();
        theme.on_get_color(|key, _| slint::Color::from_argb_encoded(ThemeManager::color(&key)));
        theme.on_get_number(|key, _| match ThemeManager::resource(&key) {
            Some(ResValue::Number(n)) => n as f32,
            _ => 0.0,
        });
        theme.on_get_radius(|key, corner, _| match ThemeManager::resource(&key) {
            Some(ResValue::Radius(r)) => r[(corner.clamp(0, 3)) as usize] as f32,
            Some(ResValue::Number(n)) => n as f32,
            _ => 0.0,
        });
        theme.on_get_font(|key, _| match ThemeManager::resource(&key) {
            // Slint tek aile adı alır: WPF yedek listesinin ilk adı.
            Some(ResValue::Font(f)) => f.split(',').next().unwrap_or("").trim().into(),
            _ => slint::SharedString::new(),
        });
        theme.on_get_weight(|_| match ThemeManager::resource(palette::FONT_WEIGHT_KEY) {
            Some(ResValue::Weight(w)) => w,
            _ => 400,
        });
        let rev = STATE.with(|s| s.borrow().rev);
        theme.set_rev(rev);
        let weak = component.as_weak();
        STATE.with(|s| {
            s.borrow_mut().windows.push(Box::new(move |rev| match weak.upgrade() {
                Some(c) => {
                    c.global::<crate::Theme>().set_rev(rev);
                    true
                }
                None => false,
            }))
        });
    }
}

/// Kullanıcı stil dosyası ezmelerini tabloya uygular; çağrılması gereken dinleyicileri döndürür.
fn apply_user_style(settings: &AppSettings, reload: bool) -> Vec<Listener> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !settings.appearance.user_style_overrides_enabled {
            // Kapalıyken dosya okunmaz; önbellek de atılır ki yeniden açılınca güncel dosya okunsun.
            let was_active = s.user_style_result.is_some();
            s.user_style_result = None;
            s.user_style_stamp = UserStyleFileStamp::default();
            return if was_active { s.user_style_applied.clone() } else { Vec::new() };
        }
        // Önbellek: tema / malzeme değişince dosya yeniden okunmaz, yalnız yeniden uygulanır.
        if reload || s.user_style_result.is_none() {
            let path = s.user_style_path.clone();
            s.user_style_stamp = UserStyleFile::get_stamp(&path);
            s.user_style_result = Some(UserStyleFile::load(&path));
        }
        let values = user_style_values(s.user_style_result.as_ref().expect("result"));
        // Kullanıcı değeri temadan ve ThemeManager'ın kendi ezmelerinden güçlüdür.
        s.map.extend(values);
        s.user_style_applied.clone()
    })
}

fn watch_system_theme() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.system_theme_sub.is_some() {
            return;
        }
        // Windows teması değişince (yalnız ayar "Sistem" ya da Windows vurgusu iken) kaynaklar yeniden uygulanır.
        s.system_theme_sub = Some(SystemEvents::on_user_preference_changed(|| {
            dispatch::post(|| {
                if let Some(s) = ThemeManager::last_settings()
                    && (s.appearance.theme == AppTheme::System || s.appearance.use_windows_accent)
                {
                    ThemeManager::apply(&s);
                }
            });
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_map_has_every_theme_key() {
        let dark = base_map(ResolvedTheme::Dark);
        let light = base_map(ResolvedTheme::Light);
        assert_eq!(dark.len(), light.len());
        assert_ne!(dark["KoyBgWindow"], light["KoyBgWindow"]);
        assert_eq!(parse_hex("#3B82F6"), Some(0xFF3B82F6));
        assert_eq!(parse_hex("#263B82F6"), Some(0x263B82F6));
    }
}

//! Etkin malzeme: seçilen seviye (Katı / Buzlu / Cam) okunabilirlik korumasından geçer. Duvar kağıdının ortalama rengi
//! yalnız saydam bir malzeme ve koruma açıkken okunur (ilk istekte bir kez, sonra duvar kağıdı değişince); yoklama yok.
//! Okuma bitince ya da duvar kağıdı değişince `changed` dinleyicileri arayüz iş parçacığında çağrılır.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use koy_core::appearance::{MaterialLevel, MaterialSetting, ReadabilityGuard, ResolvedTheme, Rgb, Surface};
use koy_core::settings::AppSettings;
use koy_platform::display::WallpaperSampler;
use koy_platform::shell::BackdropService;

use crate::infra::dispatch;
use crate::infra::system_events::{Subscription, SystemEvents};

thread_local! {
    static WALLPAPER: Cell<Option<Rgb>> = const { Cell::new(None) };
    static SAMPLED: Cell<bool> = const { Cell::new(false) };
    static SAMPLING: Cell<bool> = const { Cell::new(false) };
    static WATCH: RefCell<Option<Subscription>> = const { RefCell::new(None) };
    static LISTENERS: RefCell<Vec<Rc<dyn Fn()>>> = const { RefCell::new(Vec::new()) };
}

pub struct MaterialState;

impl MaterialState {
    /// Duvar kağıdının ortalama rengi; okunmadıysa ya da okunamadıysa `None`.
    pub fn wallpaper() -> Option<Rgb> {
        WALLPAPER.with(Cell::get)
    }

    pub fn on_changed(f: impl Fn() + 'static) {
        LISTENERS.with(|l| l.borrow_mut().push(Rc::new(f)));
    }

    /// Koy penceresi, sağ alt menü ve sihirbaz için etkin seviye. Windows 10'da DWM akrilik olmadığı için Katı.
    pub fn for_window(settings: &AppSettings, theme: ResolvedTheme) -> MaterialLevel {
        let surface = if theme == ResolvedTheme::Light { ReadabilityGuard::WINDOW_LIGHT } else { ReadabilityGuard::WINDOW_DARK };
        MaterialSetting::resolve_window_material(resolve(settings, surface), BackdropService::is_supported())
    }

    /// Açık çentik ve kartlar için etkin seviye. "Açık çentik malzemeyi izlesin" kapalıysa hep Katı.
    pub fn for_notch(settings: &AppSettings) -> MaterialLevel {
        if settings.notch.follows_material { resolve(settings, ReadabilityGuard::NOTCH) } else { MaterialLevel::Solid }
    }

    /// Dock yüzeyi için etkin seviye (çentikle aynı koyu taban).
    pub fn for_dock(settings: &AppSettings) -> MaterialLevel {
        resolve(settings, ReadabilityGuard::NOTCH)
    }

    /// Görüntü aracı ve testler: duvar kağıdını okumadan sabit bir renk ver.
    pub fn set_wallpaper_for_preview(wallpaper: Option<Rgb>) {
        WALLPAPER.with(|w| w.set(wallpaper));
        SAMPLED.with(|s| s.set(true));
    }
}

fn resolve(settings: &AppSettings, surface: Surface) -> MaterialLevel {
    let requested = settings.appearance.material;
    if requested != MaterialLevel::Solid && settings.appearance.material_readability_guard {
        ensure_sampled();
    }
    ReadabilityGuard::effective(requested, settings.appearance.material_readability_guard, MaterialState::wallpaper(), surface)
}

fn ensure_sampled() {
    WATCH.with(|w| {
        if w.borrow().is_none() {
            // Duvar kağıdı değişimi WM_SETTINGCHANGE ile gelir (C#: UserPreferenceCategory.Desktop).
            *w.borrow_mut() = Some(SystemEvents::on_user_preference_changed(|| dispatch::post(sample)));
        }
    });
    if !SAMPLED.with(|s| s.replace(true)) {
        dispatch::post(sample);
    }
}

/// Duvar kağıdı arka planda örneklenir; sonuç arayüz iş parçacığında uygulanır.
fn sample() {
    if SAMPLING.with(|s| s.replace(true)) {
        return;
    }
    std::thread::spawn(|| {
        let color = {
            let _com = koy_platform::com::ComApartment::mta();
            WallpaperSampler::sample()
        };
        dispatch::post(move || {
            SAMPLING.with(|s| s.set(false));
            if color != MaterialState::wallpaper() {
                WALLPAPER.with(|w| w.set(color));
                let listeners: Vec<Rc<dyn Fn()>> = LISTENERS.with(|l| l.borrow().clone());
                for f in listeners {
                    f();
                }
            }
        });
    });
}

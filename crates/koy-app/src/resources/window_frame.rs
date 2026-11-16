//! Koy penceresi, sihirbaz ve sağ alt menü için DWM çerçevesi (C# `WindowFrame`): sistem arka planı (akrilik),
//! koyu/açık çerçeve ve köşe tercihi. Tema ya da malzeme değişince yeniden uygulanır.
//!
//! Slint pencereleri katmansızdır ve `background: transparent` ile saydam temizlenir; DWM arka planı bu yüzden görünür
//! (C#'taki `CompositionTarget.BackgroundColor = Transparent` adımının karşılığı pencere bileşenindedir).

use std::cell::Cell;
use std::rc::Rc;

use koy_core::appearance::{CornerSetting, CornerStyle, MaterialLevel, MaterialSetting, ResolvedTheme};
use koy_core::settings::AppSettings;
use koy_platform::shell::{BackdropService, WindowBackdropType, WindowCornerStyle};

use super::material_state::MaterialState;
use super::theme_manager::ThemeManager;

/// Pencere tanıtıcısını verir (pencere gösterilmeden önce 0 olabilir).
pub type HwndProvider = Rc<dyn Fn() -> isize>;

pub struct WindowFrame;

impl WindowFrame {
    /// Çerçeveyi uygular ve tema / malzeme değişimlerine bağlar. `alive` false olunca (pencere kapandı) bağ kendiliğinden
    /// etkisizleşir.
    pub fn attach(hwnd: HwndProvider, alive: Rc<Cell<bool>>) {
        apply(hwnd());
        let (h1, a1) = (Rc::clone(&hwnd), Rc::clone(&alive));
        ThemeManager::on_applied(move || {
            if a1.get() {
                apply(h1());
            }
        });
        MaterialState::on_changed(move || {
            if alive.get() {
                apply(hwnd());
            }
        });
    }

    pub fn apply_now(hwnd: isize) {
        apply(hwnd);
    }
}

fn apply(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    let settings = ThemeManager::last_settings().unwrap_or_else(AppSettings::default);
    let level = MaterialSetting::resolve_window_material(MaterialState::for_window(&settings, ThemeManager::current()), BackdropService::is_supported());
    let sharp = CornerSetting::resolve(settings.appearance.corner_radius) == CornerStyle::Sharp;
    BackdropService::apply_window_frame(
        hwnd,
        if level == MaterialLevel::Solid { WindowBackdropType::None } else { WindowBackdropType::Acrylic },
        ThemeManager::current() == ResolvedTheme::Dark,
        if sharp { WindowCornerStyle::RoundSmall } else { WindowCornerStyle::Round },
    );
}

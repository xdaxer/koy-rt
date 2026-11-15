//! Karşılığı: `Input/CornerActions.cs`.

use serde::{Deserialize, Serialize};

use super::hot_corner_math::HotCornerMath;
use super::input_action::InputAction;
use super::input_binding::{HotCornerTrigger, InputBinding, InputTrigger, ScreenCorner};
use super::input_binding_parser::InputBindingParser;
use crate::settings::HotCornersSettings;
use crate::text;

/// Bir köşeye bağlanabilen eylem. Yalnız Koy'un bugün zaten yapabildiği şeyler; None = köşe kapalı.
/// Ayar dosyasında ad olarak yazılır: mevcut adlar değiştirilmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum CornerAction {
    #[default]
    None,
    OpenNotch,
    OpenSearch,
    OpenKoyWindow,
    ToggleDock,
}

/// Köşe ayarının saf kararları: hangi köşe hangi eylemi çalıştırır, takip gerekli mi, süre sınırları.
pub struct CornerActions;

impl CornerActions {
    pub const DEFAULT_DWELL_MS: i32 = 400;
    pub const MIN_DWELL_MS: i32 = 150;
    pub const MAX_DWELL_MS: i32 = 2000;

    /// Köşe bölgesinin boyu (fiziksel piksel). İmleç ekran kenarına dayandığı için küçük bir kare yeter.
    pub const CORNER_SIZE_PX: i32 = 6;

    pub fn clamp_dwell(dwell_ms: i32) -> i32 {
        text::clamp(dwell_ms, Self::MIN_DWELL_MS, Self::MAX_DWELL_MS)
    }

    pub fn action_for(settings: &HotCornersSettings, corner: ScreenCorner) -> CornerAction {
        match corner {
            ScreenCorner::TopLeft => settings.top_left,
            ScreenCorner::TopRight => settings.top_right,
            ScreenCorner::BottomLeft => settings.bottom_left,
            _ => settings.bottom_right,
        }
    }

    /// C#: `settings with { TopLeft = action }` (kopya üstünde değişiklik).
    pub fn with(settings: &HotCornersSettings, corner: ScreenCorner, action: CornerAction) -> HotCornersSettings {
        let mut copy = settings.clone();
        match corner {
            ScreenCorner::TopLeft => copy.top_left = action,
            ScreenCorner::TopRight => copy.top_right = action,
            ScreenCorner::BottomLeft => copy.bottom_left = action,
            _ => copy.bottom_right = action,
        }
        copy
    }

    /// Çalışacak `InputAction`; köşe kapalı ya da değer tanımsızsa `None`.
    pub fn to_input_action(action: CornerAction) -> Option<InputAction> {
        match action {
            CornerAction::OpenNotch => Some(InputAction::OpenNotch),
            CornerAction::OpenSearch => Some(InputAction::OpenSearch),
            CornerAction::OpenKoyWindow => Some(InputAction::OpenKoyWindow),
            CornerAction::ToggleDock => Some(InputAction::ToggleDock),
            _ => None,
        }
    }

    /// Takip gerekli mi: en az bir köşede geçerli bir eylem var. Hepsi None ise hiçbir şey çalışmamalı.
    pub fn is_active(settings: &HotCornersSettings) -> bool {
        ScreenCorner::ALL.iter().any(|&c| Self::to_input_action(Self::action_for(settings, c)).is_some())
    }

    /// Ayardaki köşeleri ortak bağ modeline çevirir (`InputBinding`); kapalı köşeler girmez.
    pub fn to_bindings(settings: &HotCornersSettings) -> Vec<InputBinding> {
        let dwell = Self::clamp_dwell(settings.dwell_ms);
        let mut result = Vec::new();
        for corner in ScreenCorner::ALL {
            if let Some(action) = Self::to_input_action(Self::action_for(settings, corner)) {
                result.push(InputBinding::new(
                    action,
                    InputTrigger::HotCorner(HotCornerTrigger::new(corner, dwell, InputBindingParser::DEFAULT_PUSH_BACK_PX)),
                ));
            }
        }

        result
    }
}

/// Köşe bekleme durumu (saf mantık). İmleç aynı köşede kesintisiz `dwell_ms` kalınca bir kez tetikler;
/// bir daha tetiklenmek için köşeden çıkıp yeniden girmek gerekir (köşede dururken eylem tekrarlanmaz).
/// Fare tuşu basılıyken (sürükleme) ya da bastırılmışken (tam ekran) sıfırlanır. Zaman dışarıdan verilir.
#[derive(Debug, Clone, Default)]
pub struct HotCornerTracker {
    current: Option<ScreenCorner>,
    entered_at_ms: i64,
    fired: bool,
}

impl HotCornerTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// - `corner`: İmlecin bulunduğu köşe; köşede değilse `None`.
    /// - `suppressed`: Tuş basılı ya da tam ekran gibi nedenlerle tetikleme yasak.
    /// - `now_ms`: Tekdüze artan zaman (örn. Environment.TickCount64).
    ///
    /// Bekleme bu çağrıda dolduysa köşe; aksi halde `None`.
    pub fn update(&mut self, corner: Option<ScreenCorner>, suppressed: bool, now_ms: i64, dwell_ms: i32) -> Option<ScreenCorner> {
        if corner.is_none() || suppressed {
            self.reset();
            return None;
        }

        if self.current != corner {
            self.current = corner;
            self.entered_at_ms = now_ms;
            self.fired = false;
        }

        if self.fired || !HotCornerMath::is_dwell_complete(self.entered_at_ms, now_ms, Self::clamp_dwell_for(dwell_ms)) {
            return None;
        }

        self.fired = true;
        corner
    }

    pub fn reset(&mut self) {
        self.current = None;
        self.fired = false;
    }

    fn clamp_dwell_for(dwell_ms: i32) -> i32 {
        CornerActions::clamp_dwell(dwell_ms)
    }
}

//! Karşılığı: `Input/InputBinding.cs`.

use serde::{Deserialize, Serialize};

use super::input_action::InputAction;
use crate::shortcuts::Hotkey;

/// Bir eylemi tetikleyen şeyin türü. Ayar dosyasında ad olarak yazılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum InputTriggerKind {
    #[default]
    Shortcut,
    HotCorner,
    MouseButton,
}

/// Ekranın dört köşesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ScreenCorner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ScreenCorner {
    /// C#: `Enum.GetValues<ScreenCorner>()` (tanım sırasıyla).
    pub const ALL: [ScreenCorner; 4] =
        [ScreenCorner::TopLeft, ScreenCorner::TopRight, ScreenCorner::BottomLeft, ScreenCorner::BottomRight];
}

/// Fare tetikleyicisinin yüzeyi: çentik ya da dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum InputSurface {
    #[default]
    Notch,
    Dock,
}

/// Çentik ve dock için seçilebilir fare hareketleri (mevcut WheelAction / MiddleClickAction'ın genişletilmesi).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum MouseGesture {
    #[default]
    Wheel,
    MiddleClick,
    DoubleClick,
    RightClick,
}

/// Genel klavye kısayolu (RegisterHotKey ya da kanca ile).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortcutTrigger {
    pub hotkey: Hotkey,
}

impl ShortcutTrigger {
    pub fn new(hotkey: Hotkey) -> Self {
        ShortcutTrigger { hotkey }
    }
}

/// Ekran köşesi. `dwell_ms`: imlecin köşede kalması gereken süre (yalnız veri; zamanlayıcı Platform'da).
/// `push_back_px`: köşeden geri itme mesafesi, yanlış tetiklemeyi önlemek için (yalnız veri).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HotCornerTrigger {
    pub corner: ScreenCorner,
    pub dwell_ms: i32,
    pub push_back_px: i32,
}

impl HotCornerTrigger {
    pub fn new(corner: ScreenCorner, dwell_ms: i32, push_back_px: i32) -> Self {
        HotCornerTrigger { corner, dwell_ms, push_back_px }
    }
}

/// Çentik ya da dock üzerinde bir fare hareketi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseTrigger {
    pub surface: InputSurface,
    pub gesture: MouseGesture,
}

impl MouseTrigger {
    pub fn new(surface: InputSurface, gesture: MouseGesture) -> Self {
        MouseTrigger { surface, gesture }
    }
}

/// Tetikleyici. C#'taki `abstract record InputTrigger` ve üç türevi: her tür kendi verisini taşır;
/// tür bilinmiyorsa `InputBindingParser` onu düşürür.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputTrigger {
    Shortcut(ShortcutTrigger),
    HotCorner(HotCornerTrigger),
    Mouse(MouseTrigger),
}

/// Bir eylem ile bir tetikleyicinin bağı. `custom_action_id` yalnız `InputAction::RunCustomAction`
/// için doludur ve bir `CustomAction` kimliğine işaret eder. (C#: `CustomActionId = null` varsayılanı)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InputBinding {
    pub action: InputAction,
    pub trigger: InputTrigger,
    pub custom_action_id: Option<String>,
}

impl InputBinding {
    /// `new InputBinding(action, trigger)` (özel eylem kimliği null).
    pub fn new(action: InputAction, trigger: InputTrigger) -> Self {
        InputBinding { action, trigger, custom_action_id: None }
    }

    /// `new InputBinding(action, trigger, customActionId)`.
    pub fn with_custom_action_id(action: InputAction, trigger: InputTrigger, custom_action_id: Option<String>) -> Self {
        InputBinding { action, trigger, custom_action_id }
    }
}

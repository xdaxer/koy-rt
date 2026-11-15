//! Karşılığı: `Input/InputBindingParser.cs`.

use std::fmt::Debug;

use super::input_action::InputAction;
use super::input_binding::{
    HotCornerTrigger, InputBinding, InputSurface, InputTrigger, InputTriggerKind, MouseGesture, MouseTrigger, ScreenCorner,
    ShortcutTrigger,
};
use crate::shortcuts::Hotkey;
use crate::text;

/// Ayar dosyasından okunan ham bağ. Alanlar boş ya da bilinmeyen olabilir; çözüm `InputBindingParser`'dadır.
/// `trigger`: Shortcut: "Ctrl+Alt+N". HotCorner: "TopLeft". MouseButton: "Notch.Wheel".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputBindingData {
    pub action: Option<String>,
    pub kind: Option<String>,
    pub trigger: Option<String>,
    pub dwell_ms: Option<i32>,
    pub push_back_px: Option<i32>,
    pub custom_action_id: Option<String>,
}

/// Bir bağın düşürülme nedeni. Düşürülen bağ hiçbir zaman çalıştırılmaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputBindingDropReason {
    UnknownAction,
    UnknownKind,
    InvalidTrigger,
    InvalidParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBindingParseResult {
    pub binding: Option<InputBinding>,
    pub drop_reason: Option<InputBindingDropReason>,
}

impl InputBindingParseResult {
    pub fn new(binding: Option<InputBinding>, drop_reason: Option<InputBindingDropReason>) -> Self {
        InputBindingParseResult { binding, drop_reason }
    }

    pub fn is_valid(&self) -> bool {
        self.binding.is_some()
    }
}

/// Adı (`{:?}` = C# enum üye adı) metinle eşleştirilebilen kapalı enum. C#: `Enum.GetNames<T>()` + `Enum.Parse<T>`.
pub(crate) trait NamedValues: Copy + Debug + 'static {
    /// Tanım sırasıyla bütün değerler.
    fn values() -> &'static [Self];
}

impl NamedValues for InputAction {
    fn values() -> &'static [Self] {
        const V: [InputAction; 12] = [
            InputAction::ShowDesktop,
            InputAction::OpenSearch,
            InputAction::OpenNotch,
            InputAction::OpenKoyWindow,
            InputAction::ToggleWifi,
            InputAction::ToggleBluetooth,
            InputAction::ToggleDoNotDisturb,
            InputAction::SwitchProfile,
            InputAction::MoveNotchToNextMonitor,
            InputAction::MoveDockToNextMonitor,
            InputAction::RunCustomAction,
            InputAction::ToggleDock,
        ];
        &V
    }
}

impl NamedValues for InputTriggerKind {
    fn values() -> &'static [Self] {
        &[InputTriggerKind::Shortcut, InputTriggerKind::HotCorner, InputTriggerKind::MouseButton]
    }
}

impl NamedValues for ScreenCorner {
    fn values() -> &'static [Self] {
        &ScreenCorner::ALL
    }
}

impl NamedValues for InputSurface {
    fn values() -> &'static [Self] {
        &[InputSurface::Notch, InputSurface::Dock]
    }
}

impl NamedValues for MouseGesture {
    fn values() -> &'static [Self] {
        &[MouseGesture::Wheel, MouseGesture::MiddleClick, MouseGesture::DoubleClick, MouseGesture::RightClick]
    }
}

/// Ad eşleşmesi büyük-küçük harf duyarsız; sayısal metinler (örn. "3") kabul edilmez.
pub(crate) fn try_parse_name<T: NamedValues>(input: Option<&str>) -> Option<T> {
    if text::is_null_or_white_space(input) {
        return None;
    }
    let trimmed = text::trim(input?);

    T::values().iter().copied().find(|v| text::eq_ignore_case(&format!("{v:?}"), trimmed))
}

/// Ham bağı çözer. Bilinmeyen eylem ya da tür, bozuk tetikleyici ve aralık dışı değer düşürülür; hiçbiri
/// hata fırlatmaz, böylece bir bozuk satır bütün ayarı bozmaz.
pub struct InputBindingParser;

impl InputBindingParser {
    /// Varsayılan bekleme süresi (ms). Köşeye bilinçsiz değmeyi eler.
    pub const DEFAULT_DWELL_MS: i32 = 300;

    /// En çok bekleme süresi (ms); bunun üstü köşeyi ölü bırakır.
    pub const MAX_DWELL_MS: i32 = 5000;

    /// Varsayılan geri itme mesafesi (piksel).
    pub const DEFAULT_PUSH_BACK_PX: i32 = 4;

    pub const MAX_PUSH_BACK_PX: i32 = 200;

    pub fn parse(data: Option<&InputBindingData>) -> InputBindingParseResult {
        // JSON listesindeki null öğe; hata fırlatmadan düşer.
        let Some(data) = data else {
            return Self::drop(InputBindingDropReason::UnknownAction);
        };

        let Some(action) = try_parse_name::<InputAction>(data.action.as_deref()) else {
            return Self::drop(InputBindingDropReason::UnknownAction);
        };

        let Some(kind) = try_parse_name::<InputTriggerKind>(data.kind.as_deref()) else {
            return Self::drop(InputBindingDropReason::UnknownKind);
        };

        let trigger_text = data.trigger.as_deref().map(text::trim).unwrap_or("");
        let trigger = match kind {
            InputTriggerKind::Shortcut => {
                let Some(hotkey) = Hotkey::try_parse(trigger_text) else {
                    return Self::drop(InputBindingDropReason::InvalidTrigger);
                };
                InputTrigger::Shortcut(ShortcutTrigger::new(hotkey))
            }

            InputTriggerKind::HotCorner => {
                let Some(corner) = try_parse_name::<ScreenCorner>(Some(trigger_text)) else {
                    return Self::drop(InputBindingDropReason::InvalidTrigger);
                };

                let dwell = data.dwell_ms.unwrap_or(Self::DEFAULT_DWELL_MS);
                let push_back = data.push_back_px.unwrap_or(Self::DEFAULT_PUSH_BACK_PX);
                if !(0..=Self::MAX_DWELL_MS).contains(&dwell) || !(0..=Self::MAX_PUSH_BACK_PX).contains(&push_back) {
                    return Self::drop(InputBindingDropReason::InvalidParameter);
                }

                InputTrigger::HotCorner(HotCornerTrigger::new(corner, dwell, push_back))
            }

            InputTriggerKind::MouseButton => {
                let parts: Vec<&str> = trigger_text.split('.').map(text::trim).collect();
                if parts.len() != 2 {
                    return Self::drop(InputBindingDropReason::InvalidTrigger);
                }
                let (Some(surface), Some(gesture)) =
                    (try_parse_name::<InputSurface>(Some(parts[0])), try_parse_name::<MouseGesture>(Some(parts[1])))
                else {
                    return Self::drop(InputBindingDropReason::InvalidTrigger);
                };

                InputTrigger::Mouse(MouseTrigger::new(surface, gesture))
            }
        };

        let mut custom_id: Option<String> = None;
        if action == InputAction::RunCustomAction {
            custom_id = data.custom_action_id.as_deref().map(|s| text::trim(s).to_string());
            if custom_id.as_deref().is_none_or(str::is_empty) {
                return Self::drop(InputBindingDropReason::InvalidParameter);
            }
        }

        InputBindingParseResult::new(Some(InputBinding::with_custom_action_id(action, trigger, custom_id)), None)
    }

    fn drop(reason: InputBindingDropReason) -> InputBindingParseResult {
        InputBindingParseResult::new(None, Some(reason))
    }
}

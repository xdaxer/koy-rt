use super::i_radio_service::{RadioAccess, RadioState};

/// Kontrol Merkezi'ndeki radyo döşemesinin alt satırında ne yazacağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadioTileStatus {
    Loading,
    On,
    Off,
    NotFound,
    NoAccess,
    HardwareOff,
    Failed,
}

/// Döşemeye tıklanınca ne olacağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RadioTileAction {
    None,
    Toggle,
    OpenSettings,
    OpenPrivacySettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RadioTileState {
    pub status: RadioTileStatus,
    pub is_on: bool,
    pub action: RadioTileAction,
}

impl RadioTileState {
    pub fn new(status: RadioTileStatus, is_on: bool, action: RadioTileAction) -> Self {
        RadioTileState { status, is_on, action }
    }

    /// Radyo yazılımdan değiştirilemiyor; döşeme soluk gösterilir ve ayarlara yönlendirir.
    pub fn is_unavailable(&self) -> bool {
        matches!(self.status, RadioTileStatus::NotFound | RadioTileStatus::NoAccess | RadioTileStatus::HardwareOff)
    }

    /// Kapalı çentikte durum simgesi gösterilir mi: radyo var ve durumu biliniyor. Adaptör yoksa, izin yoksa ya da
    /// tarama sürüyorsa simge çıkmaz (masaüstünde Bluetooth'suz kasada boş bir simge yanıltıcı olur).
    pub fn is_present(&self) -> bool {
        matches!(
            self.status,
            RadioTileStatus::On | RadioTileStatus::Off | RadioTileStatus::HardwareOff | RadioTileStatus::Failed
        )
    }
}

/// Radyo izni ve durumundan döşemenin görünümünü ve tıklama davranışını belirler.
/// Değiştirilemeyen radyo için sahte bir anahtar göstermek yerine nedenini yazar ve ilgili Windows ayarını açar.
pub struct RadioTileResolver;

impl RadioTileResolver {
    /// C# imzası `Resolve(access, state, lastToggleFailed = false)`.
    pub fn resolve(access: RadioAccess, state: RadioState, last_toggle_failed: bool) -> RadioTileState {
        if matches!(access, RadioAccess::DeniedByUser | RadioAccess::DeniedBySystem) {
            return RadioTileState::new(RadioTileStatus::NoAccess, false, RadioTileAction::OpenPrivacySettings);
        }

        match state {
            RadioState::On => RadioTileState::new(
                if last_toggle_failed { RadioTileStatus::Failed } else { RadioTileStatus::On },
                true,
                RadioTileAction::Toggle,
            ),
            RadioState::Off => RadioTileState::new(
                if last_toggle_failed { RadioTileStatus::Failed } else { RadioTileStatus::Off },
                false,
                RadioTileAction::Toggle,
            ),
            RadioState::Disabled => RadioTileState::new(RadioTileStatus::HardwareOff, false, RadioTileAction::OpenSettings),
            RadioState::NotPresent => RadioTileState::new(RadioTileStatus::NotFound, false, RadioTileAction::OpenSettings),
            RadioState::Unknown => RadioTileState::new(RadioTileStatus::Loading, false, RadioTileAction::None),
        }
    }
}

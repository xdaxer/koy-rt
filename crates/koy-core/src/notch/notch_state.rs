/// Notch'ın görsel durumları.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NotchState {
    #[default]
    Collapsed,
    Peek,
    Expanded,
}

/// Durum geçişini tetikleyen olaylar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotchTrigger {
    MouseEnter,
    MouseLeave,
    Click,
    TrackChanged,
    AutoPeekTimeout,
    MouseLeaveDelayTimeout,
}

/// Saf durum geçiş kuralları (deterministik ve yan etkisiz).
pub struct NotchStateTransitions;

impl NotchStateTransitions {
    pub fn transition(current: NotchState, trigger: NotchTrigger) -> NotchState {
        match (current, trigger) {
            (NotchState::Collapsed, NotchTrigger::MouseEnter) => NotchState::Peek,
            (NotchState::Collapsed, NotchTrigger::TrackChanged) => NotchState::Peek,
            (NotchState::Collapsed, NotchTrigger::Click) => NotchState::Expanded,

            (NotchState::Peek, NotchTrigger::Click) => NotchState::Expanded,
            (NotchState::Peek, NotchTrigger::MouseLeave) => NotchState::Collapsed,
            (NotchState::Peek, NotchTrigger::AutoPeekTimeout) => NotchState::Collapsed,

            (NotchState::Expanded, NotchTrigger::Click) => NotchState::Collapsed,
            (NotchState::Expanded, NotchTrigger::MouseLeaveDelayTimeout) => NotchState::Collapsed,

            _ => current,
        }
    }
}
